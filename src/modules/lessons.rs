use crate::base::account::Account;
use crate::utils::constants::URL;
use crate::utils::crypt::decrypt_encoded_tags;
use scraper::{Element, ElementRef, Html, Selector};
use std::collections::BTreeMap;
use std::time::SystemTime;
use markup5ever::interface::tree_builder::TreeSink;
use regex::Regex;
use reqwest::Client;
use reqwest::header::HeaderMap;
use reqwest::multipart::Part;

#[derive(Debug, Clone)]
pub struct Lessons {
    pub lessons: Vec<Lesson>,
}

#[derive(Debug, Clone)]
pub struct Lesson {
    pub id: i32,
    pub url: String,
    pub name: String,
    pub teacher: String,
    /// Should always be Some , if not something went wrong
    pub teacher_short: Option<String>,
    pub attendances: BTreeMap<String, String>,
    /// If this is None there is no latest entry
    pub entry_latest: Option<LessonEntry>,
    /// Will be Some(empty) if no exams are found and None if this value wasn't initialized
    pub entries: Option<Vec<LessonEntry>>,
    /// Will be empty if no exams are found and None if this value wasn't initialized
    pub marks: Option<Vec<LessonMark>>,
    /// Will be empty if no exams are found and None if this value wasn't initialized
    pub exams: Option<Vec<LessonExam>>,
}

#[derive(Debug, Clone)]
pub struct LessonEntry {
    pub id: i32,
    pub date: String,
    pub school_hours: Vec<i32>,
    pub title: String,
    pub details: Option<String>,
    pub homework: Option<Homework>,
    pub attachments: Option<Vec<Attachment>>,
    pub attachment_number: i32,
    pub uploads: Option<Vec<LessonUpload>>,
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
    pub id: i32,
    pub name: String,
    /// True if open and false if closed
    pub state: bool,
    pub url: String,
    pub uploaded: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UploadFileStatus {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LessonMark {
    pub name: String,
    pub date: String,
    pub mark: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LessonExam {
    pub date: String,
    pub name: String,
    pub finished: bool,
}

impl Lesson {
    /**
     *  Sets the data for a lesson. This data includes: <br>
     *  Entries history, marks and class tests
     */
    pub async fn set_data(&mut self, account: &Account) -> Result<(), String> {
        let client = &account.client;

        match client.get(format!("{}{}", URL::BASE, &self.url)).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    return Err(format!("Failed request with status code: {}", response.status()));
                }

                let document = decrypt_encoded_tags(response.text().await.unwrap().as_str(), &account.key_pair.public_key_string).await?;
                let document = Html::parse_document(&document);

                let mut history: Vec<LessonEntry> = vec![];

                let history_doc_selector = Selector::parse("#history").unwrap();
                let history_doc = document.select(&history_doc_selector);
                let history_doc = history_doc.clone().next().unwrap().html();
                let mut history_doc = Html::parse_document(&history_doc);

                let history_table_rows_selector = Selector::parse("table>tbody>tr").unwrap();

                let hidden_div_selector = Selector::parse(".hidden_encoded").unwrap();
                let hidden_div_ids: Vec<_> = history_doc.select(&hidden_div_selector).map(|x| x.id()).collect();

                // Remove encoded divs
                for id in hidden_div_ids {
                    history_doc.remove_from_parent(&id);
                }

                let history_table_rows = history_doc.select(&history_table_rows_selector);

                // Selectors for loop
                let title_selector = Selector::parse("big>b").unwrap();

                let details_selector = Selector::parse("span.markup i.fa-comment-alt").unwrap();

                let homework_selector = Selector::parse("span.homework + br + span.markup").unwrap();
                let homework_done_selector = Selector::parse("span.done.hidden").unwrap();

                let file_alert_selector = Selector::parse("div.alert.alert-info>a").unwrap();
                let files_selector = Selector::parse(".files").unwrap();

                let upload_group_selector = Selector::parse("div.btn-group").unwrap();
                let open_upload_selector = Selector::parse(".btn-warning").unwrap();
                let closed_upload_selector = Selector::parse(".btn-default").unwrap();
                let upload_url_selector = Selector::parse("ul.dropdown-menu li a").unwrap();
                let upload_badge_selector = Selector::parse("span.badge").unwrap();
                let upload_small_selector = Selector::parse("small").unwrap();

                for row in history_table_rows {
                    let id = row.attr("data-entry").unwrap().parse::<i32>().unwrap();

                    let title = {
                        row.child_elements().nth(1).unwrap().select(&title_selector).next().unwrap().text().next().unwrap().trim().to_string()
                    };

                    let details = {
                        let details = row.select(&details_selector).next();
                        if details.is_some() {
                            let details = details.unwrap();
                            let details = details.parent_element().unwrap().text().next().unwrap().trim().to_string();
                            Some(details)
                        } else {
                            None
                        }
                    };

                    let homework = {
                        let homework_element = row.select(&homework_selector).next();
                        let mut description: String = String::new();

                        if homework_element.is_some() {
                            for text in homework_element.unwrap().text() {
                                description += &*format!("{}\n", text.trim()).to_string();
                            }
                            description = description.rsplit_once('\n').unwrap().0.trim().to_string();
                        }

                        let completed = {
                            let element = row.select(&homework_done_selector).next();
                            !element.is_some()
                        };

                        if description.is_empty() {
                            None
                        } else {
                            Some(Homework{
                                description,
                                completed,
                            })
                        }
                    };

                    let attachments: Option<Vec<Attachment>> = {
                        if row.child_elements().nth(1).unwrap().select(&file_alert_selector).next().is_some() {
                            let mut attachments = vec![];
                            let url = format!("{}{}", URL::BASE, row.child_elements().nth(1).unwrap().select(&file_alert_selector).next().unwrap().value().attr("href").unwrap());
                            let url = url.replace("&b=zip", "").to_string();

                            for element in row.select(&files_selector).nth(0).unwrap().child_elements() {
                                let name = element.attr("data-file").unwrap().to_string();
                                let url = format!("{}&f={}", url, name);
                                attachments.push(Attachment{
                                    name,
                                    url,
                                });
                            }
                            Some(attachments)
                        } else {
                            None
                        }
                    };

                    let uploads: Option<Vec<LessonUpload>> = {
                        let upload_groups = row.child_elements().nth(1).unwrap().select(&upload_group_selector);
                        let mut uploads: Vec<LessonUpload> = vec![];

                        for group in upload_groups {
                            let open = group.select(&open_upload_selector).next();
                            let closed = group.select(&closed_upload_selector).next();

                            if open.is_some() {
                                let open = open.unwrap();

                                let name = open.children().nth(2).unwrap().value().as_text().unwrap().to_string();
                                let state = true;
                                let url = format!("{}{}", URL::BASE, group.select(&upload_url_selector).next().unwrap().value().attr("href").unwrap());
                                let uploaded = {
                                    match open.select(&upload_badge_selector).next() {
                                        Some(element) => Some(element.text().collect::<String>().trim().to_string()),
                                        None => None,
                                    }
                                };
                                let date = {
                                    let text = open.select(&upload_small_selector).next().unwrap().text().collect::<String>().trim().to_string();
                                    let text = text.replace("\n", "").trim().to_string();
                                    let text = text.replace("                                                                ", "").trim().to_string();
                                    let text = text.replace("bis ", "").trim().to_string();
                                    let text = text.replace("um", "").trim().to_string();

                                    text
                                };
                                let id = url.split("&id=").last().unwrap().parse::<i32>().unwrap();

                                uploads.push(LessonUpload{
                                    id,
                                    name,
                                    state,
                                    url,
                                    uploaded: {
                                        if uploaded.is_some() {
                                            Some(uploaded.unwrap())
                                        } else {
                                            None
                                        }
                                    },
                                    date: Some(date),
                                });
                            } else if closed.is_some() {
                                let closed = closed.unwrap();

                                let name = closed.children().nth(2).unwrap().value().as_text().unwrap().trim().to_string();
                                let state = false;
                                let url = format!("{}{}", URL::BASE, group.select(&upload_url_selector).next().unwrap().value().attr("href").unwrap());
                                let uploaded = {
                                    match closed.select(&upload_badge_selector).next() {
                                        Some(element) => Some(element.text().collect::<String>().trim().to_string()),
                                        None => None,
                                    }
                                };
                                let id = url.split("&id=").last().unwrap().parse::<i32>().unwrap();

                                uploads.push(LessonUpload{
                                    id,
                                    name,
                                    state,
                                    url,
                                    uploaded: {
                                        if uploaded.is_some() {
                                            Some(uploaded.unwrap())
                                        } else {
                                            None
                                        }
                                    },
                                    date: None,
                                })
                            }
                        }

                        if uploads.is_empty() {
                            None
                        } else {
                            Some(uploads)
                        }
                    };

                    let date = row.child_elements().nth(0).unwrap().text().collect::<String>().split("\n").nth(0).unwrap().trim().to_string();
                    let school_hours = {
                        let mut school_hours = vec![];

                        let string = row.child_elements().nth(0).unwrap().text().collect::<String>().split("\n").nth(2).unwrap().trim()
                            .replace(". ", "")
                            .replace("Stunde", "")
                            .replace("-", "")
                            .trim()
                            .to_string();

                        for hour in string.split(' ') {
                            school_hours.push(hour.parse::<i32>().unwrap_or_default())
                        }

                        school_hours
                    };

                    history.push(LessonEntry{
                        id,
                        date,
                        school_hours,
                        title,
                        details,
                        homework: {
                            if homework.is_some() {
                                Some(homework.unwrap())
                            } else {
                                None
                            }
                        },
                        attachments: {
                            if attachments.is_some() {
                                Some(attachments.clone().unwrap())
                            } else {
                                None
                            }
                        },
                        attachment_number: {
                            if attachments.is_some() {
                                attachments.unwrap().len() as i32
                            } else {
                                0
                            }
                        },
                        uploads: {
                            if uploads.is_some() {
                                Some(uploads.unwrap())
                            } else {
                                None
                            }
                        }
                    })
                }
                self.entries = Some(history);

                // Marks
                // TODO: Test parsing of existing marks
                let marks_section_selector = Selector::parse("#marks").unwrap();
                let mut marks_doc = Html::parse_document(&document.select(&marks_section_selector).nth(0).unwrap().html());

                let encoded_elements: Vec<_> = marks_doc.select(&hidden_div_selector).map(|x| x.id()).collect();
                for id in encoded_elements {
                    marks_doc.remove_from_parent(&id)
                }

                let marks_table_rows_selector = Selector::parse("table>tbody>tr").unwrap();
                let marks_table_rows = marks_doc.select(&marks_table_rows_selector);

                let mut marks = vec![];

                for row in marks_table_rows {
                    if row.child_elements().count() == 3 {
                        let name = row.child_elements().nth(0).unwrap().text().collect::<String>().trim().to_string();
                        let date = row.child_elements().nth(1).unwrap().text().collect::<String>().trim().to_string();
                        let mark = row.child_elements().nth(2).unwrap().text().collect::<String>().trim().to_string();
                        let comment = row.child_elements().nth(1).unwrap().text().collect::<String>().trim().split(":").nth(1).unwrap().trim().to_string();
                        marks.push(LessonMark{
                            name,
                            date,
                            mark,
                            comment: {
                                if comment.is_empty() {
                                    None
                                } else {
                                    Some(comment)
                                }
                            }
                        });
                    }
                }
                self.marks = Some(marks);

                // Exams
                let exam_section_selector = Selector::parse("#klausuren").unwrap();
                let exam_section = document.select(&exam_section_selector).nth(0).unwrap();
                let ul_selector = Selector::parse("ul").unwrap();
                let li_selector = Selector::parse("li").unwrap();
                let title_selector = Selector::parse("h2").unwrap();

                let mut exams= vec![];

                if !exam_section.child_elements().nth(0).unwrap().html().contains("Diese Kursmappe beinhaltet leider noch keine Leistungskontrollen!") {
                    for element in exam_section.child_elements() {
                        let elements = element.select(&ul_selector);
                        for element in elements {
                            let sibling_html = Html::parse_document(&element.prev_sibling_element().unwrap().html());
                            let title = sibling_html.select(&title_selector).nth(0).unwrap().text().collect::<String>().trim().to_string();
                            let re = Regex::new(r"\s+\n").unwrap();

                            let li_elements = element.select(&li_selector);
                            for element in li_elements {
                                let exam = {
                                    let text = element.text().collect::<String>().trim().to_string();
                                    let mut result = re.replace_all(text.as_str(), "").trim().to_string();
                                    let mut trimming = true;
                                    while trimming {
                                        let previous = result.clone();
                                        result = result.replace("  ", " ").trim().to_string();
                                        if result == previous {
                                            trimming = false;
                                        }
                                    }
                                    result = result.replace("\n", "").trim().to_string();
                                    result
                                };
                                let split = exam.split(" ");
                                let date = split.clone().nth(0).unwrap().trim().to_string();
                                let name = {
                                    let mut result = "".to_string();
                                    for i in 1..split.clone().count() {
                                        result = format!("{} {}", result, split.clone().nth(i).unwrap());
                                    }
                                    result.trim().to_string()
                                };

                                exams.push(LessonExam{
                                    date,
                                    name,
                                    finished: {
                                        if title == "Alle Leistungskontrolle(n)" {
                                            true
                                        } else {
                                            false
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                self.exams = Some(exams);

                Ok(())
            }
            Err(error) => {
                Err(format!("Failed to get '{}{}' with error: {}", URL::BASE, &self.url, error))
            }
        }
    }
}

impl Homework {
    pub async fn set_homework(&mut self, state: bool, course_id: i32, entry_id: i32, client: &Client) -> Result<(), String> {
        match client.post(URL::MEIN_UNTERRICHT)
            .header("X-Requested-With", "XMLHttpRequest")
            .form(&[("a", "sus_homeworkDone"), ("entry", entry_id.to_string().as_str()), ("id", course_id.to_string().as_str()), ("b", { if state { "done" } else { "undone" } })])
            .send().await {
            Ok(response) => {
                let text = response.text().await.unwrap();
                if text == "1" {
                    self.completed = state;
                    Ok(())
                } else {
                    Err(format!("Failed to set homework! Got instead of '1' '{}' as response", text))
                }
            } Err(e) => {
                Err(format!("Failed to set homework with error: {}", e))
            }
        }
    }
}

impl LessonUpload {
    // TODO: Test upload function
    pub async fn upload(&mut self, files: Vec<String>, course_id: i32, entry_id: i32, client: &Client) -> Result<Vec<UploadFileStatus>, String> {
        if files.is_empty() {
            return Err("Please specify a file path to upload!".to_string())
        }

        let form = reqwest::multipart::Form::new()
            .part("a", Part::text("sus_abgabe"))
            .part("b", Part::text(course_id.to_string()))
            .part("e", Part::text(entry_id.to_string()))
            .part("id", Part::text(self.id.to_string()))
            .part("file1", {
                match files.get(0) {
                    Some(path) => Part::file(path).await.unwrap(),
                    None => Part::text("")
                }
            })
            .part("file2", {
                match files.get(1) {
                    Some(path) => Part::file(path).await.unwrap(),
                    None => Part::text("")
                }
            })
            .part("file3", {
                match files.get(2) {
                    Some(path) => Part::file(path).await.unwrap(),
                    None => Part::text("")
                }
            })
            .part("file4", {
                match files.get(3) {
                    Some(path) => Part::file(path).await.unwrap(),
                    None => Part::text("")
                }
            })
            .part("file5", {
                match files.get(4) {
                    Some(path) => Part::file(path).await.unwrap(),
                    None => Part::text("")
                }
            });

        let mut headers = HeaderMap::new();
        headers.insert("Accept", "*/*".parse().unwrap());
        headers.insert("Content-Type", "multipart/form-data".parse().unwrap());

       match client.post(URL::MEIN_UNTERRICHT).multipart(form).headers(headers).send().await {
           Ok(response) => {
               let text = response.text().await.unwrap();
               let document = Html::parse_document(&text);

               let status_message_group_selector = Selector::parse("div#content div.col-md-12").unwrap();
               let status_message_group = document.select(&status_message_group_selector).nth(2).unwrap();

               let ul_ui_selector = Selector::parse("ul li").unwrap();
               let b_selector = Selector::parse("b").unwrap();
               let span_label_selector = Selector::parse("span.label").unwrap();

               let mut status_messages = vec![];
               for status_message in status_message_group.select(&ul_ui_selector) {
                   let name = status_message.select(&b_selector).nth(0).unwrap().text().collect::<String>().trim().to_string();
                   let status = status_message.select(&span_label_selector).nth(0).unwrap().text().collect::<String>().trim().to_string();
                   let message = {
                       match status_message.children().nth(4) {
                           Some(message) => {
                               match message.value().as_text() {
                                   Some(text) => {
                                       let result = text.trim().to_string();
                                       Some(result)
                                   }
                                   None => None
                               }
                           },
                           None => None,
                       }
                   };

                   status_messages.push(UploadFileStatus {
                       name,
                       status,
                       message,
                   })
               }
               Ok(status_messages)
           }
           Err(e) => {
               Err(format!("Failed to upload file with error: '{}'", e))
           }
       }
    }
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
                                    let id = url.split("id=").nth(1).unwrap().to_string().parse::<i32>().unwrap();
                                    let name = lesson.select(&h2_selector).next().unwrap().text().collect::<String>().trim().to_string();
                                    let teacher: String = lesson.select(&button_selector).next().and_then(|btn| btn.value().attr("title")).map(|s| s.to_string()).unwrap();
                                    let teacher: String = teacher.split(" (").next().unwrap().to_string();
                                    lessons.lessons.push(Lesson {
                                        id,
                                        url,
                                        name,
                                        teacher,
                                        teacher_short: None,
                                        attendances: BTreeMap::new(),
                                        entry_latest: None,
                                        entries: None,
                                        marks: None,
                                        exams: None,
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

                                let file_count_selector = Selector::parse(".file").unwrap();
                                let file_count: i32 = school_class.select(&file_count_selector).count() as i32;


                                let entry_id = school_class.value().attr("data-entry").unwrap_or("").parse::<i32>().unwrap();

                                let homework_selector = Selector::parse(".homework").unwrap();
                                let homework = school_class.select(&homework_selector).next().map(|_| {
                                    let description_selector = Selector::parse(".realHomework").unwrap();
                                    let description = school_class.select(&description_selector).next().unwrap().text().collect::<String>().trim().to_string();
                                    let completed = school_class.select(&Selector::parse(".undone").unwrap()).next().is_none();
                                    Homework { description, completed }
                                });

                                for lesson in lessons.lessons.iter_mut() {
                                    if lesson.url == course_url.to_owned() {
                                        lesson.entry_latest = Option::from(LessonEntry {
                                            id: entry_id.to_owned(),
                                            date: topic_date.to_owned(),
                                            school_hours: vec![-1],
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
                                    let value = text_elements.get(i).unwrap_or(&"".to_string()).clone();

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
                                        if course_url.contains(&lesson.id.to_string()) {
                                            lesson.attendances = attendances;
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

