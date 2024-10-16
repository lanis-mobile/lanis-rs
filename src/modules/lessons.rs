use std::collections::{BTreeMap};
use crate::utils::constants::URL;
use scraper::{ElementRef, Html, Selector};
use std::time::SystemTime;
use crate::base::account::Account;
use crate::utils::crypt::{decrypt_encoded_tags};

#[derive(Debug, Clone)]
pub struct Lessons {
    pub lessons: Vec<Lesson>,
}

#[derive(Debug, Clone)]
pub struct Lesson {
    pub id: String,
    pub url: String,
    pub name: String,
    pub teacher: String,
    pub teacher_short: Option<String>,
    pub attendances: Option<BTreeMap<String, String>>,
    pub entry_latest: Option<LessonEntry>,
    pub entries: Option<Vec<LessonEntry>>,
}

#[derive(Debug, Clone)]
pub struct LessonEntry {
    pub id: String,
    pub date: String,
    pub title: String,
    pub details: Option<String>,
    pub homework: Option<Homework>,
    pub attachments: Option<Vec<Attachment>>,
    pub attachment_number: i32,
    pub uploads: Option<LessonUpload>,
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct Homework {
    pub description: String,
    pub completed: bool,

}

#[derive(Debug, Clone)]
pub struct LessonUpload {
    pub url: String,
}

pub async fn get_lessons(account: &Account) -> Result<Lessons, String> {
    let client = &account.client;
    let unix_time = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_millis();
    match client.get(URL::BASE.to_owned() + &format!("meinunterricht.php?cacheBreaker={}", unix_time)).send().await {
        Ok(response) => {
            match response.text().await {
                Ok(response) => {
                    let response = decrypt_encoded_tags(&response, &account.key_pair.public_key_string).await?;
                    let document = Html::parse_document(&response);
                    let lesson_folders_selector = Selector::parse("#mappen").unwrap();
                    let row_selector = Selector::parse(".row").unwrap();
                    let h2_selector = Selector::parse("h2").unwrap();
                    let button_selector = Selector::parse("div.btn-group > button").unwrap();
                    let link_selector = Selector::parse("a.btn.btn-primary").unwrap();

                    if let Some(lesson_folders) = document.select(&lesson_folders_selector).next() {
                        if let Some(row) = lesson_folders.select(&row_selector).next() {
                            let mut lessons = Lessons { lessons: Vec::new() };
                            for lesson in row.child_elements() {
                                if let Some(url_element) = lesson.select(&link_selector).next() {
                                    let url = url_element.value().attr("href").unwrap().to_string();
                                    let id = url.split("id=").nth(1).unwrap().to_string();
                                    let name = lesson.select(&h2_selector).next().unwrap().text().collect::<String>().trim().to_string();
                                    let teacher: String = lesson.select(&button_selector).next().and_then(|btn| btn.value().attr("title")).map(|s| s.to_string()).unwrap();
                                    let teacher: String = teacher.split(" (").next().unwrap().to_string();
                                    lessons.lessons.push(Lesson {
                                        id,
                                        url,
                                        name,
                                        teacher,
                                        teacher_short: None,
                                        attendances: Some(BTreeMap::new()),
                                        entry_latest: None,
                                        entries: None,
                                    })
                                }
                            }

                            // Get latest lesson entry
                            let school_classes_selector = Selector::parse("tr.printable").unwrap();
                            let school_classes = document.select(&school_classes_selector);
                            for school_class in school_classes {
                                fn collect_text(element_ref: Option<ElementRef>) -> Result<String, ()> {
                                    match element_ref {
                                        Some(element_ref) => {
                                            let s = element_ref.text().collect::<String>().trim().to_string();
                                            Ok(s)
                                        }
                                        None => Err(())
                                    }
                                }
                                let topic_title_selector = Selector::parse(".thema").unwrap();
                                let topic_title = collect_text(school_class.select(&topic_title_selector).next()).unwrap_or("".to_string());

                                let teacher_short_selector = Selector::parse(".teacher .btn.btn-primary.dropdown-toggle.btn-xs").unwrap();
                                let teacher_short = collect_text(school_class.select(&teacher_short_selector).next()).unwrap_or("".to_string());

                                let topic_date_selector = Selector::parse(".datum").unwrap();
                                let topic_date = collect_text(school_class.select(&topic_date_selector).next()).unwrap_or("".to_string());

                                let course_url_selector = Selector::parse("td>h3>a").unwrap();
                                let course_url = school_class.select(&course_url_selector).next().map(|x| x.value().attr("href").unwrap().to_string().trim().to_string()).unwrap_or("".to_string());

                                let file_count_selector = Selector::parse("file").unwrap();
                                let file_count: i32 = school_class.select(&file_count_selector).count() as i32;

                                let homework_selector = Selector::parse(".homework").unwrap();
                                let homework = school_class.select(&homework_selector).next().map(|_| {
                                    let description_selector = Selector::parse(".realHomework").unwrap();
                                    let description = school_class.select(&description_selector).next().unwrap().text().collect::<String>().trim().to_string();
                                    let completed = school_class.select(&Selector::parse(".undone").unwrap()).next().is_none();
                                    Homework { description, completed }
                                });

                                let entry_id = school_class.value().attr("data-entry").unwrap_or("").to_string();

                                for lesson in lessons.lessons.iter_mut() {
                                    if lesson.url == course_url.to_owned() {
                                        lesson.entry_latest = Option::from(LessonEntry {
                                            id: entry_id.to_owned(),
                                            date: topic_date.to_owned(),
                                            title: topic_title.to_owned(),
                                            details: None,
                                            homework: homework.clone(),
                                            attachments: None,
                                            attachment_number: file_count,
                                            uploads: None,
                                        });
                                        lesson.teacher_short = Some(teacher_short.to_owned());
                                    }
                                }
                            }

                            let attendance_selector = Selector::parse("#anwesend").unwrap();
                            let thead_selector = Selector::parse("thead > tr").unwrap();
                            let tbody_selector = Selector::parse("tbody > tr").unwrap();
                            let link_selector = Selector::parse("a").unwrap();

                            let attendance_element = document.select(&attendance_selector).next().unwrap();
                            let thead_element = attendance_element.select(&thead_selector).next().unwrap();

                            let keys: Vec<String> = thead_element.select(&Selector::parse("th").unwrap()).map(|el| el.text().collect::<String>().trim().to_string()).collect();

                            for row in attendance_element.select(&tbody_selector) {
                                let mut text_elements: Vec<String> = vec![];
                                let mut attendances: BTreeMap<String, String> = BTreeMap::new();


                                for element in row.child_elements() {
                                    if let Some(attr) = element.attr("class") {
                                        if attr.contains("hidden") && attr.contains("hidden_encoded") {
                                            continue
                                        }
                                    }
                                    text_elements.push(element.text().collect::<String>().trim().to_string());
                                }

                                for (i, key) in keys.iter().enumerate() {
                                    let key_lower = key.to_lowercase();
                                    let mut value = text_elements.get(i).unwrap_or(&"".to_string()).clone();

                                    if ["kurs", "lehrkraft"].contains(&key_lower.as_str()) {
                                        continue;
                                    }

                                    let mut value = value.lines().skip(1).next().unwrap_or("").trim().to_string();

                                    if value.is_empty() {
                                        value = "0".to_string();
                                    }

                                    attendances.insert(key_lower, value);
                                }

                                if let Some(hyperlink) = row.select(&link_selector).next() {
                                    let course_url = hyperlink.value().attr("href").unwrap_or("");
                                    for lesson in &mut lessons.lessons {
                                        if course_url.contains(&lesson.id) {
                                            lesson.attendances = Some(attendances);
                                            break;
                                        }
                                    }
                                }
                            }

                            Ok(lessons)
                        } else {
                            Err("Failed to select rows from lesson folders".to_string())
                        }
                    } else {
                        Err("Failed to select lesson folders".to_string())
                    }
                }
                Err(e) => {
                    Err(format!("Failed converting response into text: {}", e))
                }
            }
        }
        Err(e) => {
            Err(format!("Failed to fetch lessons from '{}?cacheBreaker={}':\n{}", URL::BASE, unix_time, e))
        }
    }
}

