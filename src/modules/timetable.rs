use chrono::{DateTime, FixedOffset, NaiveDate};
use reqwest::Client;
use scraper::{Html, Selector};
use crate::utils::constants::URL;

#[derive(Debug, Clone)]
pub enum Error {
    Network(String),
    Parse(String),
    Html(String),
}

#[derive(Debug, Clone)]
pub enum Provider {
    Lanis(LanisType),
    Untis,
}

#[derive(Debug, Clone)]
pub enum LanisType {
    All,
    Own,
}

#[derive(Debug, Clone)]
pub struct Week {
    pub week: NaiveDate,
    pub week_type: char,
    pub monday: Vec<Entry>,
    pub tuesday: Vec<Entry>,
    pub wednesday: Vec<Entry>,
    pub thursday: Vec<Entry>,
    pub friday: Vec<Entry>,
    pub saturday: Vec<Entry>,
    pub sunday: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// The short of the subject (e.g. INF)
    pub name: String,
    /// The short of the teacher (e.g. RST)
    pub teacher: String,
    /// The full lastname of the teacher (only available if [Provider::Untis] is used as TimeTable [Provider])
    pub teacher_long: Option<String>,
    pub school_hours: Vec<i32>,
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
    /// The room number (e.g. B209)
    pub room: String,
    /// Only available if [Provider::Untis] is used as TimeTable [Provider]
    pub substitution: Option<Substitution>,
}

#[derive(Debug, Clone)]
pub struct Substitution {
    /// The short of the teacher (e.g. RST)
    pub new_teacher: String,
    /// The full lastname of the teacher
    pub new_teacher_long: String,
    pub text: String,
}

impl Week {
    pub async fn new(provider: Provider, client: &Client) -> Result<(), Error> {
        match provider {
            Provider::Lanis(LanisType::All) => {
                let result = lanis(LanisType::All, client).await?;
                return Ok(result)
            }
            Provider::Lanis(LanisType::Own) => {
                let result = lanis(LanisType::Own, client).await?;
                return Ok(result)
            }
            // TODO: Implement Untis support
            Provider::Untis => {
                return Ok(())
                // Ok(untis(&client).await?)
            }
        }

        async fn lanis(lanis_type: LanisType, client: &Client) -> Result<(), Error> {
            let mut week = NaiveDate::parse_from_str("01.01.1970", "%d.%m.%Y").map_err(|_| Error::Parse("failed to parse initial date".to_string()))?;
            let document = get(LanisType::All, client).await?;


            let result = parse(&document, &mut week).await?;

            async fn parse(document_text: &String, mut week: &mut NaiveDate) -> Result<(), Error> {
                let document = Html::parse_document(&document_text);

                let tr_selector = Selector::parse("tr").unwrap();
                let tr_td_selector = Selector::parse("tr>td").unwrap();

                let row = document.select(&tr_selector).nth(1);
                if row.is_none() {
                    return Err(Error::Html("there is no timetable row associated with the timetable element".to_string()));
                }
                let rows = row.unwrap();

                let day_count = rows.select(&tr_td_selector).count() as i32 - 1;

                let date_selector = Selector::parse("div.col-md-6>span").unwrap();
                let date = document.select(&date_selector).nth(0).unwrap().text().collect::<String>().replace("\n", "").replace(" ", "").replace("Stundenplangültig", "").replace("ab", "").trim().to_string();
                let date = NaiveDate::parse_from_str(&date, "%d.%m.%Y").map_err(|_| Error::Parse(format!("Failed to parse date string '{}' as Date", date)))?;
                *week = date;

                let lesson_selector = Selector::parse("div.stunde ").unwrap();
                let school_hour_selector = Selector::parse("span.hidden-xs>b").unwrap();
                let rows = document.select(&tr_selector);
                let mut day = 0;
                for row in rows {
                    if row.select(&tr_td_selector).nth(0).is_none() || row.select(&tr_td_selector).nth(0).unwrap().text().collect::<String>().replace(" ", "").replace("\n", "").is_empty() { continue; }

                    let mut current_school_hour = -1;

                    let columns = row.select(&tr_td_selector);
                    for column in columns {
                        if column.text().collect::<String>().trim().is_empty() { continue; }

                        let element = column.select(&school_hour_selector).nth(0);
                        if element.is_some() {
                            let result = element.unwrap().text().collect::<String>().replace(". Stunde", "").trim().parse::<i32>().unwrap();
                            current_school_hour = result;
                        }

                        let attr = column.attr("rowspan");
                        if attr.is_none() { continue; }

                        let hours = attr.unwrap().parse::<i32>().map_err(|_| Error::Parse("failed to parse rowspan as i32".to_string()))?;

                        for lesson in column.select(&lesson_selector) {
                            let name = lesson.text().nth(1).unwrap().replace("\n","").trim().to_string();
                            let room = lesson.text().nth(2).unwrap().replace("\n","").trim().to_string();
                            let teacher = lesson.text().nth(3).unwrap().replace("\n","").trim().to_string();
                            let school_hours = {
                                if hours >= 2 {
                                    let mut result = vec![];
                                    for i in current_school_hour..(current_school_hour+hours) {
                                        result.push(i);
                                    }
                                    result
                                } else {
                                    vec![current_school_hour]
                                }
                            };

                            println!("Name: {} room: {} teacher: {} school_hours {:?} day: {}", name, room, teacher, school_hours, day);
                        }

                        if day == 4 {
                            day == 0;
                        } else {
                            day += 1;
                        }
                    }
                }

                Ok(())
            }

            async fn get(lanis_type: LanisType, client: & Client) -> Result<String, Error> {
                match client.get(URL::TIMETABLE).send().await {
                    Ok(response) => {
                        if response.status() != 302 {
                            return Err(Error::Network(format!("HTTP error status: {}", response.status())))
                        }

                        let location = response.headers().get("Location");
                        if location == None { return Err(Error::Network("no location header".to_string())); }
                        let location = location.unwrap().to_str().map_err(|_| Error::Parse("failed to parse location header".to_string()))?.to_string();

                        match client.get(format!("{}/{}", URL::TIMETABLE, location)).send().await {
                            Ok(response) => {
                                if !response.status().is_success() {
                                    return Err(Error::Network(format!("HTTP error status: {}", response.status())))
                                }

                                let text = response.text().await.map_err(|e| Error::Parse("failed to parse response text".to_string()))?;
                                let html = Html::parse_document(&text);

                                let all_selector = Selector::parse("#all").unwrap();
                                let own_selector = Selector::parse("#own").unwrap();

                                let select = {
                                    match lanis_type {
                                        LanisType::All => {
                                            html.select(&all_selector).nth(0)
                                        }
                                        LanisType::Own => {
                                            html.select(&own_selector).nth(0)
                                        }
                                    }
                                };

                                if select.is_none() { return Err(Error::Html("no matching tbody".to_string())) }

                                let result = select.unwrap().html();

                                Ok(result)
                            }
                            Err(e) => Err(Error::Network(format!("{}", e)))
                        }
                    }
                    Err(e) => Err(Error::Network(format!("{}", e))),
                }
            }
            Ok(result)
        }

        async fn untis(client: &Client) -> Result<Week, Error> {
            unimplemented!()
        }
    }
}