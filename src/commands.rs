use std::time::Duration;

use poise::{command, Command};
use serenity::all::User;
use songbird::{
    input::{Compose as _, YoutubeDl},
    TrackEvent,
};

use crate::{
    utils::{check_msg, Context, Data, Error},
    TrackErrorNotifier,
};

#[command(slash_command, guild_only)]
async fn join(ctx: Context<'_>) -> Result<(), Error> {
    let (guild_id, channel_id) = {
        let guild = ctx.guild().unwrap();
        let channel_id = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id);

        (guild.id, channel_id)
    };

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(ctx.reply("Not in a voice channel").await);

            return Ok(());
        }
    };

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Ok(handler_lock) = manager.join(guild_id, connect_to).await {
        // Attach an event handler to see notifications of all track errors.
        let mut handler = handler_lock.lock().await;
        handler.add_global_event(TrackEvent::Error.into(), TrackErrorNotifier);
    }

    check_msg(ctx.say("joined").await);

    Ok(())
}

#[command(slash_command, guild_only)]
async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(ctx.say(format!("Failed: {:?}", e)).await);
        }

        check_msg(ctx.say("Left voice channel").await);
    } else {
        check_msg(ctx.reply("Not in a voice channel").await);
    }

    Ok(())
}

#[command(slash_command, guild_only)]
async fn play(ctx: Context<'_>, #[description = "youtube url"] url: String) -> Result<(), Error> {
    let _ = ctx.defer().await;
    let do_search = !url.starts_with("http");

    let guild_id = ctx.guild_id().unwrap();

    let http_client = {
        let data = ctx.data();
        data.client.clone()
    };

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let mut src = if do_search {
            YoutubeDl::new_search(http_client, url)
        } else {
            YoutubeDl::new(http_client, url)
        };

        let track_handle =
            handler.enqueue_with_preload(src.clone().into(), Some(Duration::from_secs(5)));

        let track_title = src
            .aux_metadata()
            .await
            .unwrap()
            .title
            .unwrap_or_else(|| "This track has no title".to_string());

        let _ = track_handle.set_volume(0.7);

        check_msg(ctx.say(format!("Queued {}", &track_title)).await);

        let mut song_map = ctx.data().songs.lock().await;
        song_map.insert(track_handle.uuid(), track_title);

        check_msg(
            ctx.say(format!(
                "currently queued songs: {:?}",
                handler
                    .queue()
                    .current_queue()
                    .iter()
                    .filter_map(|t| song_map.get(&t.uuid()))
                    .collect::<Vec<&String>>()
            ))
            .await,
        )
    } else {
        check_msg(ctx.say("Not in a voice channel to play in").await);
    }

    Ok(())
}

#[command(slash_command, guild_only)]
async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(ctx.guild_id().unwrap()) {
        let handler = handler_lock.lock().await;

        let _ = handler.queue().skip().unwrap();

        check_msg(ctx.say("Skipped track").await);
    }

    Ok(())
}

#[command(slash_command, guild_only)]
async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().stop();
    }

    Ok(())
}

#[command(slash_command, guild_only)]
async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        match handler.queue().current() {
            Some(track) => {
                let _ = track.pause();
                check_msg(ctx.say("Track paused").await);
            }
            None => check_msg(ctx.say("Nothing is playing").await),
        }
    }

    Ok(())
}

#[command(slash_command, guild_only)]
async fn annoy(ctx: Context<'_>, #[description = "hmmm"] user: User) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    crate::annoy_handlers::add_annoy_user(&guild_id, user.id).await;

    check_msg(ctx.say("WARIO").await);

    Ok(())
}

#[command(slash_command, guild_only)]
async fn stop_annoy(ctx: Context<'_>, #[description = "hmmmm"] user: User) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    crate::annoy_handlers::remove_annoy_user(&guild_id, user.id).await;

    check_msg(ctx.say("no more WARIO").await);
    Ok(())
}

pub fn get_commands() -> Vec<Command<Data, Box<(dyn std::error::Error + std::marker::Send + Sync)>>>
{
    vec![join(), leave(), play(), skip(), annoy(), stop_annoy()]
}
