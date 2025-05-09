use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::utils::datetime::datetime_string_stupid_to_datetime;
use crate::{utils::constants::URL, Error};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CalendarEntry {
    pub id: String,
    pub school_id: Option<i32>,
    pub external_uid: Option<String>,
    /// The person / group who is responsible for the entry / event
    pub responsible: Option<CalendarEntryPerson>,
    pub target_audience: Vec<CalendarEntryPerson>,
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

/// May also be a group and not a single person
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CalendarEntryPerson {
    pub id: String,
    pub name: String,
}

impl CalendarEntry {
    pub fn new(
        id: String,
        school_id: Option<i32>,
        external_uid: Option<String>,
        responsible: Option<CalendarEntryPerson>,
        target_audience: Vec<CalendarEntryPerson>,
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
            responsible,
            target_audience,
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

impl StudyGroup {
    pub fn new(id: i32, name: String) -> Self {
        Self { id, name }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct CalendarEntryCategory {
    pub id: i32,
    pub name: String,
    /// a hexadecimal color (css)
    pub color: String,
}

/// Get all calendar entries in an specific time frame <br>
/// You can also use a optional search query to filter for events (this is server side)
pub async fn get_entries(
    from: NaiveDate,
    to: NaiveDate,
    search_query: Option<String>,
    client: &Client,
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
                            .replace(");", ",")
                            .replace("id", "\"id\"")
                            .replace("name", "\"name\"")
                            .replace("color", "\"color\"")
                            .replace("logo", "\"logo\"")
                            .replace("\'", "\"");
                        let final_content = match content.rsplit_once(",") {
                            Some(split) => split.0.trim().to_string(),
                            None => content, // Happens if no categories exist at all
                        };

                        format!("[{}]", final_content.trim())
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
        study_group: Option<serde_json::Value>,
        category: Option<String>,
        #[serde(rename = "Neu")]
        new: String,
        #[serde(rename = "Oeffentlich")]
        public: String,
        #[serde(rename = "Privat")]
        private: String,
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

    let mut entries = Vec::new();
    for json_event in json_events {
        let school_id: Option<i32> = match json_event.school_id {
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

        let start = datetime_string_stupid_to_datetime(&json_event.start)
            .map_err(|e| {
                Error::DateTime(format!(
                    "failed to parse start datetime of entry with error '{}'",
                    e
                ))
            })?
            .to_utc();

        let end = datetime_string_stupid_to_datetime(&json_event.end)
            .map_err(|e| {
                Error::DateTime(format!(
                    "failed to parse end datetime of entry with error '{}'",
                    e
                ))
            })?
            .to_utc();

        let last_modified = match json_event.last_modified {
            Some(datetime_string) => Some(
                datetime_string_stupid_to_datetime(&datetime_string)
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

        let study_group = match json_event.study_group {
            Some(study_group) => match study_group.as_str() {
                Some(json_object) => {
                    #[derive(Deserialize)]
                    struct JsonStudyGroup {
                        #[serde(rename = "Name")]
                        name: String,
                        #[serde(rename = "Id")]
                        id: String,
                    }

                    let json_study_group: JsonStudyGroup = serde_json::from_str(&json_object)
                        .map_err(|e| {
                            Error::Parsing(format!(
                                "failed to parse json of study group with error '{}'",
                                e
                            ))
                        })?;

                    let id: i32 = json_study_group.id.parse().map_err(|e| {
                        Error::Parsing(format!(
                            "failed to parse study group id ({}) as i32 with error '{}'",
                            json_study_group.id, e
                        ))
                    })?;

                    Some(StudyGroup::new(id, json_study_group.name))
                }
                None => None,
            },
            None => None,
        };

        let category = match json_event.category {
            Some(json_object) => {
                let id: i32 = json_object.parse().map_err(|e| {
                    Error::Parsing(format!("failed to parse category id with error '{}'", e))
                })?;
                categories.iter().find(|&c| c.id == id).cloned()
            }
            None => None,
        };

        let new = json_event.new != "nein";
        let public = json_event.public != "nein";
        let private = json_event.private != "nein";
        let secret = json_event.secret != "nein";

        let (responsible_name, target_audience) = {
            #[derive(Deserialize)]
            struct JsonDetails {
                properties: JsonDetailsProperties,
            }

            #[derive(Deserialize)]
            struct JsonDetailsProperties {
                #[serde(rename = "zielgruppen")]
                target_audience: Option<serde_json::Value>,
                #[serde(rename = "verantwortlich")]
                responsible_name: Option<String>,
            }

            let json_details = match client
                .post(URL::CALENDAR)
                .form(&[("f", "getEvent"), ("id", json_event.id.as_str())])
                .send()
                .await
            {
                Ok(response) => response.text().await.map_err(|e| {
                    Error::Html(format!(
                        "failed to parse html / json of entry details as text with error '{}'",
                        e
                    ))
                })?,
                Err(e) => {
                    return Err(Error::Network(format!(
                        "failed to post '{}' with error '{}'",
                        URL::CALENDAR,
                        e
                    )))
                }
            };

            let details: JsonDetails = serde_json::from_str(&json_details).map_err(|e| {
                Error::Parsing(format!(
                    "failed to parse json of entry details ({}) with error '{}'",
                    json_event.id, e
                ))
            })?;

            let raw_target_audience = details.properties.target_audience.unwrap_or_default();
            let json_target_audience = raw_target_audience.to_string();
            let target_audience_split = json_target_audience.split(",");

            let mut targets = Vec::new();
            for target in target_audience_split {
                let (broken_id, name) = target.split_once(":").unwrap_or_default();

                let id = broken_id
                    .replace("\"", "")
                    .replacen("-", "", 1)
                    .replacen("{", "", 1)
                    .trim()
                    .to_string();
                let name = name.replace("\"", "").replace("}", "").trim().to_string();

                if broken_id.is_empty() || name.is_empty() {
                    continue;
                }

                targets.push(CalendarEntryPerson { id, name });
            }

            (
                details
                    .properties
                    .responsible_name
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                targets,
            )
        };

        let responsible = match json_event.responsible_id {
            Some(id) => {
                if id.is_empty() || responsible_name.is_empty() {
                    None
                } else {
                    Some(CalendarEntryPerson {
                        id,
                        name: responsible_name,
                    })
                }
            }
            None => None,
        };

        entries.push(CalendarEntry::new(
            json_event.id,
            school_id,
            json_event.external_uid,
            responsible,
            target_audience,
            json_event.title,
            json_event.description,
            start,
            end,
            last_modified,
            json_event.place,
            study_group,
            category,
            new,
            public,
            private,
            secret,
            json_event.all_day,
        ));
    }

    Ok(entries)
}
