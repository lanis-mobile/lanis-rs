use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde::{ser, Deserialize, Serialize};

use crate::{
    utils::{constants::URL, datetime::datetime_string_to_datetime},
    Error,
};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CalendarEntry {
    pub id: i32,
    pub school_id: Option<i32>,
    pub external_uid: Option<String>,
    pub responsible_id: Option<i32>,
    pub title: String,
    /// May be empty
    pub description: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub last_modified: Option<DateTime<Utc>>,
    pub place: Option<String>,
    /// The study group of the entry (Lerngruppe)
    pub study_group: Option<StudyGroup>,
    pub category: Option<CalendarEntryCategory>,
    /// Indicates if an entry is new
    pub new: bool,
    /// Indicates if an entry is public
    pub public: bool,
    // Indicates if an entry is private
    pub private: bool,
    /// Indicates if an entry is secret (probably)
    pub secret: bool,
    pub all_day: bool,
}

impl CalendarEntry {
    pub fn new(
        id: i32,
        school_id: Option<i32>,
        external_uid: Option<String>,
        responsible_id: Option<i32>,
        title: String,
        description: String,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        last_modified: Option<DateTime<Utc>>,
        place: Option<String>,
        study_group: Option<StudyGroup>,
        category: Option<CalendarEntryCategory>,
        new: bool,
        public: bool,
        private: bool,
        secret: bool,
        all_day: bool,
    ) -> Self {
        Self {
            id,
            school_id,
            external_uid,
            responsible_id,
            title,
            description,
            start,
            end,
            last_modified,
            place,
            study_group,
            category,
            new,
            public,
            private,
            secret,
            all_day,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StudyGroup {
    pub id: i32,
    pub name: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct CalendarEntryCategory {
    pub id: i32,
    pub name: String,
    /// a hexadecimal color (css)
    pub color: String,
}

pub async fn get_entries(
    from: NaiveDate,
    to: NaiveDate,
    search_query: Option<String>,
    client: Client,
) -> Result<Vec<CalendarEntry>, Error> {
    let categories = match client.get(URL::CALENDAR).send().await {
        Ok(response) => {
            let html = match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    return Err(Error::Html(format!(
                        "failed to parse html of '{}' with error '{}'",
                        URL::CALENDAR,
                        e
                    )))
                }
            };

            let json_categories = match html.split("var categories = new Array();").nth(1) {
                Some(part) => match part.split("var groups = new Array();").next() {
                    Some(part) => {
                        let content = part
                            .trim()
                            .replace("categories.push(", "")
                            .replace(");", ",");
                        let final_content = match content.rsplit_once(",") {
                            Some(split) => split.0.trim().to_string(),
                            None => content, // Happens if no categories exist at all
                        };
                        format!("{{{}}}", final_content.trim())
                    }
                    None => return Err(Error::Parsing(String::from(
                        "failed to parse json categories (missing first part of 'var groups...')",
                    ))),
                },
                None => return Err(Error::Parsing(String::from(
                    "failed to parse json categories (missing second part of 'var categories...')",
                ))),
            };

            let categories: Vec<CalendarEntryCategory> =
                match serde_json::from_str(json_categories.as_str()) {
                    Ok(result) => result,
                    Err(e) => {
                        return Err(Error::Parsing(format!(
                            "failed to parse json of categories with error '{}'",
                            e
                        )));
                    }
                };

            categories
        }
        Err(e) => {
            return Err(Error::Network(format!(
                "failed to get '{}' with error '{}'",
                URL::CALENDAR,
                e
            )))
        }
    };

    let f = String::from("getEvents");
    let s = search_query.unwrap_or_default();
    let start = format!("{}", from);
    let end = format!("{}", to);

    let events_json = match client
        .post(URL::CALENDAR)
        .form(&[("f", f), ("s", s), ("start", start), ("end", end)])
        .send()
        .await
    {
        Ok(response) => {
            // technically its a json but who cares
            match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    return Err(Error::Html(format!(
                        "failed to parse html of '{}' with error '{}'",
                        URL::CALENDAR,
                        e
                    )))
                }
            }
        }
        Err(e) => {
            return Err(Error::Network(format!(
                "failed to post '{}' with error '{}'",
                URL::CALENDAR,
                e
            )))
        }
    };

    #[derive(Debug, Serialize, Deserialize)]
    struct JsonEvent {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Institution")]
        school_id: Option<String>,
        #[serde(rename = "FremdUID")]
        external_uid: Option<String>,
        #[serde(rename = "Verantwortlich")]
        responsible_id: Option<String>,
        title: String,
        description: String,
        #[serde(rename = "Anfang")]
        start: String,
        #[serde(rename = "Ende")]
        end: String,
        #[serde(rename = "LetzteAenderung")]
        last_modified: Option<String>,
        #[serde(rename = "Ort")]
        place: Option<String>,
        #[serde(rename = "Lerngruppe")]
        study_group: Option<String>,
        category: Option<String>,
        #[serde(rename = "Neu")]
        new: String,
        #[serde(rename = "Oeffentlich")]
        public: String,
        #[serde(rename = "Privat")]
        private: bool,
        #[serde(rename = "Geheim")]
        secret: String,
        #[serde(rename = "allDay")]
        all_day: bool,
    }

    let json_events: Vec<JsonEvent> = match serde_json::from_str(&events_json) {
        Ok(events) => events,
        Err(e) => {
            return Err(Error::Parsing(format!(
                "failed to parse json of events with error '{}'",
                e
            )));
        }
    };

    for json_event in json_events {
        let id: i32 = json_event
            .id
            .parse()
            .map_err(|e| Error::Parsing(format!("failed to parse id as i32 with error '{}'", e)))?;

        let schoold_id: Option<i32> = match json_event.school_id {
            Some(id_string) => match id_string.parse() {
                Ok(school_id) => Some(school_id),
                Err(e) => {
                    return Err(Error::Parsing(format!(
                        "failed to parse school_id as i32 with error '{}'",
                        e
                    )));
                }
            },
            None => None,
        };

        let resposible_id: Option<i32> = match json_event.responsible_id {
            Some(id_string) => match id_string.parse() {
                Ok(responisble_id) => Some(responisble_id),
                Err(e) => {
                    return Err(Error::Parsing(format!(
                        "failed to parse resposible_id as i32 with error '{}'",
                        e
                    )));
                }
            },
            None => None,
        };

        let start = datetime_string_to_datetime(&json_event.start.replace("-", "."))
            .map_err(|e| {
                Error::DateTime(format!(
                    "failed to parse start datetime of entry with error '{}'",
                    e
                ))
            })?
            .to_utc();

        let end = datetime_string_to_datetime(&json_event.end.replace("-", "."))
            .map_err(|e| {
                Error::DateTime(format!(
                    "failed to parse end datetime of entry with error '{}'",
                    e
                ))
            })?
            .to_utc();

        let last_modified = match json_event.last_modified {
            Some(datetime_string) => Some(
                datetime_string_to_datetime(&datetime_string.replace("-", "."))
                    .map_err(|e| {
                        Error::DateTime(format!(
                            "failed to parse end datetime of entry with error '{}'",
                            e
                        ))
                    })?
                    .to_utc(),
            ),
            None => None,
        };
    }

    Err(Error::KeyPair)
}
