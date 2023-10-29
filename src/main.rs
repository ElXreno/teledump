mod bot;
mod config;
mod db;

use crate::bot::Bot;
use crate::config::Config;
use crate::db::Db;
use dotenvy::dotenv;
use log::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let config = Config::init();
    let db = Db::init(config.database_url).await;
    let bot = Bot::init(
        config.api_id,
        config.api_hash,
        config.teledump_session_path,
        config.media_path,
        db,
    )
    .await?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Got SIGINT; quitting early gracefully");
        },
        r = bot.run_event_loop() => {
            match r {
                Ok(_) => info!("Work done, gracefully shutting down..."),
                Err(e) => error!("Got error, exiting... {e}")
            }
        }
    }

    bot.save_session()?;

    Ok(())
}
