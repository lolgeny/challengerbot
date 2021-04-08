use chrono::{Duration, Utc};
use futures::{join, try_join};
use serenity::{client::Context, framework::standard::{macros::command, Args, CommandResult, Delimiter}, model::{channel::{
            Channel, Message, PermissionOverwrite, PermissionOverwriteType, Reaction, ReactionType,
        }, guild::Guild, id::{ChannelId, RoleId}, misc::Mentionable, permissions::Permissions}};
use tokio::{task, time::sleep};

use crate::{database::Challenge, get_database};

use super::consts::*;

const INVALID_MSG: &str =
", The bot couldn't extract details from your message, which probably means you didn't follow the correct format. \
Please rewrite your message to follow the format in the pinned message. \
If you believe this is a mistake, contact a moderator.";

#[inline]
fn is_suggestion(msg: &Message) -> bool {
    msg.channel_id.as_u64() == &CHALLENGE_SUGGESTION_CHANNEL && !msg.author.bot
}

async fn get_category(ctx: &Context, line: &str) -> Option<Channel> {
    let category_lower = line.to_lowercase();
    println!("loeeer {}", category_lower);
    let category = category_lower
        .strip_prefix("type:")
        .or_else(|| category_lower.strip_prefix("**type**:"))?
        .trim(); // verify category is good
    println!("{}", &category[2..category.len() - 1]);
    ChannelId::from(category[2..category.len() - 1].parse::<u64>().ok()?)
        .to_channel(ctx)
        .await
        .ok()
}

async fn try_post(ctx: &Context, msg: &Message, msg_content: &str) {
    if let Option::<()>::None = try {
        if is_suggestion(msg) {
            let mut parts = msg.content.splitn(3, '\n');
            let title = parts.next()?;
            let category_raw = parts.next()?;
            get_category(ctx, category_raw).await?;
            // msg
            // .guild(ctx).await.unwrap()
            // .channel_id_from_name(
            // ctx, category
            // ).await?;
            // if !CATEGORIES.contains(&&category[1..]) {None?}
            let content = format!("{}\n{}", category_raw, parts.next()?);
            if content.is_empty() {
                None?
            }
            ChannelId::from(CHALLENGE_DISCUSSION_CHANNEL)
                .send_message(ctx, |m| {
                    m.content(msg_content);
                    m.embed(|e| {
                        e.author(|a| a.name(&msg.author.name))
                            .color(EMBED_COLOR)
                            .title(title)
                            .description(content)
                    })
                })
                .await
                .unwrap();
        };
    } {
        msg.delete(ctx).await.unwrap();
        let notification = msg
            .channel_id
            .send_message(ctx, |m| {
                m.content(format!("{}{}", msg.author.mention(), INVALID_MSG))
            })
            .await
            .unwrap();
        let ctx = ctx.clone();
        task::spawn(async move {
            notification.delete(ctx).await.unwrap();
        });
    }
}

pub async fn message(ctx: &Context, msg: &Message) {
    join!(try_post(ctx, msg, "New challenge suggestion:"), async {
        if is_suggestion(msg) {
            msg.react(ctx, '👍').await.unwrap();
            msg.react(ctx, '👎').await.unwrap();
        }
    });
}

pub async fn message_update(ctx: &Context, msg: &Message) -> anyhow::Result<()> {
    try_post(ctx, msg, "Challenge updated:").await;
    Ok(())
}

pub async fn reaction_add(ctx: &Context, reaction: &Reaction) -> anyhow::Result<()> {
    if reaction.channel_id.as_u64() == &CHALLENGE_SUGGESTION_CHANNEL
        && reaction
            .user(ctx)
            .await
            .unwrap()
            .has_role(ctx, reaction.guild_id.unwrap(), RoleId::from(MODERATOR))
            .await
            .unwrap()
        && matches!(&reaction.emoji, ReactionType::Unicode(e) if e == "✅")
    {
        let msg = reaction.message(ctx).await.unwrap();
        let mut parts = msg.content.splitn(3, '\n');
        let title = parts.next().unwrap();
        let line = parts.next().unwrap();
        println!("line {}", line);
        let ty = get_category(ctx, line).await.unwrap().guild().unwrap();
        println!("{}", ty.name());
        let category = ty.category_id.unwrap();
        println!("{}", category.name(ctx).await.unwrap());
        if let Ok(new) = ty
            .guild(ctx)
            .await
            .unwrap()
            .create_channel(ctx, |g| g.name(title).category(category))
            .await
        {
            // only delete if successful
            let description = new
                .send_message(ctx, |m| {
                    m.content(msg.author.mention()).embed(|e| {
                        e.color(EMBED_COLOR)
                            .title(title)
                            .author(|a| a.name(&msg.author.name))
                            .description(parts.next().unwrap())
                    })
                })
                .await
                .unwrap();
            description.pin(ctx).await.unwrap();
            ChannelId::from(CHALLENGE_DISCUSSION_CHANNEL)
                .send_message(ctx, |m| {
                    m.content(format!("Challenge accepted - {}!", title))
                })
                .await
                .unwrap();
            msg.delete(ctx).await.unwrap();
        }
    }
    Ok(())
}

#[command]
#[allowed_roles(Moderator)]
pub async fn suggestion(ctx: &Context, cmd: &Message) -> CommandResult {
    let id = Args::new(&cmd.content, &[Delimiter::Single(' ')])
        .advance()
        .single::<String>()?;
    println!("Suggestion command called {}", id);
    let msg = ChannelId::from(CHALLENGE_SUGGESTION_CHANNEL)
        .message(ctx, id.parse::<u64>()?)
        .await?;
    println!("messa");
    message(ctx, &msg).await;
    Ok(())
}

#[command]
#[allowed_roles(Moderator)]
pub async fn start(ctx: &Context, cmd: &Message) -> CommandResult {
    println!("Time command");
    let mut args = Args::new(&cmd.content, &[Delimiter::Single(' ')]);
    args.advance();
    let days = args.single().unwrap();
    let end = Utc::now() + Duration::days(days);
    let channel = cmd.channel(ctx).await.unwrap().id();
    let (name, ..) = join!(channel.name(ctx), async {
        let msg = cmd
            .channel(ctx)
            .await
            .unwrap()
            .id()
            .send_message(ctx, |m| {
                m.content(format!(
                    "Started the competition! It will end in {} days time, on {}. Good luck.",
                    days, end
                ))
            })
            .await
            .unwrap();
        msg.pin(ctx).await.unwrap();
    });
    let mut db = get_database().lock().await;
    db.challenges.push(Challenge {
        active: true,
        name: name.unwrap(),
        time: end,
        channel,
        submissions: vec![],
    });
    db.dirty();
    Ok(())
}

pub async fn update_challenges(ctx: Context) {
    loop {
        println!("Checking challenges...");
        let mut db = get_database().lock().await;
        let now = Utc::now();
        let mut modified = false;
        for challenge in &mut db.challenges {
            if challenge.active && now > challenge.time {
                println!("Challenge completed");
                challenge.active = false;
                modified = true;
                let notification = challenge.channel.send_message(&ctx, |m| {
                    m.content(
                        "Time up! \
                        The deadline has passed for this challenge. \
                        If you have any problems, please ping the Moderator role\
                        in this category's discussion channel.",
                    )
                });
                let overwrite = PermissionOverwrite {
                    allow: Permissions::empty(),
                    deny: Permissions::SEND_MESSAGES
                        | Permissions::ATTACH_FILES
                        | Permissions::EMBED_LINKS,
                    kind: PermissionOverwriteType::Role(RoleId(828343353811927080)),
                };
                let perms = challenge.channel.create_permission(&ctx, &overwrite);
                let name = challenge.channel.name(&ctx).await.unwrap();
                let mut guild_channel = challenge
                    .channel
                    .to_channel(&ctx)
                    .await
                    .unwrap()
                    .guild()
                    .unwrap();
                let rename = guild_channel.edit(&ctx, |e| {
                    println!("✅{}", name);
                    e.name(format!("✅{}", name))
                });
                try_join!(notification, perms, rename).unwrap();
            }
        }
        if modified {db.dirty()}
        drop(db);
        sleep(tokio::time::Duration::new(60, 0)).await;
    }
}
