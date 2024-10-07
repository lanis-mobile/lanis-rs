use std::time::SystemTime;
use reqwest::Client;
use scraper::{Html, Selector};
use crate::utils::constants::URL;

#[derive(Debug)]
pub struct Lessons {
    lessons: Vec<Lesson>,
}

#[derive(Debug)]
pub struct Lesson {
    pub id: String,
    pub url: String,
    pub name: String,
    pub teacher: String,
}

#[derive(Debug)]
pub struct LessonEntry {
    pub id: String,
    pub date: String,
    pub title: String,
    pub details: Option<String>,
    pub homework: Option<Homework>,
    pub attachment: Option<Vec<Attachment>>,
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
                                    })
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
