use crate::{
    consts::MODERATOR,
    database::{Database, Submission},
};
use anyhow::{anyhow, bail, Context as _, Result};
use futures::{join, lock::MutexGuard};
use serenity::{client::Context, model::{channel::{Channel, Message, Reaction}, id::{MessageId, RoleId}, misc::Mentionable}};

use crate::get_database;

pub async fn reaction_add(ctx: &Context, reaction: &Reaction) -> Result<()> {
    if reaction.emoji.unicode_eq("🏅") {
        let channel = reaction.channel(ctx).await?;
        let name = channel
            .id()
            .name(ctx)
            .await
            .ok_or(anyhow!("Could not get channel name"))?;
        let user = reaction.user(ctx).await?;
        let message = reaction.message(ctx).await?;
        let mut db = get_database().lock().await;
        if db.has_challenge(&name) {
            if user.id == message.author.id
                || user
                    .has_role(
                        ctx,
                        reaction
                            .guild_id
                            .ok_or(anyhow!("Could not get reaction's guild"))?,
                        RoleId(MODERATOR),
                    )
                    .await?
            {
                match db.get_challenge(&name) {
                    Some(challenge) => {
                        if challenge.has_submission(message.id) {
                            bail!("Submission already registered");
                        } else {
                            challenge.submissions.push(Submission {
                                user: user.id,
                                message: message.id,
                                content: message.content.clone(),
                            });
                            db.dirty();
                            channel
                                .id()
                                .send_message(ctx, |m| {
                                    m.content("Registered submission!")
                                        .reference_message(&message)
                                })
                                .await?;
                            message.pin(ctx).await?;
                            message.react(ctx, '🏅').await?;
                            println!("Registered submission");
                        }
                    }
                    None => {
                        channel
                            .id()
                            .send_message(ctx, |m| {
                                m.content(format!(
                                    "No competition is active here (yet) {}",
                                    user.mention()
                                ))
                            })
                            .await?;
                        bail!("Attempted to submit but channel hasn't started a competition");
                    }
                }
            }
        }
    } else if reaction.emoji.unicode_eq("❌") {
        println!("X emoji");
        let channel = reaction.channel(ctx).await?;
        let channel_name = channel
            .id()
            .name(ctx)
            .await
            .ok_or(anyhow!("Could not get channel name"))?;
        let message = reaction.message(ctx).await?;
        let mut db = get_database().lock().await;
        if db.has_challenge(&channel_name) {
            let challenge = db.get_challenge(&channel_name).unwrap();
            if challenge.has_submission(message.id) {
                challenge.remove_submission(message.id);
                channel.id().send_message(ctx, |m| {
                    m.content("Un-marked as a submission")
                        .reference_message(&message)
                }).await?;
                message.react(ctx, '❌').await?;
                message.unpin(ctx).await?;
                db.dirty();
            }
        }
    }
    Ok(())
}

pub async fn message_update(ctx: &Context, message: &Message) -> Result<()> {
    let mut db = get_database().lock().await;
    'challenges: for challenge in &mut db.challenges {
        for s in &mut challenge.submissions {
            if s.message == message.id {
                s.content = message.content.clone();
                db.dirty();
                break 'challenges;
            }
        }
    }
    Ok(())
}

pub async fn message_delete(ctx: &Context, message_id: &MessageId) -> Result<()> {
    let mut db = get_database().lock().await;
    for challenge in &mut db.challenges {
        let mut index = None;
        for (i, s) in challenge.submissions.iter().enumerate() {
            if &s.message == message_id {
                index = Some(i);
                break;
            }
        }
        if let Some(index) = index {
            challenge.submissions.remove(index);
            db.dirty();
            break;
        }
    }
    Ok(())
}