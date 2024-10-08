use std::time::SystemTime;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use crate::utils::constants::URL;

#[derive(Debug)]
pub struct Lessons {
    pub lessons: Vec<Lesson>,
}

#[derive(Debug)]
pub struct Lesson {
    pub id: String,
    pub url: String,
    pub name: String,
    pub teacher: String,
    pub entry_latest: Option<LessonEntry>,
    pub entries: Option<Vec<LessonEntry>>
}

#[derive(Debug)]
pub struct LessonEntry {
    pub id: String,
    pub date: String,
    pub title: String,
    pub details: Option<String>,
    pub homework: Option<Homework>,
    pub attachments: Option<Vec<Attachment>>,
    pub attachment_number: i32,
    pub uploads: Option<LessonUpload>
}

#[derive(Debug)]
pub struct Attachment {
    pub name: String,
    pub url: String,
}

#[derive(Debug)]
pub struct Homework {
    pub description: String,
    pub completed: bool,

}

#[derive(Debug)]
pub struct LessonUpload {
    pub url: String,
}

pub async fn get_lessons(client: &Client) -> Result<Lessons, String> {
    let unix_time = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_millis();
    match client.get(URL::BASE.to_owned() + &format!("meinunterricht.php?cacheBreaker={}", unix_time)).send().await {
        Ok(response) => {
            match response.text().await {
                Ok(response) => {
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
                                    lessons.lessons.push(Lesson{
                                        id,
                                        url,
                                        name,
                                        teacher,
                                        entry_latest: None,
                                        entries: None,
                                    })
                                }
                            }

                            // Get latest lesson entry
                            let school_classes_selector = Selector::parse("tr.printable").unwrap();
                            let school_classes = document.select(&school_classes_selector);
                            for school_class in school_classes {
                                let teacher_selector = Selector::parse(".teacher").unwrap();
                                let teacher = school_class.select(&teacher_selector).next();

                                if let Some(date) = school_class.select(&Selector::parse(".datum").unwrap()).next() {
                                    fn collect_text(element_ref: Option<ElementRef>) -> Result<String, ()> {
                                        match element_ref {
                                            Some(element_ref) => {
                                                match element_ref.text().collect::<String>().trim().to_string() {
                                                    Ok(s) => Ok(s),
                                                    Err(_) => Err(())
                                                }
                                            }
                                            None => Err(())
                                        }
                                    }
                                    let topic_title_selector = Selector::parse(".thema").unwrap();
                                    let topic_title = collect_text(school_class.select(&topic_title_selector).next()).unwrap_or("".to_string());

                                    let teacher_short_selector =  Selector::parse(".teacher .btn.btn-primary.dropdown-toggle.btn-xs").unwrap();
                                    let teacher_short = collect_text(school_class.select(&teacher_short_selector).next()).unwrap_or("".to_string());

                                    let teacher_name_selector = Selector::parse(".teacher ul>li>a>i.fa").unwrap();
                                    let teacher_name = collect_text(school_class.select(&teacher_name_selector).next()).unwrap_or("".to_string());

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
                                            lesson.entry_latest = Option::from(LessonEntry{
                                                id: entry_id.to_owned(),
                                                date: topic_date.to_owned(),
                                                title: topic_title.to_owned(),
                                                details: None,
                                                homework: homework.to_owned(),
                                                attachments: None,
                                                attachment_number: file_count,
                                                uploads: None,
                                            })
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

