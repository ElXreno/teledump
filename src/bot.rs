use std::ffi::OsStr;
use std::io;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use grammers_client::types::Media::{Contact, Document, Photo, Sticker};
use grammers_client::types::Message;
use grammers_client::types::{Chat, Media};
use grammers_client::{Client, Config, InitParams, SignInError, Update};
use grammers_session::Session;
use log::{debug, error, info, warn};
use mime::Mime;
use moka::future::Cache;
use tokio::fs::create_dir_all;
use tokio::sync::{mpsc, Mutex, Semaphore};

use crate::db::Db;

pub type ApiId = i32;
pub type ApiHash = String;

#[derive(Clone)]
pub struct Bot {
    client: Client,
    client_handler: Client,
    teledump_session_path: String,
    media_path: String,
    db: Db,
    message_sender: mpsc::Sender<Message>,
    message_receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
    download_semaphore: Arc<Semaphore>,
}

impl Bot {
    pub async fn init(
        api_id: ApiId,
        api_hash: ApiHash,
        teledump_session_path: String,
        media_path: String,
        db: Db,
    ) -> anyhow::Result<Self> {
        let client = Client::connect(Config {
            session: Session::load_file_or_create(&teledump_session_path)?,
            api_id,
            api_hash: api_hash.to_string(),
            params: InitParams {
                catch_up: true,
                update_queue_limit: Some(2_000),
                ..Default::default()
            },
        })
        .await?;

        if !client.is_authorized().await? {
            let phone = prompt("Enter your phone number (international format): ")?;
            let token = client.request_login_code(&phone).await?;
            let code = prompt("Enter the code you received: ")?;
            let signed_in = client.sign_in(&token, &code).await;
            match signed_in {
                Err(SignInError::PasswordRequired(password_token)) => {
                    let hint = password_token.hint().unwrap_or("empty");
                    let prompt_message = format!("Enter the password (hint {}): ", &hint);
                    let password = prompt(prompt_message.as_str())?;

                    client
                        .check_password(password_token, password.trim())
                        .await?;
                }
                Ok(_) => (),
                Err(e) => panic!("{}", e),
            };
            info!("Signed in!");
            match client.session().save_to_file(&teledump_session_path) {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to save session! Will sign out & terminate...");
                    client.sign_out().await?;
                    panic!("Failed to save session! Error: {}", e)
                }
            }
        }

        let client_handler = client.clone();

        let (message_sender, message_receiver) = mpsc::channel(4096);
        let message_receiver = Arc::new(Mutex::new(message_receiver));
        let download_semaphore = Arc::new(Semaphore::new(8));

        Ok(Bot {
            client,
            client_handler,
            teledump_session_path,
            media_path,
            db,
            message_sender,
            message_receiver,
            download_semaphore,
        })
    }

    pub async fn run_event_loop(&self) -> anyhow::Result<()> {
        let message_process = tokio::spawn(self.clone().process_message_queue());

        while let Err(_) = self.save_user_private_chats().await {
            sleep(Duration::from_secs(10));
        }

        let (updates_result, message_process_result, media_process_result) = tokio::join!(
            self.handle_updates(),
            message_process,
            self.process_media_queue()
        );

        updates_result?;
        media_process_result?;
        match message_process_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(join_error) => Err(anyhow::anyhow!(join_error)),
        }
    }

    pub async fn get_user_private_chats(&self) -> anyhow::Result<Vec<Chat>> {
        let mut chats = vec![];
        let mut iter_dialogs = self.client_handler.iter_dialogs();
        while let Some(dialog) = iter_dialogs.next().await? {
            let chat = dialog.chat;
            if matches!(chat, Chat::User(_)) {
                chats.push(chat);
            }
        }

        Ok(chats)
    }

    pub async fn save_user_private_chats(&self) -> anyhow::Result<()> {
        let chats = self.get_user_private_chats().await?;

        for chat in &chats {
            let mut messages = self.client_handler.iter_messages(chat);
            let total_messages = messages.total().await?;

            println!(
                "Chat {} has {} total messages.",
                chat.name(),
                total_messages
            );

            let mut first_message = true;
            let mut partial_sync = false;
            let last_loaded_message_id_by_chat =
                self.db.get_last_loaded_message_id_by_chat(chat.id()).await;
            while let Some(message) = messages.next().await? {
                if first_message {
                    let last_message = self.db.get_last_message_by_chat(chat.id()).await;
                    if last_message.is_err() || message.date().to_string() != last_message?.date {
                        info!("Data for chat {} is outdated, full resync...", chat.id());
                    } else if let Ok(last_message_id) = last_loaded_message_id_by_chat {
                        info!(
                            "Data for chat {} is mostly up to date, patrial sync...",
                            chat.id()
                        );
                        messages = messages.offset_id(last_message_id);
                        partial_sync = true;
                    }

                    first_message = false;
                }
                let is_already_saved = self.save_message(&message).await.unwrap();

                if is_already_saved {
                    if let Ok(last_message_id) = last_loaded_message_id_by_chat {
                        if !partial_sync {
                            info!(
                                "Data for chat {} is mostly up to date, patrial sync...",
                                chat.id()
                            );
                            messages = messages.offset_id(last_message_id);
                            partial_sync = true;
                            // Or break it?
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_updates(&self) -> anyhow::Result<()> {
        while let Some(update) = &self.client_handler.next_update().await? {
            match update {
                Update::NewMessage(message) if !message.outgoing() => {
                    match self.handle_message(&message).await {
                        Ok(_) => {}
                        Err(err) => {}
                    };
                }
                Update::NewMessage(message) if message.outgoing() => {
                    match self.save_message(message).await {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }
                _ => {}
            };
        }

        Ok(())
    }

    async fn handle_message(&self, message: &Message) -> anyhow::Result<()> {
        let mut save_message = false;

        if message.sender().is_none() {
            warn!(
                "Got a message but sender is none (chat name {}, id '{}'), not handling...",
                message.chat().name(),
                message.chat().id()
            );
            return Ok(());
        }

        if matches!(message.chat(), Chat::User(_)) {
            info!(
                "Got a PM from user {} with id {}",
                message.sender().unwrap().name(),
                message.sender().unwrap().id()
            );
            save_message = true;
        } else {
            info!(
                "Got a message from chat '{}' with id {} from userid {}",
                message.chat().name(),
                message.chat().id(),
                message.sender().unwrap().id()
            );
        }

        if save_message {
            self.save_message(message).await?;
        }

        Ok(())
    }

    async fn save_message(&self, message: &Message) -> anyhow::Result<bool> {
        if self.db.is_message_already_saved(&message).await {
            debug!(
                "Message {} in chat {} already exists, skipping...",
                message.id(),
                message.chat().id()
            );
            return Ok(true);
        }

        self.message_sender.send(message.clone()).await.unwrap();
        Ok(false)
    }

    pub async fn process_message_queue(self) -> anyhow::Result<()> {
        let mut receiver = self.message_receiver.lock().await;
        loop {
            while let Some(message) = receiver.recv().await {
                let has_media = {
                    match message.media() {
                        None => false,
                        Some(_) => true,
                    }
                };

                self.db.save_message(&message, has_media).await.unwrap();
            }
        }
    }

    pub async fn process_media_queue(&self) -> anyhow::Result<()> {
        let dialog_cache = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(60 * 10))
            .build();

        loop {
            sleep(Duration::from_secs(15));

            while let Some(message_model) = self.db.get_message_with_media_not_downloaded().await {
                let chat = {
                    if !dialog_cache.contains_key(&message_model.chat_id) {
                        let chats = self.get_user_private_chats().await?;
                        for chat in chats {
                            dialog_cache.insert(chat.id(), chat).await;
                        }
                    }

                    dialog_cache.get(&message_model.chat_id).await.unwrap()
                };
                let messages = self
                    .client_handler
                    .get_messages_by_id(chat, &vec![message_model.id])
                    .await?;

                if let Some(message) = messages.first().unwrap() {
                    let mut media_type = None;
                    let mut media_path = None;
                    if let Some(media) = message.media() {
                        match media {
                            Photo(_) | Document(_) | Sticker(_) | Contact(_) => {
                                media_type = Some(get_file_extension(&media));
                                let mut media_name = media_type.clone().unwrap();
                                if let Document(document) = &media {
                                    info!(
                                        "Downloading document from message {} with size {} kbytes...",
                                        message.id(),
                                        document.size() / 1024
                                    );
                                    media_name = if document.name().is_empty() {
                                        format!(
                                            "-unknown{}",
                                            media_type.clone().unwrap_or(".unknown".to_string())
                                        )
                                    } else {
                                        format!("-{}", document.name())
                                    };
                                }

                                let dst =
                                    format!("{}/chat-{}", self.media_path, message.chat().id());
                                create_dir_all(&dst).await?;

                                media_path = Some(format!(
                                    "{}/media-{}{}",
                                    dst,
                                    message.id().to_string(),
                                    media_name
                                ));

                                if let Photo(photo) = media {
                                    if photo.photo.photo.is_some() {
                                        while let Err(e) = message
                                            .download_media(media_path.clone().unwrap())
                                            .await
                                        {
                                            warn!("Failed to download media from message {}, retrying after 5 secs... {}", message.id(), e);
                                            sleep(Duration::from_secs(5));
                                        }

                                        info!("Download media from message {} done!", message.id());
                                    } else {
                                        warn!(
                                            "Bypassing media download from message {}...",
                                            message.id()
                                        )
                                    }
                                }
                            }
                            _ => {}
                        };

                        self.db
                            .save_message_media_status(message_model, true, media_path, media_type)
                            .await?;
                    }
                }
            }
        }
    }

    pub fn save_session(&self) -> anyhow::Result<()> {
        let _ = &self
            .client
            .session()
            .save_to_file(&self.teledump_session_path)?;

        Ok(())
    }
}

fn prompt(message: &str) -> anyhow::Result<String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(message.as_bytes())?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut line = String::new();
    stdin.read_line(&mut line)?;
    Ok(line)
}

fn get_file_extension(media: &Media) -> String {
    match media {
        Photo(_) => ".jpg".to_string(),
        Sticker(sticker) => {
            get_mime_extension(sticker.document.mime_type()).unwrap_or(".sticker".to_string())
        }
        Document(document) => {
            let mime_type = document.mime_type();

            return if mime_type.is_none() || mime_type.unwrap().is_empty() {
                format!(
                    ".{}",
                    Path::new(document.name())
                        .extension()
                        .and_then(OsStr::to_str)
                        .unwrap_or_default()
                )
            } else {
                get_mime_extension(document.mime_type()).unwrap()
            };
        }
        Contact(_) => ".vcf".to_string(),
        _ => String::new(),
    }
}

fn get_mime_extension(mime_type: Option<&str>) -> Option<String> {
    return mime_type.map(|m| {
        let mime: Mime = m.parse().unwrap();
        format!(".{}", mime.subtype().to_string())
    });
}
