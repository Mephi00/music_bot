use dotenv::dotenv;
use poise::{Framework, FrameworkOptions};
use reqwest::Client as HttpClient;
use serenity::all::VoiceState;
use serenity::client::Context as SerenityContext;
use serenity::{
    async_trait,
    client::{Client, EventHandler},
    model::gateway::Ready,
    prelude::GatewayIntents,
};
use songbird::events::{Event, EventContext, EventHandler as VoiceEventHandler};
use songbird::SerenityInit;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

mod annoy_handlers;
mod commands;
mod utils;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: SerenityContext, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn voice_state_update(
        &self,
        ctx: SerenityContext,
        old: Option<VoiceState>,
        new: VoiceState,
    ) {
        annoy_handlers::voice_state_update(ctx, old, new).await;
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILDS;

    let framework = Framework::builder()
        .options(FrameworkOptions {
            commands: commands::get_commands(),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(utils::Data {
                    client: HttpClient::default(),
                    songs: Arc::new(Mutex::new(HashMap::new())),
                    annoy: Arc::new(RwLock::new(HashMap::new())),
                })
            })
        })
        .build();

    let mut client = Client::builder(&token, intents)
        .framework(framework)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

struct TrackErrorNotifier;

#[async_trait]
impl VoiceEventHandler for TrackErrorNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (state, handle) in *track_list {
                println!(
                    "Track {:?} encountered an error: {:?}",
                    handle.uuid(),
                    state.playing
                );
            }
        }

        None
    }
}
