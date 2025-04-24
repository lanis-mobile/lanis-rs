use crate::base::account::UntisSecrets;
use crate::utils::constants::URL;
use crate::utils::datetime::merge_naive_date_time_to_datetime;
use crate::Error;
use chrono::{DateTime, Days, NaiveDate, NaiveTime, Utc};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;
use untis::LessonCode;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum Provider {
    Lanis(LanisType),
    Untis(UntisSecrets),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum LanisType {
    All,
    Own,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Week {
    pub week: NaiveDate,
    pub week_type: Option<char>,
    pub entries: Vec<LessonEntry>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct LessonEntry {
    pub status: LessonEntryStatus,
    /// The names of the Subjects
    pub subjects: Vec<String>,
    pub teachers: Vec<String>,
    /// School hours are **only** available if [Provider::Lanis] is used
    pub school_hours: Vec<i32>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// The room numbers (e.g. B209)
    pub rooms: Vec<String>,
    /// Only available if [Provider::Untis] is used
    pub lesson_text: Option<String>,
    /// Only available if [Provider::Untis] is used
    pub substitution_text: Option<String>,
}

impl LessonEntry {
    pub fn new(
        status: LessonEntryStatus,
        subjects: Vec<String>,
        teachers: Vec<String>,
        school_hours: Vec<i32>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        rooms: Vec<String>,
        lesson_text: Option<String>,
        substitution_text: Option<String>,
    ) -> Self {
        Self {
            status,
            subjects,
            teachers,
            school_hours,
            start,
            end,
            rooms,
            lesson_text,
            substitution_text,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum LessonEntryStatus {
    Normal,
    Abnormal,
    Cancelled,
}

impl Week {
    pub async fn new(provider: Provider, client: &Client, date: NaiveDate) -> Result<Week, Error> {
        return match provider {
            Provider::Lanis(LanisType::All) => {
                let result = lanis(LanisType::All, client).await?;
                Ok(result)
            }
            Provider::Lanis(LanisType::Own) => {
                let result = lanis(LanisType::Own, client).await?;
                Ok(result)
            }
            // TODO: Implement Untis support
            Provider::Untis(secrets) => {
                let result = untis(secrets, date).await?;
                Ok(result)
            }
        };

        async fn lanis(lanis_type: LanisType, client: &Client) -> Result<Week, Error> {
            let mut week = NaiveDate::parse_from_str("01.01.1970", "%d.%m.%Y")
                .map_err(|_| Error::Parsing("failed to parse initial date".to_string()))?;
            let document = get(lanis_type, client).await?;

            let result = parse(&document, &mut week).await?;

            async fn parse(document_text: &String, week: &mut NaiveDate) -> Result<Week, Error> {
                let document = Html::parse_document(&document_text);

                let tr_selector = Selector::parse("tr").unwrap();
                let tr_td_selector = Selector::parse("tr>td").unwrap();

                let row = document.select(&tr_selector).nth(1);
                if row.is_none() {
                    return Err(Error::Html(
                        "there is no timetable row associated with the timetable element"
                            .to_string(),
                    ));
                }
                let rows = row.unwrap();

                let day_count = rows.select(&tr_td_selector).count() as i32;

                let date_selector = Selector::parse("div.col-md-6>span").unwrap();
                let date = document
                    .select(&date_selector)
                    .nth(0)
                    .unwrap()
                    .text()
                    .collect::<String>()
                    .replace("\n", "")
                    .replace(" ", "")
                    .replace("Stundenplangültig", "")
                    .replace("ab", "")
                    .trim()
                    .to_string();
                let date = NaiveDate::parse_from_str(&date, "%d.%m.%Y").map_err(|_| {
                    Error::DateTime(format!("Failed to parse date string '{}' as Date", date))
                })?;
                *week = date;

                let lesson_selector = Selector::parse("div.stunde ").unwrap();
                let school_hour_time_selector =
                    Selector::parse("span.hidden-xs>span.VonBis>small").unwrap();

                let rows = document.select(&tr_selector);
                let mut entries = vec![];
                let mut hour_times = BTreeMap::new();

                let elements = document.select(&school_hour_time_selector);

                for (i, element) in elements.enumerate() {
                    // Time of School hours
                    let text = element.text().collect::<String>();

                    let time_string = text.replace(" ", "");
                    let mut time_string = time_string.split("-");

                    async fn get_time(time_string: &mut String) -> Result<NaiveTime, Error> {
                        NaiveTime::parse_from_str(&format!("{}:00", time_string), "%H:%M:%S")
                            .map_err(|_| {
                                Error::DateTime(format!(
                                    "Failed to parse time string '{}' as NaiveTime",
                                    time_string
                                ))
                            })
                    }

                    let start_time = get_time(&mut time_string.nth(0).unwrap().to_string()).await?;
                    let end_time = get_time(&mut time_string.nth(0).unwrap().to_string()).await?;

                    hour_times.insert(i + 1, [start_time, end_time]);
                }

                let mut claimed_slots: BTreeMap<[i32; 2], bool> = BTreeMap::new();
                for i in 1..hour_times.len() as i32 + 1 {
                    for j in 1..day_count {
                        claimed_slots.insert([i, j], false);
                    }
                }

                for (ri, row) in rows.enumerate() {
                    if ri == 0 {
                        continue;
                    }
                    if ri == 1 {
                        continue;
                    }

                    let columns = row.select(&tr_td_selector);
                    for (ci, column) in columns.enumerate() {
                        if ci == 0 {
                            continue;
                        }

                        // Choose next free slot as day
                        let day_hour = {
                            let mut result = [1, 1];
                            for (key, value) in &claimed_slots {
                                if !value {
                                    result = *key;
                                    break;
                                }
                            }
                            result
                        };

                        let day = day_hour[1];
                        let current_school_hour = day_hour[0];

                        let attr = column.attr("rowspan");
                        if attr.is_none() {
                            claimed_slots.insert([current_school_hour, day], true);
                            continue;
                        }

                        let hours = attr.unwrap().parse::<i32>().map_err(|_| {
                            Error::Parsing("failed to parse rowspan as i32".to_string())
                        })?;

                        for lesson in column.select(&lesson_selector) {
                            let subjects = vec![lesson
                                .text()
                                .nth(1)
                                .unwrap()
                                .replace("\n", "")
                                .trim()
                                .to_string()];
                            let rooms = vec![lesson
                                .text()
                                .nth(2)
                                .unwrap()
                                .replace("\n", "")
                                .trim()
                                .to_string()];
                            let mut teachers = Vec::new();
                            for teacher in lesson.text().nth(3).unwrap().split("\n") {
                                if !teacher.trim().is_empty() {
                                    teachers.push(teacher.to_string().trim().to_string());
                                }
                            }
                            let school_hours = {
                                if hours >= 2 {
                                    let mut result = vec![];
                                    for i in current_school_hour..(current_school_hour + hours) {
                                        claimed_slots.insert([i, day], true);
                                        result.push(i);
                                    }
                                    result
                                } else {
                                    claimed_slots.insert([current_school_hour, day], true);
                                    vec![current_school_hour]
                                }
                            };

                            let start = merge_naive_date_time_to_datetime(
                                &date.checked_add_days(Days::new((day - 1) as u64)).unwrap(),
                                &hour_times
                                    .get(&(school_hours.first().unwrap().clone() as usize))
                                    .unwrap()[0],
                            )
                            .map_err(|e| {
                                Error::DateTime(format!(
                                    "Failed to parse NaiveDate & NaiveTime as DateTime: {:?}",
                                    e
                                ))
                            })?
                            .to_utc();

                            let end = merge_naive_date_time_to_datetime(
                                &date.checked_add_days(Days::new((day - 1) as u64)).unwrap(),
                                &hour_times
                                    .get(&(school_hours.last().unwrap().clone() as usize))
                                    .unwrap()[1],
                            )
                            .map_err(|e| {
                                Error::DateTime(format!(
                                    "Failed to parse NaiveDate & NaiveTime as DateTime: {:?}",
                                    e
                                ))
                            })?
                            .to_utc();

                            entries.push(LessonEntry {
                                status: LessonEntryStatus::Normal,
                                subjects,
                                teachers,
                                school_hours,
                                start,
                                end,
                                rooms,
                                lesson_text: None,
                                substitution_text: None,
                            });
                        }
                    }
                }

                let week_type_selector = Selector::parse("div.col-md-6.hidden-pdf.hidden-print>div.pull-right.hidden-pdf>span#aktuelleWoche").unwrap();
                let week_type = {
                    match document.select(&week_type_selector).nth(0) {
                        Some(week_type) => Some(
                            week_type
                                .text()
                                .collect::<String>()
                                .trim()
                                .to_string()
                                .chars()
                                .next()
                                .unwrap(),
                        ),
                        None => None,
                    }
                };

                let week = Week {
                    week: week.to_owned(),
                    week_type,
                    entries,
                };
                Ok(week)
            }

            async fn get(lanis_type: LanisType, client: &Client) -> Result<String, Error> {
                match client.get(URL::TIMETABLE).send().await {
                    Ok(response) => {
                        if response.status() != 302 {
                            return Err(Error::Network(format!(
                                "HTTP error status: {}",
                                response.status()
                            )));
                        }

                        let location = response.headers().get("Location");
                        if location == None {
                            return Err(Error::Network("no location header".to_string()));
                        }
                        let location = location
                            .unwrap()
                            .to_str()
                            .map_err(|_| {
                                Error::Parsing("failed to parse location header".to_string())
                            })?
                            .to_string();

                        match client
                            .get(format!("{}/{}", URL::TIMETABLE, location))
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if !response.status().is_success() {
                                    return Err(Error::Network(format!(
                                        "HTTP error status: {}",
                                        response.status()
                                    )));
                                }

                                let text = response.text().await.map_err(|_| {
                                    Error::Parsing("failed to parse response text".to_string())
                                })?;
                                let html = Html::parse_document(&text);

                                let all_selector = Selector::parse("#all").unwrap();
                                let own_selector = Selector::parse("#own").unwrap();

                                let select = {
                                    match lanis_type {
                                        LanisType::All => html.select(&all_selector).nth(0),
                                        LanisType::Own => html.select(&own_selector).nth(0),
                                    }
                                };

                                if select.is_none() {
                                    return Err(Error::Html("no matching tbody".to_string()));
                                }

                                let result = select.unwrap().html();

                                Ok(result)
                            }
                            Err(e) => Err(Error::Network(format!("{}", e))),
                        }
                    }
                    Err(e) => Err(Error::Network(format!("{}", e))),
                }
            }
            Ok(result)
        }

        async fn untis(untis_secrets: UntisSecrets, week: NaiveDate) -> Result<Week, Error> {
            let school = tokio::task::spawn_blocking(move || {
                untis::schools::get_by_name(untis_secrets.school_name.as_str())
                    .map_err(|e| Error::Credentials(format!("failed to get school: '{}'", e)))
            })
            .await
            .map_err(|e| Error::Threading(format!("Failed to join handle: '{}'", e)))??;

            let mut client = tokio::task::spawn_blocking(move || {
                school
                    .client_login(&untis_secrets.username, &untis_secrets.password)
                    .map_err(|e| Error::Credentials(format!("failed to login: '{}'", e)))
            })
            .await
            .map_err(|e| Error::Threading(format!("Failed to join handle: '{}'", e)))??;

            let timetable = tokio::task::spawn_blocking(move || {
                client
                    .own_timetable_for_week(&week.into())
                    .map_err(|e| Error::UntisAPI(format!("failed to get timetable: '{}'", e)))
            })
            .await
            .map_err(|e| Error::Threading(format!("Failed to join handle: '{}'", e)))??;

            let mut entries = Vec::new();

            for lesson in timetable {
                let status = match lesson.code {
                    LessonCode::Regular => LessonEntryStatus::Normal,
                    LessonCode::Irregular => LessonEntryStatus::Abnormal,
                    LessonCode::Cancelled => LessonEntryStatus::Cancelled,
                };

                let subjects = lesson
                    .subjects
                    .iter()
                    .map(|id| id.name.clone())
                    .collect::<Vec<_>>();
                let teachers = lesson
                    .teachers
                    .iter()
                    .map(|id| id.name.clone())
                    .collect::<Vec<_>>();
                let school_hours = Vec::new();
                let date = lesson.date.to_chrono();
                let start = merge_naive_date_time_to_datetime(&date, &lesson.start_time)
                    .map_err(|e| {
                        Error::DateTime(format!("Failed to convert start time of lesson: {:?}", e))
                    })?
                    .to_utc();
                let end = merge_naive_date_time_to_datetime(&date, &lesson.end_time)
                    .map_err(|e| {
                        Error::DateTime(format!("Failed to convert end time of lesson: {:?}", e))
                    })?
                    .to_utc();
                let rooms = lesson
                    .rooms
                    .iter()
                    .map(|id| id.name.clone())
                    .collect::<Vec<_>>();
                let lesson_text = if lesson.lstext.is_empty() {
                    None
                } else {
                    Some(lesson.lstext)
                };
                let substitution_text = lesson.subst_text;

                entries.push(LessonEntry::new(
                    status,
                    subjects,
                    teachers,
                    school_hours,
                    start,
                    end,
                    rooms,
                    lesson_text,
                    substitution_text,
                ));
            }

            Ok(Week {
                week,
                week_type: None,
                entries,
            })
        }
    }
}

