use chrono::{DateTime, FixedOffset};
use markup5ever::interface::TreeSink;
use reqwest::Client;
use reqwest::header::HeaderValue;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use crate::utils::constants::URL;
use crate::utils::crypt::{decrypt_lanis_string_with_key, LanisKeyPair};

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
                    #[serde(rename = "unread")]
                    pub unread: i32,
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
                        let read = match json_row.unread { 0 => true, 1 => false, _ => return Err(ConversationError::Parsing(String::from("failed to parse unread as bool (read) 'unexpected i32'"))) };
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
}