use anyhow::anyhow;
use grammers_client::types::Message;
use log::info;
use migration::{Migrator, MigratorTrait};
use moka::future::Cache;
use sea_orm::{
    sea_query, ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder,
};
use std::time::Duration;

#[derive(Clone)]
pub struct Db {
    db: DatabaseConnection,
    message_cache: Cache<i64, Vec<entity::messages::Model>>,
}

impl Db {
    pub async fn init(database_url: String) -> Self {
        let connection = sea_orm::Database::connect(database_url)
            .await
            .expect("Failed to initialize database connection!");
        Migrator::up(&connection, None).await.unwrap();

        let message_cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(Duration::from_secs(60 * 10))
            .build();

        Db {
            db: connection,
            message_cache,
        }
    }

    pub async fn is_message_already_saved(&self, message: &Message) -> bool {
        let messages = {
            if !self.message_cache.contains_key(&message.chat().id()) {
                self.preload_messages_by_chat_id(message.chat().id()).await;
            }

            self.message_cache.get(&message.chat().id()).await.unwrap()
        };

        let msg = messages.iter().find(|msg| msg.id == message.id());

        return if let Some(_) = msg { true } else { false };
    }

    async fn preload_messages_by_chat_id(&self, chat_id: i64) {
        let messages = entity::prelude::Messages::find()
            .filter(entity::messages::Column::ChatId.eq(chat_id))
            .all(&self.db)
            .await;

        self.message_cache.invalidate(&chat_id).await;
        match messages {
            Ok(messages) => {
                self.message_cache.insert(chat_id, messages).await;
            }
            Err(_) => {
                self.message_cache.insert(chat_id, vec![]).await;
            }
        }
    }

    pub async fn save_message(&self, message: &Message, has_media: bool) -> anyhow::Result<()> {
        let message_model = entity::messages::ActiveModel {
            id: ActiveValue::set(message.id()),
            chat_id: ActiveValue::Set(message.chat().id()),
            user_id: ActiveValue::Set(message.sender().unwrap().id()),
            text: ActiveValue::Set(message.text().to_string()),
            has_binary_data: ActiveValue::Set(has_media),
            binary_data_downloaded: ActiveValue::Set(false),
            binary_data_path: ActiveValue::Set(None),
            binary_data_type: ActiveValue::Set(None),
            date: ActiveValue::Set(message.date().to_string()),
        };

        entity::prelude::Messages::insert(message_model)
            .on_conflict(
                sea_query::OnConflict::columns(vec![
                    entity::messages::Column::Id,
                    entity::messages::Column::ChatId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .on_empty_do_nothing()
            .exec(&self.db)
            .await?;

        info!("Saved message from chat id {}", message.chat().id());

        Ok(())
    }

    pub async fn get_last_loaded_message_id_by_chat(&self, chat_id: i64) -> anyhow::Result<i32> {
        let msg = entity::prelude::Messages::find()
            .filter(entity::messages::Column::ChatId.eq(chat_id))
            .order_by_asc(entity::messages::Column::Date)
            .one(&self.db)
            .await;

        if let Some(msg) = msg? {
            Ok(msg.id)
        } else {
            Err(anyhow!("Possible missing chat: {}", chat_id))
        }
    }

    pub async fn get_last_message_by_chat(
        &self,
        chat_id: i64,
    ) -> anyhow::Result<entity::messages::Model> {
        let msg = entity::prelude::Messages::find()
            .filter(entity::messages::Column::ChatId.eq(chat_id))
            .order_by_desc(entity::messages::Column::Date)
            .one(&self.db)
            .await;

        if let Some(msg) = msg? {
            Ok(msg)
        } else {
            Err(anyhow!("Possible missing chat: {}", chat_id))
        }
    }

    pub async fn get_message_with_media_not_downloaded(&self) -> Option<entity::messages::Model> {
        let message = entity::prelude::Messages::find()
            .filter(entity::messages::Column::HasBinaryData.eq(true))
            .filter(entity::messages::Column::BinaryDataDownloaded.eq(false))
            .one(&self.db)
            .await;

        if let Ok(msg) = message {
            msg
        } else {
            None
        }
    }

    pub async fn save_message_media_status(
        &self,
        model: entity::messages::Model,
        downloaded: bool,
        path: Option<String>,
        media_type: Option<String>,
    ) -> anyhow::Result<()> {
        let mut message: entity::messages::ActiveModel = model.into();

        message.binary_data_downloaded = ActiveValue::Set(downloaded);
        message.binary_data_path = ActiveValue::Set(path);
        message.binary_data_type = ActiveValue::Set(media_type);

        message.update(&self.db).await?;
        Ok(())
    }

    pub async fn get_message_count_by_chat(&self, chat_id: i64) -> anyhow::Result<usize> {
        Ok(entity::messages::Entity::find()
            .filter(entity::messages::Column::ChatId.eq(chat_id))
            .count(&self.db)
            .await? as usize)
    }
}
