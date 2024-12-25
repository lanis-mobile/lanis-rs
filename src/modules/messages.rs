use chrono::{DateTime, FixedOffset};
use reqwest::Client;
use crate::utils::constants::URL;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ConversationOverview {
    pub sender: Vec<String>,
    pub receiver: Vec<String>,
    pub subject: String,
    pub date: DateTime<FixedOffset>,
    pub visible: bool,
}

impl ConversationOverview {
    /// Get all [ConversationOverview]'s (hidden and visible)
    pub async fn get_root(client: &Client) {
        unimplemented!();
        //match client.get(URL::MESSAGES).query(&[("a", "headers"), ("getType", "All"), ("last", "0")]);
    }
}