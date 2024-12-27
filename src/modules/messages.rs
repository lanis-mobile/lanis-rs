use chrono::{DateTime, FixedOffset};
use markup5ever::interface::TreeSink;
use reqwest::{Client, Response};
use reqwest::header::HeaderValue;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use crate::base::account::AccountType;
use crate::utils::constants::URL;
use crate::utils::crypt::{decrypt_lanis_string_with_key, encrypt_lanis_data, LanisKeyPair};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum ConversationError {
    Network(String),
    /// Happens if anything goes wrong with parsing
    Parsing(String),
    Crypto(String),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ConversationOverview {
    pub id: i32,
    pub uid: String,
    pub sender: String,
    pub sender_id: i32,
    pub sender_short: String,
    pub receiver: Vec<String>,
    pub subject: String,
    pub date_time: DateTime<FixedOffset>,
    pub read: bool,
    pub visible: bool,
}

impl ConversationOverview {
    /// Get all [ConversationOverview]'s (hidden and visible)
    pub async fn get_root(client: &Client, keys: &LanisKeyPair) -> Result<Vec<ConversationOverview>, ConversationError> {
        match client.post(URL::MESSAGES).form(&[("a", "headers"), ("getType", "All"), ("last", "0")]).header("X-Requested-With", "XMLHttpRequest".parse::<HeaderValue>().unwrap()).send().await {
            Ok(response) => {
                #[derive(Serialize, Deserialize)]
                struct EncryptedResponseData {
                    total: i32,
                    rows: String,
                }

                let enc_text = response.text().await.map_err(|e| ConversationError::Parsing(format!("failed to parse response as text '{}'", e)))?;
                let enc_data = serde_json::from_str::<EncryptedResponseData>(&enc_text).map_err(|e| ConversationError::Parsing(format!("failed to parse response JSON as EncryptedResponseData '{}'", e)))?;

                let dec_rows_json_invalid = decrypt_lanis_string_with_key(&enc_data.rows, &keys.public_key_string).await.map_err(|e| ConversationError::Crypto(format!("failed to decrypt rows '{}'", e)))?;
                let dec_rows_json = format!("{}]", dec_rows_json_invalid.rsplit_once(']').unwrap_or(("[{}", "]")).0);

                #[derive(Serialize, Deserialize, Debug)]
                #[serde(rename_all = "PascalCase")]
                struct ConversationRowJson {
                    pub id: String,
                    pub uniquid: String,
                    pub sender: String,
                    pub sender_name: String,
                    #[serde(rename = "kuerzel")]
                    pub kuerzel: String,
                    pub betreff: String,
                    pub papierkorb: String,
                    #[serde(rename = "empf")]
                    pub empf: Vec<String>,
                    pub weitere_empfaenger: String,
                    pub datum_unix: i64,
                    /// If all conversations are hidden then this is missing for some reason
                    #[serde(rename = "unread")]
                    pub unread: Option<i32>,
                }

                impl From<ConversationRowJson> for Result<ConversationOverview, ConversationError> {
                    fn from(json_row: ConversationRowJson) -> Result<ConversationOverview, ConversationError> {
                        let id = json_row.id.parse::<i32>().map_err(|e| ConversationError::Parsing(format!("failed to parse id as i32 '{}'", e)))?;
                        let uid = json_row.uniquid.to_owned();
                        fn parse_name(html_string: &String) -> Result<String, ConversationError> {
                            let html = Html::parse_fragment(&html_string);
                            let selector = Selector::parse("span.label.label-info").unwrap();
                            let new_html = Html::parse_fragment(&match html.select(&selector).nth(0) {
                                Some(element) => element.inner_html(),
                                None => html.to_owned().html()
                            });

                            let mut html = new_html.to_owned();
                            let i_selector = Selector::parse("i.fas").unwrap();
                            let _ = new_html.select(&i_selector).map(|element| html.remove_from_parent(&element.id()));

                            Ok(html.root_element().text().collect::<String>().trim().to_string())
                        }
                        let sender = parse_name(&json_row.sender_name)?;
                        let sender_id = json_row.sender.parse::<i32>().map_err(|e| ConversationError::Parsing(format!("failed to parse sender as i32 '{}'", e)))?;
                        let sender_short = parse_name(&json_row.kuerzel)?;
                        let receiver = {
                            let mut result = Vec::new();
                            for receiver in &json_row.empf {
                                result.push(parse_name(&receiver)?);
                            };
                            result
                        };
                        let subject = json_row.betreff.to_owned();
                        let date_time_utc = DateTime::from_timestamp(json_row.datum_unix.to_owned(), 0).unwrap_or(DateTime::UNIX_EPOCH);
                        let date_time = date_time_utc.fixed_offset();
                        let read = match json_row.unread.unwrap_or(0) { 0 => true, 1 => false, _ => return Err(ConversationError::Parsing(String::from("failed to parse unread as bool (read) 'unexpected i32'"))) };
                        let visible = match json_row.papierkorb.as_str() { "ja" => false, "nein" => true, _ => return Err(ConversationError::Parsing(String::from("failed to parse visible as bool 'unexpected &str'"))) };

                        Ok(ConversationOverview {
                            id,
                            uid,
                            sender,
                            sender_id,
                            sender_short,
                            receiver,
                            subject,
                            date_time,
                            read,
                            visible
                        })
                    }
                }

                println!("JSON: {}", dec_rows_json);
                let json_rows = serde_json::from_str::<Vec<ConversationRowJson>>(&dec_rows_json).map_err(|e| ConversationError::Parsing(format!("failed to parse rows of decrypted json '{}'", e)))?;
                let overviews = {
                    let mut result: Vec<ConversationOverview> = Vec::new();
                    for json_row in json_rows {
                        result.push(<ConversationRowJson as Into<Result<ConversationOverview, ConversationError>>>::into(json_row)?);
                    }
                    result
                };

                Ok(overviews)
            }
            Err(error) => Err(ConversationError::Network(format!("failed to get conversations  '{}'", error)))
        }
    }

    async fn parse_recycle_response(&mut self, response: Response) -> Result<bool, ConversationError> {
        let response_bool = !response.text().await.map_err(|e| ConversationError::Parsing(format!("failed to get text of response '{}'", e)))?.parse::<bool>().map_err(|e| ConversationError::Parsing(format!("failed to parse response as bool '{}'", e)))?;
        let result = response_bool != self.visible;
        self.visible = response_bool;
        Ok(result)
    }

    /// Hides a visible conversation and returns the result if the hiding succeeded
    pub async fn hide(&mut self, client: &Client) -> Result<bool, ConversationError> {
        match client.post(URL::MESSAGES).form(&[("a", "deleteAll"), ("uniqid", self.uid.as_str())]).header("X-Requested-With", "XMLHttpRequest".parse::<HeaderValue>().unwrap()).send().await {
            Ok(response) => {
                self.parse_recycle_response(response).await
            },
            Err(e) => Err(ConversationError::Network(format!("failed to hide conversation '{}'", e)))
        }
    }

    /// Shows a hidden conversation and returns the result if the hiding succeeded
    pub async fn show(&mut self, client: &Client) -> Result<bool, ConversationError> {
        match client.post(URL::MESSAGES).form(&[("a", "recycleMsg"), ("uniqid", self.uid.as_str())]).header("X-Requested-With", "XMLHttpRequest".parse::<HeaderValue>().unwrap()).send().await {
            Ok(response) => {
                self.parse_recycle_response(response).await
            },
            Err(e) => Err(ConversationError::Network(format!("failed to show conversation '{}'", e)))
        }
    }


    /// Get the full [Conversation]
    pub async fn get(&self, client: &Client, keys: &LanisKeyPair) -> Result<(), ConversationError> {
        let enc_uid = encrypt_lanis_data(self.uid.as_bytes(), &keys.public_key_string);

        let query = [("a", "read"), ("msg", self.uid.as_str())];
        let enc_uid = enc_uid.await.map_err(|e| ConversationError::Crypto(format!("failed to encrypt uid '{}'", e)))?;
        let form = [("a", "read"), ("uniqid", enc_uid.as_str())];
        match client.post(URL::MESSAGES).query(&query).form(&form).header("X-Requested-With", "XMLHttpRequest".parse::<HeaderValue>().unwrap()).send().await {
            Ok(response) => {
                #[derive(Serialize, Deserialize, Debug)]
                struct EncJsonConversation {
                    /// actually an [i32]
                    error: String,
                    message: String,
                    time: i64,
                    /// actually an [i32]
                    #[serde(rename = "userId")]
                    user_id: String,
                    #[serde(rename = "ToolOptions")]
                    tool_options: JsonConversationToolOptions,
                    #[serde(rename = "UserTyp")]
                    user_typ: String,
                }

                #[derive(Serialize, Deserialize, Debug)]
                struct JsonConversation {
                    error: i32,
                    message: JsonConversationMessage,
                    time: i64,
                    user_id: i32,
                    tool_options: JsonConversationToolOptions,
                    user_typ: String,
                }

                #[derive(Serialize, Deserialize, Debug)]
                struct JsonConversationMessage {
                    /// Actually an [i32]
                    #[serde(rename = "Id")]
                    id: String,
                    #[serde(rename = "Uniquid")]
                    uid: String,
                    /// Actually an [i32]
                    #[serde(rename = "Sender")]
                    sender_id: String,
                    sender_type: String,

                }

                #[derive(Serialize, Deserialize, Debug)]
                struct JsonConversationMessageStats {

                }

                #[derive(Serialize, Deserialize, Debug)]
                struct JsonConversationToolOptions {
                    #[serde(rename = "AllowSuSToSuSMessages")]
                    allow_sus_to_sus_messages: String,
                }

                let text = response.text().await.map_err(|e| ConversationError::Parsing(format!("failed to parse text of response '{}'", e)))?;
                let encrypted_json = serde_json::from_str::<EncJsonConversation>(&text).map_err(|e| ConversationError::Parsing(format!("failed to parse encrypted json '{}'", e)))?;
                println!("{:#?}", encrypted_json);
                let decrypted_json = {
                    let mut result = encrypted_json;
                    let decrypted_message = decrypt_lanis_string_with_key(&result.message, &keys.public_key_string).await.map_err(|e| ConversationError::Crypto(format!("failed to decrypt message json '{}'", e)))?;
                    result.message = decrypted_message;
                    result
                };
                println!();
                println!("{:#?}", decrypted_json);

                Ok(())
            }
            Err(e) => Err(ConversationError::Network(format!("failed to post message '{e}'")))
        }
    }
}


#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Conversation {
    pub id: i32,
    pub uid: String,

    /// Is the [Conversation] a group chat
    pub group_chat: bool,
    /// Does the [Conversation] only allow private answers
    pub only_private_answers: bool,
    /// Does the [Conversation] allow replies
    pub can_reply: bool,

    /// How many students are in the [Conversation]
    pub amount_students: i32,
    /// How many teachers are in the [Conversation]
    pub amount_teachers: i32,
    /// How many parents are in the [Conversation]
    pub amount_parents: i32,

    /// All [Participant]'s / receiver that are in the [Conversation]
    pub participants: Vec<Participant>,

    /// All [Message]'s in the conversation
    pub messages: Vec<Message>,
}


#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Participant {
    /// The name of the [Participant]
    pub name: String,
    /// may be unknown if the [Participant] never wrote something in the chat
    pub account_type: AccountType
}

use crate::base::account::Account;
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Message {
    /// The date this [Message] was sent
    pub date: DateTime<FixedOffset>,
    /// The author of this [Message]
    pub author: Participant,
    /// Was this [Message] send by the current [Account]
    pub own: bool,
    /// The content of this [Message]
    pub content: String,
}