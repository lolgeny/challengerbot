use std::fs::File;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serenity::model::{
    channel::Message,
    id::{ChannelId, MessageId, UserId},
    prelude::User,
};

#[derive(Serialize, Deserialize)]
pub struct Database {
    pub challenges: Vec<Challenge>,
}
impl Database {
    pub fn load() -> Self {
        serde_json::from_reader(File::open("database.json").unwrap()).unwrap()
    }
    pub fn dirty(&self) {
        println!("dirtying!");
        serde_json::to_writer_pretty(File::create("database.json").unwrap(), self).unwrap();
    }
    pub fn has_challenge(&self, name: &str) -> bool {
        for challenge in &self.challenges {
            if challenge.name == name {
                return true;
            }
        }
        false
    }
    pub fn get_challenge(&mut self, name: &str) -> Option<&mut Challenge> {
        for challenge in &mut self.challenges {
            if challenge.name == name {
                return Some(challenge);
            }
        }
        None
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Challenge {
    #[serde(default = "Default::default")]
    pub active: bool,
    pub name: String,
    pub time: DateTime<Utc>,
    pub channel: ChannelId,
    pub submissions: Vec<Submission>,
}
impl Challenge {
    pub fn has_submission(&self, message: MessageId) -> bool {
        for submission in &self.submissions {
            if submission.message == message {
                return true;
            }
        }
        false
    }
    pub fn get_submission(&mut self, message: MessageId) -> Option<&mut Submission> {
        for submission in &mut self.submissions {
            if submission.message == message {
                return Some(submission);
            }
        }
        None
    }
    pub fn remove_submission(&mut self, message: MessageId) {
        let mut removed = vec![];
        for (index, submission) in self.submissions.iter().enumerate() {
            if submission.message == message {
                removed.push(index);
            }
        }
        for removed in removed {
            self.submissions.remove(removed);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Submission {
    pub user: UserId,
    pub message: MessageId,
    pub content: String,
}
