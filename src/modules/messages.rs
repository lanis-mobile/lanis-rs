use crate::base::account::AccountType;
use crate::utils::constants::URL;
use crate::utils::crypt::{decrypt_lanis_string_with_key, encrypt_lanis_data, LanisKeyPair};
use chrono::{DateTime, Utc};
use markup5ever::interface::TreeSink;
use reqwest::header::HeaderValue;
use reqwest::{Client, Response};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ConversationOverview {
    pub id: i32,
    pub uid: String,
    pub sender: Participant,
    pub receiver: Vec<Participant>,
    pub subject: String,
    pub date_time: DateTime<Utc>,
    pub read: bool,
    pub visible: bool,
}

impl ConversationOverview {
    fn parse_name(html_string: &String) -> Result<String, Error> {
        let mut html = Html::parse_fragment(&html_string);

        let i_selector = Selector::parse("i.fas").unwrap();
        let _ = html
            .to_owned()
            .select(&i_selector)
            .map(|element| html.remove_from_parent(&element.id()));

        Ok(html
            .root_element()
            .text()
            .collect::<String>()
            .trim()
            .to_string())
    }

    /// Parses a [Participant] from the name html <br>
    /// <br>
    /// NOTE: This will not include an id
    fn parse_participant(html_string: &String) -> Result<Participant, Error> {
        let mut html = Html::parse_fragment(&html_string);

        let i_selector = Selector::parse("i.fas").unwrap();
        let i_selector_teacher = Selector::parse("i.fas.fa-user").unwrap();
        let i_selector_student = Selector::parse("i.fas.fa-child").unwrap();
        let i_selector_parent = Selector::parse("i.fas.fa-user-circle").unwrap();

        let account_type = {
            if html.select(&i_selector_student).nth(0).is_some() {
                AccountType::Student
            } else if html.select(&i_selector_teacher).nth(0).is_some() {
                AccountType::Teacher
            } else if html.select(&i_selector_parent).nth(0).is_some() {
                AccountType::Parent
            } else {
                AccountType::Unknown
            }
        };

        let _ = html
            .to_owned()
            .select(&i_selector)
            .map(|element| html.remove_from_parent(&element.id()));
        let name = html
            .root_element()
            .text()
            .collect::<String>()
            .trim()
            .to_string();

        Ok(Participant {
            id: None,
            name,
            account_type,
        })
    }

    /// Get all [ConversationOverview]'s (hidden and visible)
    pub async fn get_root(
        client: &Client,
        keys: &LanisKeyPair,
    ) -> Result<Vec<ConversationOverview>, Error> {
        match client
            .post(URL::MESSAGES)
            .form(&[("a", "headers"), ("getType", "All"), ("last", "0")])
            .header(
                "X-Requested-With",
                "XMLHttpRequest".parse::<HeaderValue>().unwrap(),
            )
            .send()
            .await
        {
            Ok(response) => {
                #[derive(Serialize, Deserialize)]
                struct EncryptedResponseData {
                    total: i32,
                    rows: String,
                }

                let enc_text = response.text().await.map_err(|e| {
                    Error::Parsing(format!("failed to parse response as text '{}'", e))
                })?;
                let enc_data =
                    serde_json::from_str::<EncryptedResponseData>(&enc_text).map_err(|e| {
                        Error::Parsing(format!(
                            "failed to parse response JSON as EncryptedResponseData '{}'",
                            e
                        ))
                    })?;

                let dec_rows_json_invalid =
                    decrypt_lanis_string_with_key(&enc_data.rows, &keys.public_key_string)
                        .await
                        .map_err(|e| Error::Crypto(format!("failed to decrypt rows '{}'", e)))?;
                let dec_rows_json = format!(
                    "{}]",
                    dec_rows_json_invalid
                        .rsplit_once(']')
                        .unwrap_or(("[{}", "]"))
                        .0
                );

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

                impl From<ConversationRowJson> for Result<ConversationOverview, Error> {
                    fn from(json_row: ConversationRowJson) -> Result<ConversationOverview, Error> {
                        let id = json_row.id.parse::<i32>().map_err(|e| {
                            Error::Parsing(format!("failed to parse id as i32 '{}'", e))
                        })?;
                        let uid = json_row.uniquid.to_owned();
                        let mut sender =
                            ConversationOverview::parse_participant(&json_row.sender_name)?;
                        let sender_id = json_row.sender.parse::<i32>().map_err(|e| {
                            Error::Parsing(format!("failed to parse sender as i32 '{}'", e))
                        })?;
                        sender.id = Some(sender_id);
                        let receiver = {
                            let mut result = Vec::new();
                            for receiver in &json_row.empf {
                                result.push(ConversationOverview::parse_participant(&receiver)?);
                            }
                            result
                        };
                        let subject = json_row.betreff.to_owned();
                        let date_time = DateTime::from_timestamp(json_row.datum_unix.to_owned(), 0)
                            .unwrap_or(DateTime::UNIX_EPOCH);
                        let read = match json_row.unread.unwrap_or(0) {
                            0 => true,
                            1 => false,
                            _ => {
                                return Err(Error::Parsing(String::from(
                                    "failed to parse unread as bool (read) 'unexpected i32'",
                                )))
                            }
                        };
                        let visible = match json_row.papierkorb.as_str() {
                            "ja" => false,
                            "nein" => true,
                            _ => {
                                return Err(Error::Parsing(String::from(
                                    "failed to parse visible as bool 'unexpected &str'",
                                )))
                            }
                        };

                        Ok(ConversationOverview {
                            id,
                            uid,
                            sender,
                            receiver,
                            subject,
                            date_time,
                            read,
                            visible,
                        })
                    }
                }

                let json_rows = serde_json::from_str::<Vec<ConversationRowJson>>(&dec_rows_json)
                    .map_err(|e| {
                        Error::Parsing(format!("failed to parse rows of decrypted json '{}'", e))
                    })?;
                let overviews = {
                    let mut result: Vec<ConversationOverview> = Vec::new();
                    for json_row in json_rows {
                        result.push(<ConversationRowJson as Into<
                            Result<ConversationOverview, Error>,
                        >>::into(json_row)?);
                    }
                    result
                };

                Ok(overviews)
            }
            Err(error) => Err(Error::Network(format!(
                "failed to get conversations  '{}'",
                error
            ))),
        }
    }

    async fn parse_recycle_response(&mut self, response: Response) -> Result<bool, Error> {
        let response_bool = !response
            .text()
            .await
            .map_err(|e| Error::Parsing(format!("failed to get text of response '{}'", e)))?
            .parse::<bool>()
            .map_err(|e| Error::Parsing(format!("failed to parse response as bool '{}'", e)))?;
        let result = response_bool != self.visible;
        self.visible = response_bool;
        Ok(result)
    }

    /// Hides a visible conversation and returns the result if the hiding succeeded
    pub async fn hide(&mut self, client: &Client) -> Result<bool, Error> {
        match client
            .post(URL::MESSAGES)
            .form(&[("a", "deleteAll"), ("uniqid", self.uid.as_str())])
            .header(
                "X-Requested-With",
                "XMLHttpRequest".parse::<HeaderValue>().unwrap(),
            )
            .send()
            .await
        {
            Ok(response) => self.parse_recycle_response(response).await,
            Err(e) => Err(Error::Network(format!(
                "failed to hide conversation '{}'",
                e
            ))),
        }
    }

    /// Shows a hidden conversation and returns the result if the hiding succeeded
    pub async fn show(&mut self, client: &Client) -> Result<bool, Error> {
        match client
            .post(URL::MESSAGES)
            .form(&[("a", "recycleMsg"), ("uniqid", self.uid.as_str())])
            .header(
                "X-Requested-With",
                "XMLHttpRequest".parse::<HeaderValue>().unwrap(),
            )
            .send()
            .await
        {
            Ok(response) => Ok(!self.parse_recycle_response(response).await?),
            Err(e) => Err(Error::Network(format!(
                "failed to show conversation '{}'",
                e
            ))),
        }
    }

    /// Get the full [Conversation]
    pub async fn get(&self, client: &Client, keys: &LanisKeyPair) -> Result<Conversation, Error> {
        let enc_uid = encrypt_lanis_data(self.uid.as_bytes(), &keys.public_key_string);

        let query = [("a", "read"), ("msg", self.uid.as_str())];
        let enc_uid = enc_uid.await;
        let form = [("a", "read"), ("uniqid", enc_uid.as_str())];
        match client
            .post(URL::MESSAGES)
            .query(&query)
            .form(&form)
            .header(
                "X-Requested-With",
                "XMLHttpRequest".parse::<HeaderValue>().unwrap(),
            )
            .send()
            .await
        {
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
                    #[serde(rename = "teilnehmer")]
                    pub participants: i32,
                    #[serde(rename = "betreuer")]
                    pub supervisors: i32,
                    #[serde(rename = "eltern")]
                    pub parents: i32,
                }

                #[derive(Serialize, Deserialize, Debug)]
                struct JsonConversationToolOptions {
                    #[serde(rename = "AllowSuSToSuSMessages")]
                    allow_sus_to_sus_messages: String,
                }

                #[derive(Serialize, Deserialize, Debug)]
                pub struct DecJsonMessageField {
                    #[serde(rename = "Id")]
                    id: String,
                    #[serde(rename = "Uniquid")]
                    uid: String,
                    #[serde(rename = "Sender")]
                    sender: String,
                    #[serde(rename = "SenderArt")]
                    sender_type: String,
                    /// None if a reply
                    #[serde(rename = "groupOnly")]
                    group_only: Option<String>,
                    /// None if a reply
                    #[serde(rename = "privateAnswerOnly")]
                    private_answer_only: Option<String>,
                    /// None if a reply
                    #[serde(rename = "noAnswerAllowed")]
                    no_answer_allowed: Option<String>,
                    #[serde(rename = "Betreff")]
                    subject: String,
                    #[serde(rename = "Datum")]
                    date: String,
                    #[serde(rename = "Inhalt")]
                    content: String,
                    /// None if a reply
                    #[serde(rename = "Papierkorb")]
                    hidden: Option<String>,
                    #[serde(rename = "statistik")]
                    stats: JsonConversationMessageStats,
                    own: bool,
                    #[serde(rename = "username")]
                    sender_name: String,
                    noanswer: bool,
                    #[serde(rename = "Delete")]
                    delete: String,
                    #[serde(rename = "reply")]
                    replies: Vec<DecJsonMessageField>,
                    private: i32,
                    #[serde(rename = "ungelesen")]
                    unread: bool,
                    #[serde(rename = "AntwortAufAusgeblendeteNachricht")]
                    answer_to_hidden: bool,
                }

                let text = response.text().await.map_err(|e| {
                    Error::Parsing(format!("failed to parse text of response '{}'", e))
                })?;
                let encrypted_json =
                    serde_json::from_str::<EncJsonConversation>(&text).map_err(|e| {
                        Error::Parsing(format!("failed to parse encrypted json '{}'", e))
                    })?;
                let decrypted_json = {
                    let mut result = encrypted_json;
                    let decrypted_message =
                        decrypt_lanis_string_with_key(&result.message, &keys.public_key_string)
                            .await
                            .map_err(|e| {
                                Error::Crypto(format!("failed to decrypt message json '{}'", e))
                            })?;
                    result.message = format!(
                        "{}}}",
                        decrypted_message.rsplit_once("}").unwrap_or_default().0
                    );
                    result
                };
                let decrypted_json_message_field = serde_json::from_str::<DecJsonMessageField>(
                    // Who is responseble for that shit
                    &decrypted_json.message.replace(
                        "\"AntwortAufAusgeblendeteNachricht\":\"on\"",
                        "\"AntwortAufAusgeblendeteNachricht\":true",
                    ),
                )
                .map_err(|e| {
                    Error::Parsing(format!(
                        "failed to parse message field in decrypted json '{}'",
                        e
                    ))
                })?;

                fn parse_messages(json: &DecJsonMessageField) -> Result<Vec<Message>, Error> {
                    let mut messages = Vec::new();
                    messages.push({
                        let id = json.id.parse().map_err(|e| {
                            Error::Parsing(format!("failed to parse message id '{}'", e))
                        })?;
                        let mut date_split = json.date.split_once(" ").unwrap_or_default();
                        let mut date = date_split.0.to_string();
                        if date_split.0 == "heute" {
                            let new_date =
                                format!("{}", chrono::Local::now().date_naive().format("%d.%m.%Y"));
                            date = new_date
                        }
                        if date_split.0 == "gestern" {
                            let new_date = format!(
                                "{}",
                                (chrono::Local::now() - chrono::Duration::days(1))
                                    .date_naive()
                                    .format("%d.%m.%Y")
                            );
                            date = new_date
                        }
                        date_split.0 = date.as_str();
                        let date = date_time_string_to_datetime(
                            date_split.0,
                            &format!("{}:00", date_split.1),
                        )
                        .map_err(|e| {
                            Error::DateTime(format!(
                                "failed to parse date & time of message '{:?}'",
                                e
                            ))
                        })?
                        .to_utc();
                        let author = {
                            let id = Some(json.sender.parse().map_err(|e| {
                                Error::Parsing(format!("failed to parse sender id '{}'", e))
                            })?);
                            let name = ConversationOverview::parse_name(&json.sender_name)?;
                            let account_type = match json.sender_type.as_str() {
                                "Teilnehmer" => AccountType::Student,
                                "Betreuer" => AccountType::Teacher,
                                "Eltern" => AccountType::Parent,
                                _ => AccountType::Unknown,
                            };

                            Participant {
                                id,
                                name,
                                account_type,
                            }
                        };

                        let own = json.own.to_owned();
                        let content = json.content.to_owned();
                        let html_content =
                            Html::parse_document(&format!("<body>{}</body>", content));
                        let content = html_content
                            .root_element()
                            .text()
                            .collect::<String>()
                            .trim()
                            .to_owned();

                        Message {
                            id,
                            date,
                            author,
                            own,
                            content,
                        }
                    });

                    for reply in &json.replies {
                        let reply_messages = parse_messages(reply)?;
                        messages.extend(reply_messages);
                    }

                    Ok(messages)
                }

                let messages = parse_messages(&decrypted_json_message_field)?;

                async fn parse_participants(
                    messages: &Vec<Message>,
                    receivers: &Vec<Participant>,
                ) -> Result<Vec<Participant>, Error> {
                    let mut participants = Vec::new();
                    participants.append(receivers.to_owned().as_mut());

                    for message in messages {
                        if !participants.contains(&message.author) {
                            participants.push(message.author.to_owned());
                        }
                    }

                    Ok(participants)
                }

                let participants = parse_participants(&messages, &self.receiver).await?;

                let id = self.id.to_owned();
                let uid = self.uid.to_owned();

                async fn match_german_string_bool(string: &Option<String>) -> Result<bool, Error> {
                    if let Some(string) = string {
                        Ok(match string.as_str() {
                            "ja" => true,
                            "nein" => false,
                            _ => false,
                        })
                    } else {
                        Err(Error::Parsing(String::from(
                            "group chat entry is missing 'is None'",
                        )))
                    }
                }

                let group_chat =
                    match_german_string_bool(&decrypted_json_message_field.group_only).await?;
                let only_private_answers =
                    match_german_string_bool(&decrypted_json_message_field.private_answer_only)
                        .await?;
                let can_reply =
                    !match_german_string_bool(&decrypted_json_message_field.no_answer_allowed)
                        .await?;

                let mut amount_students = decrypted_json_message_field.stats.participants;
                let mut amount_teachers = decrypted_json_message_field.stats.supervisors;
                let mut amount_parents = decrypted_json_message_field.stats.parents;

                match self.sender.account_type {
                    AccountType::Student => amount_students += 1,
                    AccountType::Teacher => amount_teachers += 1,
                    AccountType::Parent => amount_parents += 1,
                    AccountType::Unknown => (),
                }

                let amount_participants = amount_students + amount_teachers + amount_parents;

                let visible = self.visible;
                let read = self.read;
                let date_time = self.date_time.to_owned();

                let subject = self.subject.to_owned();
                let author = self.sender.to_owned();

                Ok(Conversation {
                    id,
                    uid,
                    visible,
                    read,
                    date_time,

                    subject,
                    author,

                    group_chat,
                    only_private_answers,
                    can_reply,

                    amount_participants,
                    amount_students,
                    amount_teachers,
                    amount_parents,

                    participants,

                    messages,
                })
            }
            Err(e) => Err(Error::Network(format!("failed to post message '{e}'"))),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Conversation {
    pub id: i32,
    pub uid: String,
    pub visible: bool,
    pub read: bool,
    pub date_time: DateTime<Utc>,

    /// The subject of the [Conversation]
    pub subject: String,
    /// The person who created this [Conversation]
    pub author: Participant,

    /// Is the [Conversation] a group chat
    pub group_chat: bool,
    /// Does the [Conversation] only allow private answers
    pub only_private_answers: bool,
    /// Does the [Conversation] allow replies
    pub can_reply: bool,

    /// How many participants are in the [Conversation]
    pub amount_participants: i32,
    /// How many students / other are in the [Conversation]
    pub amount_students: i32,
    /// How many teachers are in the [Conversation]
    /// Note: technically these are supervisors but teachers are as far as I know always supervisors
    pub amount_teachers: i32,
    /// How many parents are in the [Conversation]
    pub amount_parents: i32,

    /// All [Participant]'s / receiver that are in the [Conversation]
    pub participants: Vec<Participant>,

    /// All [Message]'s in the conversation
    pub messages: Vec<Message>,
}

impl Conversation {
    pub async fn refresh(&mut self, client: &Client, key_pair: &LanisKeyPair) -> Result<(), Error> {
        let overview = ConversationOverview {
            id: self.id,
            uid: self.uid.to_owned(),
            sender: self.author.to_owned(),
            receiver: self.participants.to_owned(),
            subject: self.subject.to_owned(),
            date_time: self.date_time.to_owned(),
            read: self.read,
            visible: self.visible,
        };

        Ok(*self = ConversationOverview::get(&overview, client, key_pair).await?)
    }

    /// Reply to a [Conversation] (send a message) <br>
    /// `message` supports lanis formatting (see [here](https://support.schulportal.hessen.de/knowledgebase.php?article=664) for more info) <br>
    /// returns the UID of the new message (None if new message failed)
    pub async fn reply(
        &self,
        message: &str,
        client: &Client,
        key_pair: &LanisKeyPair,
    ) -> Result<Option<String>, Error> {
        #[derive(Serialize, Deserialize)]
        struct JSON {
            to: String,
            #[serde(rename = "groupOnly")]
            group_only: String,
            #[serde(rename = "privateAnswerOnly")]
            private_answers_only: String,
            message: String,
            #[serde(rename = "replyToMsg")]
            replay_to_message: String,
        }

        let sender_id = match self.messages.get(0) {
            Some(msg) => match msg.author.id {
                Some(id) => id,
                None => return Err(Error::Parsing(String::from("no author"))),
            },
            None => return Err(Error::Parsing(String::from("no messages"))),
        };

        fn bool_to_german(bool: &bool) -> String {
            if *bool {
                "ja".to_string()
            } else {
                "nein".to_string()
            }
        }

        let json = JSON {
            to: sender_id.to_string(),
            group_only: bool_to_german(&self.group_chat),
            private_answers_only: bool_to_german(&self.only_private_answers),
            message: message.trim().to_string(),
            replay_to_message: self.uid.to_owned(),
        };

        let json_string = serde_json::to_string(&json)
            .map_err(|e| Error::Parsing(format!("failed to serialize message '{e}'")))?;
        let enc_json_string =
            encrypt_lanis_data(json_string.as_bytes(), &key_pair.public_key_string).await;

        match client
            .post(URL::MESSAGES)
            .form(&[("a", "reply"), ("c", enc_json_string.as_str())])
            .send()
            .await
        {
            Ok(response) => {
                #[derive(Serialize, Deserialize)]
                struct ResponseJson {
                    back: bool,
                    /// UID
                    id: String,
                }

                let text = response.text().await.map_err(|e| {
                    Error::Parsing(format!("failed to parse response as text: {}", e))
                })?;
                let json: ResponseJson = serde_json::from_str(&text)
                    .map_err(|e| Error::Parsing(format!("failed to deserialize JSON: {}", e)))?;

                if !json.back {
                    return Ok(None);
                }
                Ok(Some(json.id))
            }
            Err(e) => Err(Error::Network(format!("failed to send message '{e}'"))),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Participant {
    pub id: Option<i32>,
    /// The name of the [Participant]
    pub name: String,
    /// may be unknown if the [Participant] never wrote something in the chat
    pub account_type: AccountType,
}

#[allow(unused_imports)]
use crate::base::account::Account;
use crate::utils::datetime::date_time_string_to_datetime;
use crate::Error;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Message {
    pub id: i32,
    /// The date this [Message] was sent
    pub date: DateTime<Utc>,
    /// The author of this [Message]
    pub author: Participant,
    /// Was this [Message] send by the current [Account]
    pub own: bool,
    /// The content of this [Message]
    pub content: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum ConversationType {
    /// No answers
    NoAnswersAllowed,
    /// Answers only to sender
    PrivateAnswersOnly,
    /// Answers to everyone
    GroupOnly,
    /// Private messages among themselves possible
    OpenChat,
}

impl ConversationType {
    /// Converts the enum to a String format that lanis expects
    pub fn to_lanis_string(&self) -> String {
        match &self {
            ConversationType::NoAnswersAllowed => String::from("noAnswerAllowed"),
            ConversationType::PrivateAnswersOnly => String::from("privateAnswerOnly"),
            ConversationType::GroupOnly => String::from("groupOnly"),
            ConversationType::OpenChat => String::from("openChat"),
        }
    }
}

impl Display for ConversationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            ConversationType::NoAnswersAllowed => write!(f, "NoAnswersAllowed"),
            ConversationType::PrivateAnswersOnly => write!(f, "PrivateAnswersOnly"),
            ConversationType::GroupOnly => write!(f, "GroupOnly"),
            ConversationType::OpenChat => write!(f, "OpenChat"),
        }
    }
}

/// Returns `true` if the use can freely choose what type a conversation should have
pub async fn can_choose_type(client: &Client) -> Result<bool, Error> {
    match client.get(URL::MESSAGES).send().await {
        Ok(response) => {
            let html = Html::parse_document(
                &response
                    .text()
                    .await
                    .map_err(|e| Error::Parsing(format!("failed to parse message '{:?}'", e)))?,
            );
            let options_selector = Selector::parse("#MsgOptions").unwrap();

            Ok(html.select(&options_selector).nth(0).is_some())
        }
        Err(e) => Err(Error::Network(format!("failed to get message page '{e}'"))),
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Receiver {
    pub id: String,
    pub name: String,
    pub account_type: AccountType,
}

/// Searches for a person using the provided query and returns the results as a [Vec] of [Receiver]'s <br> <br>
///
/// NOTE: Everything under 2 chars will return an empty [Vec<Receiver>]
pub async fn search_receiver(query: &str, client: &Client) -> Result<Vec<Receiver>, Error> {
    if query.len() < 2 {
        return Ok(Vec::new());
    }

    match client
        .get(URL::MESSAGES)
        .query(&[("a", "searchRecipt"), ("q", query)])
        .send()
        .await
    {
        Ok(response) => {
            let text = response.text().await.map_err(|e| {
                Error::Parsing(format!("failed to parse response as text '{:?}'", e))
            })?;
            if text.contains("\"items\":null") {
                return Ok(Vec::new());
            }

            #[derive(Serialize, Deserialize)]
            struct JSON {
                items: Vec<JSONItem>,
            }

            #[derive(Serialize, Deserialize)]
            struct JSONItem {
                #[serde(rename = "type")]
                account_type: String,
                id: String,
                text: String,
            }

            let json: JSON = serde_json::from_str(&text)
                .map_err(|e| Error::Parsing(format!("failed to parse response as JSON: {}", e)))?;

            let mut result = Vec::new();

            for item in json.items {
                let id = item.id;
                let name = item.text;
                let account_type = match item.account_type.as_str() {
                    // TODO: Parent accounts
                    "sus" => AccountType::Student,
                    "lul" => AccountType::Teacher,
                    _ => AccountType::Unknown,
                };

                result.push(Receiver {
                    id,
                    name,
                    account_type,
                })
            }

            Ok(result)
        }
        Err(e) => Err(Error::Network(format!(
            "failed to perform a search query '{e}'"
        ))),
    }
}

/// ### Creates a new Conversation
/// Receivers can be obtained with [search_receiver] <br>
/// Receivers should be one or higher
/// Text is the content of the message and supports lanis formatting (see [here](https://support.schulportal.hessen.de/knowledgebase.php?article=664)) <br>
/// Text should not be empty <br> <br>
///
/// returns the new UID of the Conversation if creation was successful
pub async fn create_conversation(
    receiver: &Vec<Receiver>,
    subject: &str,
    text: &str,
    client: &Client,
    key_pair: &LanisKeyPair,
) -> Result<Option<String>, Error> {
    #[derive(Serialize, Deserialize)]
    struct JSONItem {
        name: String,
        value: String,
    }

    let mut json_vec = Vec::new();
    json_vec.push(JSONItem {
        name: String::from("subject"),
        value: String::from(subject),
    });
    json_vec.push(JSONItem {
        name: String::from("text"),
        value: String::from(text),
    });

    for receiver in receiver {
        json_vec.push(JSONItem {
            name: String::from("to[]"), // This should be a crime
            value: receiver.id.to_owned(),
        })
    }

    let json = serde_json::to_string(&json_vec)
        .map_err(|e| Error::Parsing(format!("failed to serialize JSON: {}", e)))?;
    let enc_json = encrypt_lanis_data(json.as_bytes(), &key_pair.public_key_string).await;

    match client
        .post(URL::MESSAGES)
        .form(&[("a", "newmessage"), ("c", enc_json.as_str())])
        .send()
        .await
    {
        Ok(response) => {
            #[derive(Serialize, Deserialize)]
            struct ResponseJson {
                back: bool,
                /// UID
                id: String,
            }

            let text = response
                .text()
                .await
                .map_err(|e| Error::Parsing(format!("failed to parse response as text: {}", e)))?;
            let json: ResponseJson = serde_json::from_str(&text)
                .map_err(|e| Error::Parsing(format!("failed to deserialize JSON: {}", e)))?;

            if !json.back {
                return Ok(None);
            }
            Ok(Some(json.id))
        }
        Err(e) => Err(Error::Network(format!(
            "failed to create conversation '{e}'"
        ))),
    }
}
