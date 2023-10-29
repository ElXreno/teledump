use crate::bot::{ApiHash, ApiId};
use std::env;
use std::fs::create_dir_all;

static API_ID: &str = "API_ID";
static API_HASH: &str = "API_HASH";
static STORE_PATH: &str = "STORE_PATH";

pub struct Config {
    pub api_id: ApiId,
    pub api_hash: ApiHash,
    pub store_path: String,
    pub media_path: String,
    pub database_url: String,
    pub teledump_session_path: String,
}

impl Config {
    pub fn init() -> Self {
        let api_id = env::var(API_ID)
            .expect(&format!("{API_ID} must be set!"))
            .parse::<ApiId>()
            .expect(&format!("Failed to parse {API_ID}"));
        let api_hash = env::var(API_HASH).expect(&format!("{API_HASH} must be set!"));
        let store_path = {
            let store_path = env::var(STORE_PATH).expect(&format!("{STORE_PATH} must be set!"));
            let store_path = shellexpand::full(&store_path).ok().unwrap();

            create_dir_all(store_path.as_ref()).unwrap();

            store_path.to_string()
        };

        let media_path = format!("{}/media", store_path);

        let database_url = format!("sqlite://{}/teledump.db?mode=rwc", store_path);

        let teledump_session_path = format!("{}/teledump.session", store_path);

        Config {
            api_id,
            api_hash,
            store_path,
            media_path,
            database_url,
            teledump_session_path
        }
    }
}
