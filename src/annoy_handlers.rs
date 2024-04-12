use std::collections::HashMap;
use std::sync::Arc;

use lazy_static::lazy_static;
use serenity::all::UserId;
use serenity::async_trait;
use serenity::http::CacheHttp;
use serenity::model::prelude::{ChannelId, GuildId};
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use songbird::model::payload::Speaking;
use songbird::tracks::TrackHandle;
use songbird::{
    CoreEvent, Event as VoiceEvent, EventContext, EventHandler as VoiceEventHandler, Songbird,
};
use tracing::info;

lazy_static! {
    static ref RECEIVER_STATE: Arc<RwLock<HashMap<GuildId, TrackHandle>>> =
        Arc::new(RwLock::new(HashMap::new()));
    static ref ANNOY_USERS: Arc<RwLock<HashMap<GuildId, Vec<UserId>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    static ref SSRC_MAP: Arc<RwLock<HashMap<u32, UserId>>> = Arc::new(RwLock::new(HashMap::new()));
}

pub async fn voice_state_update(ctx: Context, old: Option<VoiceState>, new: VoiceState) {
    let user = new.user_id.to_user(ctx.http()).await.unwrap();

    {
        if let Some(users) = ANNOY_USERS.read().await.get(&new.guild_id.unwrap()) {
            if !users.contains(&user.id) {
                return;
            }
        }
    }

    let bird_manager = songbird::get(&ctx)
        .await
        .expect("couldn't get songbird manager");

    let num_members_in_old_channel = match &old {
        Some(old_state) => Some(
            old_state
                .channel_id
                .unwrap()
                .to_channel(&ctx.http)
                .await
                .expect(&format!(
                    "channel {} doesnt exist",
                    old_state.channel_id.unwrap()
                ))
                .guild()
                .unwrap()
                .members(&ctx.cache)
                .unwrap()
                .len(),
        ),
        None => None,
    };

    if new.channel_id.is_none() {
        if num_members_in_old_channel.is_none() {
            info!("There is no old VoiceState");
            return;
        }

        match num_members_in_old_channel {
            Some(members) => {
                if !members > 1 {
                    let _ = bird_manager.leave(old.unwrap().guild_id.unwrap()).await;
                }
            }
            None => {}
        }

        return;
    }

    let current_channel_opt = bird_manager
        .get(new.guild_id.unwrap())
        .unwrap()
        .lock()
        .await
        .current_channel();

    let mut num_members_curr_channel = 0;

    if let Some(current_channel) = current_channel_opt {
        num_members_curr_channel = ChannelId::from(current_channel.0)
            .to_channel(&ctx.http)
            .await
            .unwrap()
            .guild()
            .unwrap()
            .members(ctx.cache)
            .unwrap()
            .len();
    }

    if user.bot
        || RECEIVER_STATE
            .read()
            .await
            .contains_key(&new.guild_id.unwrap())
            && (current_channel_opt.is_some() && num_members_curr_channel > 1
                || current_channel_opt.is_none())
    {
        info!(
            "already connected in guild {:?}",
            new.channel_id.unwrap().get()
        );
        return;
    }

    match new.user_id.get() {
        149572638933647360 => {}
        691267491992830014 => {}
        204063301137596417 => {}
        _ => {}
    }

    let annoy_map = ANNOY_USERS.read().await;

    if let Some(users) = annoy_map.get(&new.guild_id.unwrap()) {
        if users.contains(&user.id) {
            join_channel(
                &bird_manager,
                new.channel_id.unwrap(),
                new.guild_id.unwrap(),
            )
            .await;
        }
    }

    println!(
        "user {} joined channel {:?}",
        &user.name,
        new.channel_id.unwrap()
    );
}

pub async fn add_annoy_user(guild_id: &GuildId, user_id: UserId) {
    let mut annoy_map = ANNOY_USERS.write().await;

    if let Some(users) = annoy_map.get_mut(guild_id) {
        if !users.contains(&user_id) {
            users.push(user_id);
        }
    } else {
        annoy_map.insert(*guild_id, vec![user_id]);
    }
}

pub async fn remove_annoy_user(guild_id: &GuildId, user_id: UserId) {
    let mut annoy_map = ANNOY_USERS.write().await;

    if let Some(users) = annoy_map.get_mut(guild_id) {
        users.retain(|id| *id != user_id);
    }
}

async fn join_channel(
    // ctx: &Context,
    bird_manager: &Arc<Songbird>,
    channel_id: ChannelId,
    guild_id: GuildId,
) {
    let result = bird_manager.join(guild_id, channel_id).await;

    if let Ok(call) = result {
        let mut handler = call.lock().await;

        let in_memory = include_bytes!("../assets/wario.mp3");
        let in_memory_input = in_memory.into();

        let track_handle = handler.play_input(in_memory_input);

        let _ = track_handle.enable_loop();
        let _ = track_handle.pause();
        let _ = track_handle.set_volume(1.5);

        RECEIVER_STATE.write().await.insert(guild_id, track_handle);

        handler.add_global_event(CoreEvent::VoiceTick.into(), Receiver::new(guild_id));

        handler.add_global_event(
            CoreEvent::SpeakingStateUpdate.into(),
            Receiver::new(guild_id),
        );
    } else {
        println!("couldn't join channel {}: {:#?}", channel_id.get(), result);
    }
}

pub struct Receiver {
    guild_id: GuildId,
}

impl Receiver {
    pub fn new(guild_id: GuildId) -> Self {
        // You can manage state here, such as a buffer of audio packet bytes so
        // you can later store them in intervals.
        Self { guild_id }
    }
}

#[async_trait]
impl VoiceEventHandler for Receiver {
    #[allow(unused_variables)]
    async fn act(&self, ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        use EventContext as Ctx;
        match ctx {
            Ctx::SpeakingStateUpdate(Speaking {
                speaking,
                ssrc,
                user_id,
                ..
            }) => {
                if user_id.is_some() {
                    let mut state = SSRC_MAP.write().await;

                    state.insert(*ssrc, UserId::from(user_id.unwrap().0));
                }
            }

            Ctx::VoiceTick(data) => {
                let ssrc_map = SSRC_MAP.read().await.clone();
                let annoy_lock = ANNOY_USERS.read().await;
                let annoy_list_opt = annoy_lock.get(&self.guild_id).cloned();
                drop(annoy_lock);

                if annoy_list_opt.is_none() {
                    return None;
                }

                let annoy_list = annoy_list_opt.unwrap();

                let user_ids = data
                    .speaking
                    .keys()
                    .filter_map(|ssrc| {
                        if let Some(user) = ssrc_map.get(ssrc) {
                            if annoy_list.contains(user) {
                                Some(*user)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<UserId>>();

                // if user_ids.contains(&UserId::from(163367066659848192))
                //     || user_ids.contains(&UserId::from(204063301137596417))
                // {
                //     return None;
                // }

                let tracks = RECEIVER_STATE.read().await;

                let vc_state = tracks
                    .get(&self.guild_id)
                    .expect("couldn't find track handle");

                if user_ids.is_empty() {
                    let _ = vc_state.pause();
                } else {
                    let _ = vc_state.play();
                }
            }
            _ => {}
        }

        None
    }
}
