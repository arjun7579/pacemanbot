use std::sync::Arc;

use serenity::{builder::CreateEmbedAuthor, client::Context, prelude::Mentionable};

use crate::{
    cache::{
        guild_data::GuildData,
        players::PlayerSplitsData,
        split::{Split, Structure},
    },
    eprintln,
    utils::{format_time::format_time, millis_to_mins_secs::millis_to_mins_secs},
    ws::response::{Event, Item, Response},
};

use super::{
    consts::{OFFLINE_EMOJI, PEARL_EMOJI, ROD_EMOJI, SPECIAL_UNDERSCORE, TWITCH_EMOJI},
    get_run_info::get_run_info,
    run_info::RunType,
};

pub async fn handle_pace_event(
    ctx: Arc<Context>,
    response: &Response,
    live_link: String,
    stats_link: String,
    author: CreateEmbedAuthor,
    last_event: &Event,
    guild_data: &mut GuildData,
) {
    let run_info = match get_run_info(response, last_event) {
        Some(info) => info,
        None => {
            return eprintln!(
                "HandlePaceEvent: Unrecognized event id: {:#?}.",
                last_event.event_id
            );
        }
    };

    let player_data = match guild_data
        .players
        .get_mut(&response.nickname.to_lowercase())
    {
        Some(data) => data,
        None => {
            if guild_data.is_private {
                return println!(
                        "Skipping guild because player name: {} is not in the runners channel for guild name: {}", 
                        response.nickname, 
                        guild_data.name
                    );
            }
            let player_data = PlayerSplitsData::default();
            guild_data
                .players
                .insert(response.nickname.to_owned().to_lowercase(), player_data);
            guild_data
                .players
                .get_mut(&response.nickname.to_lowercase())
                .unwrap()
        }
    };
    let split_desc = match run_info.split.desc(&run_info.structure) {
        Some(desc) => desc,
        None => {
            return eprintln!(
                "HandlePaceEvent: get split desc for split: {:#?}",
                run_info.split
            );
        }
    };

    let split_emoji = match run_info.split.get_emoji(&run_info.structure) {
        Some(emoji) => emoji,
        None => {
            return eprintln!(
                "HandlePaceEvent: get split emoji for split: {:#?}",
                run_info.split
            );
        }
    };

    let roles_to_ping = guild_data
        .roles
        .iter()
        .filter(|role| {
            let (split_minutes, split_seconds) = millis_to_mins_secs(last_event.igt as u64);
            if role.guild_role.name.contains("PB") {
                if !guild_data.is_private {
                    return false;
                }
                let pb_minutes = player_data.get(&role.split).unwrap().to_owned();
                role.split == run_info.split && pb_minutes > split_minutes
            } else if role.guild_role.name.contains("+") {
                role.split == run_info.split
                    && role.runner.to_lowercase() == response.nickname.to_lowercase()
                    && role.minutes >= split_minutes
                    && (role.minutes != split_minutes || role.seconds > split_seconds)
            } else {
                role.split == run_info.split
                    && role.minutes >= split_minutes
                    && (role.minutes != split_minutes || role.seconds > split_seconds)
            }
        })
        .collect::<Vec<_>>();

    if roles_to_ping.is_empty() {
        return println!(
            "Skipping split: '{}' because there are no roles to ping in guild name: {}.",
            split_desc, guild_data.name
        );
    }

    let live_indicator = if response.user.live_account.is_some() {
        "🔴"
    } else {
        "⚪"
    };

    let mut item_data_content = String::new();

    match &response.item_data {
        Some(data) => {
            let pearl_count = data
                .estimated_counts
                .get(&Item::MinecraftEnderPearl)
                .unwrap_or(&0)
                .to_string();
            let mut rod_count = data
                .estimated_counts
                .get(&Item::MinecraftBlazeRod)
                .unwrap_or(&0)
                .to_string();
            if let Some(Structure::Bastion) = run_info.structure {
                if rod_count == String::from("0") && run_info.split == Split::SecondStructure {
                    rod_count = String::from("1+");
                }
            }
            if item_data_content == "" {
                item_data_content = format!("{} {}", ROD_EMOJI, rod_count);
            } else {
                item_data_content = format!("{}  {} {}", item_data_content, ROD_EMOJI, rod_count);
            }
            if item_data_content == "" {
                item_data_content = format!("{} {}", PEARL_EMOJI, pearl_count);
            } else {
                item_data_content =
                    format!("{}  {} {}", item_data_content, PEARL_EMOJI, pearl_count);
            }
        }
        None => {
            item_data_content = format!("{} {}", ROD_EMOJI, 0);
            item_data_content = format!("{}  {} {}", item_data_content, PEARL_EMOJI, 0);
        }
    }
    let metadata = format!(
        "{} {} - {} {}",
        live_indicator,
        format_time(last_event.igt as u64),
        split_desc,
        response.nickname.replace("_", SPECIAL_UNDERSCORE)
    );

    let ping_content = format!(
        "{}\n-# {}",
        metadata.clone(),
        roles_to_ping
            .iter()
            .map(|role| role.guild_role.mention().to_string())
            .collect::<Vec<_>>()
            .join(" "),
    );

    let pace_content = format!(
        "{}  {} - {}",
        split_emoji,
        format_time(last_event.igt as u64),
        split_desc,
    );

    match guild_data
        .pace_channel
        .send_message(&ctx, |m| {
            m.embed(|e| {
                e.set_author(author.clone());
                e.field(pace_content.clone(), "", false);
                if response.user.live_account.is_some() {
                    e.field(format!("{} {}", TWITCH_EMOJI, live_link.clone()), "", false);
                } else {
                    e.field(format!("{}  Offline", OFFLINE_EMOJI), "", false);
                }
                e.field("Splits", format!("[Link]({})", stats_link.clone()), true);
                e.field(
                    "Time",
                    format!("<t:{}:R>", (response.last_updated / 1000) as u64),
                    true,
                );
                e.field("Items", item_data_content.clone(), true);
                if let RunType::Bastionless = run_info.run_type {
                    e.field("Bastionless", "Yes", true);
                }
                e
            })
            .content(ping_content.to_owned())
        })
        .await
    {
        Ok(mut message) => {
            println!(
                "Sent pace-ping for user with name: '{}' for split: '{}' in guild name: {}.",
                response.nickname, split_desc, guild_data.name
            );
            let removable_roles = roles_to_ping
                .iter()
                .filter(|r| r.runner.as_str() != "")
                .map(|r| r.guild_role.mention())
                .collect::<Vec<_>>();
            let mut new_content = ping_content.to_owned();
            for role in removable_roles {
                let replacable_str = format!("{} ", role);
                new_content = new_content.replace(replacable_str.as_str(), "");
            }
            let content_removed_metadata =
                new_content.replace(format!("{}\n", metadata).as_str(), "");
            match message
                .edit(&ctx.http, |m| {
                    m.embed(|e| {
                        e.set_author(author);
                        e.field(pace_content, "", false);
                        if response.user.live_account.is_some() {
                            e.field(format!("{} {}", TWITCH_EMOJI, live_link.clone()), "", false);
                        } else {
                            e.field(format!("{}  Offline", OFFLINE_EMOJI), "", false);
                        }
                        e.field("Splits", format!("[Link]({})", stats_link), true);
                        e.field(
                            "Time",
                            format!("<t:{}:R>", (response.last_updated / 1000) as u64),
                            true,
                        );
                        e.field("Items", item_data_content, true);
                        if let RunType::Bastionless = run_info.run_type {
                            e.field("Bastionless", "Yes", true);
                        }
                        e
                    })
                    .content(content_removed_metadata)
                })
                .await
            {
                Ok(_) => (),
                Err(err) => {
                    return eprintln!("HandlePaceEvent: edit message due to: {}", err);
                }
            };
        }
        Err(err) => {
            return eprintln!(
                "HandlePaceEvent: send split: '{}' with roles: {:?} due to: {}",
                split_desc, roles_to_ping, err
            );
        }
    };
}
