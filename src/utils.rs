use reqwest::Client;
use serenity::{
    all::{GuildId, UserId},
    Result,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub fn check_msg<T>(result: Result<T>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub struct Data {
    pub client: Client,
    pub songs: Arc<Mutex<HashMap<Uuid, String>>>,
    pub annoy: Arc<RwLock<HashMap<GuildId, Vec<UserId>>>>,
} // User data, which is stored and accessible in all command invocations
pub type Context<'a> = poise::Context<'a, Data, Error>;
