#![feature(try_blocks)]
#![deny(rust_2018_idioms)]

use async_trait::async_trait;
use database::Database;
use futures::{join, lock::Mutex, try_join};
use serenity::{client::{bridge::gateway::GatewayIntents, ClientBuilder, Context, EventHandler}, framework::{
        standard::{
            macros::{command, group},
            CommandResult,
        },
        StandardFramework,
    }, model::{channel::{Message, Reaction}, event::MessageUpdateEvent, guild::GuildStatus, id::{ChannelId, GuildId, MessageId}, prelude::Ready}};
use tokio::task;

use challenges::{START_COMMAND, SUGGESTION_COMMAND};

mod challenges;
mod consts;
mod database;
mod submissions;

#[command]
async fn vote(ctx: &Context, msg: &Message) -> CommandResult {
    println!("Vote command");
    msg.react(ctx, '👍').await.unwrap();
    msg.react(ctx, '👎').await.unwrap();
    Ok(())
}

#[group]
#[commands(suggestion, vote, start)]
struct General;

static mut DATABASE: Option<Mutex<Database>> = None;
fn get_database() -> &'static Mutex<Database> {
    unsafe { DATABASE.as_ref().unwrap() }
}

#[tokio::main]
async fn main() {
    unsafe { DATABASE = Some(Mutex::new(database::Database::load())) }
    let token = std::env::var("token").expect("Could not get token");
    let framework = StandardFramework::new()
        .configure(|c| c.prefixes(&[">", "."]))
        .group(&GENERAL_GROUP);
    let mut client = ClientBuilder::new(token)
        .framework(framework)
        .event_handler(Handler)
        .intents(GatewayIntents::all())
        .await
        .expect("Could not create client");

    println!("Initialised client");
    client.start().await.expect("could not start client");
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _data: Ready) {
        println!("Client ready");
        task::spawn(challenges::update_challenges(ctx));
    }
    async fn message(&self, ctx: Context, msg: Message) {
        println!("Received message");
        // join!(
        // challenges::message(&ctx, &msg)
        // ).await;
        challenges::message(&ctx, &msg).await;
    }
    async fn message_update(
        &self,
        ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        println!("Message updated");
        if let Ok(new) = event.channel_id.message(&ctx, event.id).await {
            if let Err(e) = try_join!(
                challenges::message_update(&ctx, &new),
                submissions::message_update(&ctx, &new)
            ) {
                eprintln!("ERROR in message_update event: {}", e);
            }
        }
    }
    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        println!("Received reaction");
        if let Err(e) = try_join!(
            challenges::reaction_add(&ctx, &add_reaction),
            submissions::reaction_add(&ctx, &add_reaction)
        ) {
            eprintln!("ERROR in reaction_add event: {}", e);
        }
    }
    async fn message_delete(&self, ctx: Context, _channel_id: ChannelId, message_id: MessageId, _guild_id: Option<GuildId>) {
        if let Err(e) = submissions::message_delete(&ctx, &message_id).await {
            eprintln!("ERROR in message_delete event: {}", e);
        }
    }
}
