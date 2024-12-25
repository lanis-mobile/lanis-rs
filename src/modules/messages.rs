use chrono::{DateTime, FixedOffset};
use reqwest::Client;
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use crate::utils::constants::URL;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum ConversationError {
    Network(String),
    /// Happens if anything goes wrong with parsing
    Parsing(String),
    Crypto(String),
}

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
    pub async fn get_root(client: &Client) -> Result<Vec<ConversationOverview>, ConversationError> {
        match client.post(URL::MESSAGES).form(&[("a", "headers"), ("getType", "visibleOnly"), ("last", "0")]).header("X-Requested-With", "XMLHttpRequest".parse::<HeaderValue>().unwrap()).send().await {
            Ok(response) => {
                let json = response.text().await.map_err(|e| ConversationError::Parsing(format!("failed to parse response as text '{}'", e)));
                unimplemented!()
            }
            Err(error) => Err(ConversationError::Network(format!("failed to get conversations  '{}'", error)))
        }
    }
}