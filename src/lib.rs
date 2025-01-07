use serde::{Deserialize, Serialize};

pub mod base;
pub mod utils;
pub mod modules;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum Feature {
    LanisTimetable,
    MeinUnttericht,
    FileStorage,
    MessagesBeta,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::base::schools::{get_school_id, get_schools, School};
    use crate::modules::lessons::{get_lessons};
    use crate::base::account::{Account, AccountSecrets, UntisSecrets};
    use crate::modules::timetable;
    use crate::modules::timetable::{Provider, Week};

    use std::{env, fs};
    use std::path::Path;
    use stopwatch_rs::StopWatch;
    use crate::modules::file_storage::FileStoragePage;
    use crate::modules::messages::ConversationOverview;
    use crate::utils::crypt::{decrypt_any, encrypt_any};

    #[tokio::test]
    async fn test_encryption() {
        let text = fs::read_to_string("test_file.txt").unwrap();

        #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
        struct TestText {
            text: String,
        }

        let data = TestText { text };
        let key = b"ILikeToast12!EncryptionIsSoNice!";

        let encrypted = encrypt_any(&data, key).await.unwrap();
        let decrypted: TestText = decrypt_any(&encrypted, key).await.unwrap();

        assert_eq!(data, decrypted);
    }

    #[tokio::test]
    async fn test_schools_get_school_id() {
        let mut schools: Vec<School> = vec![];
        schools.push(School{
            id: 3120,
            name: String::from("The Almighty Rust School"),
            city: String::from("Rust City")
        });
        schools.push(School{
            id: 3920,
            name: String::from("The Almighty Rust School"),
            city: String::from("Rust City 2")
        });
        schools.push(School{
            id: 4031,
            name: String::from("The Almighty Rust School 2"),
            city: String::from("Rust City")
        });
        let result = get_school_id("The Almighty Rust School", "Rust City 2", &schools).await;
        assert_eq!(result, 3920);
    }

    #[tokio::test]
    async fn test_schools_get_schools() {
        let client = reqwest::Client::new();

        let result = get_schools(&client).await.unwrap();
        assert_eq!(result.get(0).unwrap().id, 3354);
    }

    async fn create_account() -> Account {
        let mut stopwatch = StopWatch::start();

        let account_secrets = AccountSecrets::new(
            {
                env::var("LANIS_SCHOOL_ID").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_SCHOOL_ID' in env?", e);
                    String::from("0")
                }).parse().expect("Couldn't parse 'LANIS_SCHOOL_ID'.\nDid you define SCHOOL_ID as an i32?")
            },
            {
                env::var("LANIS_USERNAME").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_USERNAME' in env?", e);
                    String::from("")
                })
            },
            {
                env::var("LANIS_PASSWORD").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_PASSWORD' in env?", e);
                    String::from("")
                })
            },
        );
        let account = Account::new(account_secrets).await.unwrap();
        println!("account::new() took {}ms", stopwatch.split().split.as_millis());

        account
    }

    #[tokio::test]
    async fn test_student_account() {
        let account = create_account().await;

        let mut stopwatch = StopWatch::start();
        account.prevent_logout().await.unwrap();
        println!("account.prevent_logout() took {}ms", stopwatch.split().split.as_millis());
        println!();

        println!("Private Key:\n{}", account.key_pair.private_key_string);
        println!("Public Key:\n{}", account.key_pair.public_key_string);

        assert_eq!(account.data.is_empty(), false);

        println!()
    }

    #[tokio::test]
    async fn test_timetable() {
        let mut account = create_account().await;

        if account.is_supported(Feature::LanisTimetable) {
            // Lanis (All)
            let mut stopwatch = StopWatch::start();
            let time_table_week = Week::new(Provider::Lanis(timetable::LanisType::All), &account.client, chrono::Local::now().date_naive()).await.unwrap();
            let ms = stopwatch.split().split.as_millis();
            println!("Lanis All: {:?}", time_table_week);
            println!("Week::new() took {}ms", ms);
            println!();

            // Lanis (Own)
            let mut stopwatch = StopWatch::start();
            let time_table_week = Week::new(Provider::Lanis(timetable::LanisType::Own), &account.client, chrono::Local::now().date_naive()).await.unwrap();
            let ms = stopwatch.split().split.as_millis();
            println!("Lanis Own: {:?}", time_table_week);
            println!("Week::new() took {}ms", ms);
            println!();
        } else {
            println!("LanisTimetable is not supported by this account! Skipping.");
        }

        // Untis
        if env::var("UNTIS_TEST_TIMETABLE").unwrap_or("FALSE".to_string()).eq("TRUE") {
            let mut stopwatch = StopWatch::start();
            let school_name = env::var("UNTIS_SCHOOL_NAME").expect("Couldn't find 'UNTIS_SCHOOL_NAME' in env! Did you set it?");
            let username = env::var("UNTIS_USERNAME").expect("Couldn't find 'UNTIS_USERNAME' in env! Did you set it?");
            let password = env::var("UNTIS_PASSWORD").expect("Couldn't find 'UNTIS_PASSWORD' in env! Did you set it?");

            let secrets = UntisSecrets::new(school_name, username, password);
            account.secrets.untis_secrets = Some(secrets);

            let time_table_week = Week::new(Provider::Untis(account.secrets.untis_secrets.as_ref().unwrap().clone()), &account.client, chrono::Local::now().date_naive() - chrono::Duration::weeks(1)).await.unwrap();
            let ms = stopwatch.split().split.as_millis();
            println!("Untis: {:?}", time_table_week);
            println!("Week::new() took {}ms", ms);
        }

        println!();
    }

    #[tokio::test]
    async fn test_lessons() {
        let account = create_account().await;

        if account.is_supported(Feature::MeinUnttericht) {
            let mut stopwatch = StopWatch::start();
            let mut lessons = get_lessons(&account).await.unwrap();
            println!("get_lessons() took {}ms", stopwatch.split().split.as_millis());

            let mut stopwatch = StopWatch::start();
            for lesson in lessons.lessons.iter_mut() {
                println!("\tid: {}", lesson.id);
                println!("\turl: {}", lesson.url);
                println!("\tname: {}", lesson.name);
                println!("\tteacher: {}", lesson.teacher);
                println!("\tteacher_short: {:?}", lesson.teacher_short);
                println!("\tattendances: {:?}", lesson.attendances);
                println!("\tentry_latest: {:?}", lesson.entry_latest);
                let mut stopwatch = StopWatch::start();
                lesson.set_data(&account).await.unwrap();
                println!("\tlesson.set_data() took {}ms", stopwatch.split().split.as_millis());
                println!("\tmarks: {:?}", lesson.marks);
                println!("\tentries:");
                let mut stopwatch = StopWatch::start();
                for mut entry in lesson.entries.clone().unwrap() {
                    println!("\t\t{:?}", entry);
                    if entry.homework.is_some() {
                        let mut homework = entry.homework.clone().unwrap();
                        let mut new_homework = !homework.completed;

                        let mut stopwatch = StopWatch::start();
                        homework.set_homework(new_homework, lesson.id, entry.id, &account.client).await.unwrap();
                        println!("\t\t\tHomework was changed from {} to {} and took {}ms", !homework.completed, new_homework, stopwatch.split().split.as_millis());
                        entry.homework = Some(homework.to_owned());
                        println!("\t\t\tHomework after change: {:?}", entry.homework);

                        new_homework = !new_homework;

                        let mut stopwatch = StopWatch::start();
                        homework.set_homework(new_homework, lesson.id, entry.id, &account.client).await.unwrap();
                        println!("\t\t\tHomework was changed from {} to {} and took {}", !homework.completed, new_homework, stopwatch.split().split.as_millis());
                        entry.homework = Some(homework);
                        println!("\t\t\tHomework after change: {:?}", entry.homework);
                    }
                    if entry.uploads.is_some() {
                        let mut uploads = entry.uploads.clone().unwrap();
                        for upload in &mut uploads {
                            let mut stopwatch = StopWatch::start();
                            upload.info = Some(upload.get_info(&account.client).await.unwrap());
                            println!("\t\t\tupload.get_info() took {}ms", stopwatch.split().split.as_millis());
                            println!("\t\t\tUpload: {:?}", upload);
                            if upload.state {
                                let mut stopwatch = StopWatch::start();
                                let path = env::var("LANIS_TEST_FILE").unwrap_or_else(|e| { panic!("Error ({})\nDid you define 'LANIS_TEST_FILE' in env?", e)});
                                let path = Path::new(&path);
                                let status = upload.upload(vec![path], &account.client).await.unwrap();
                                let ms = stopwatch.split().split.as_millis();
                                println!("\t\t\tUploaded test file: {}", upload.url);
                                println!("\t\t\t\tUrl: {}", upload.url);
                                println!("\t\t\t\tStatus: {:?}", status);
                                println!("\t\t\tupload.upload() took {}ms", ms);

                                let i = {
                                    upload.info = Some(upload.get_info(&account.client).await.unwrap());
                                    let own_files = upload.info.clone().unwrap().own_files;
                                    let mut i = -1;
                                    for file in own_files {
                                        if file.name == status.get(0).unwrap().name {
                                            i = file.index;
                                        }
                                    }

                                    i
                                };

                                // Delete uploaded file
                                let mut stopwatch = StopWatch::start();
                                if i != -1 {
                                    upload.delete(&i, &account).await.unwrap();
                                }
                                println!("\t\t\tupload.delete() took {}ms", stopwatch.split().split.as_millis());
                            }
                        }
                    }
                }
                println!("\tIteration of all entries took {}ms", stopwatch.split().split.as_millis());
                println!("\texams:");
                for exam in lesson.exams.clone().unwrap() {
                    println!("\t\t{:?}", exam)
                }

                println!(" ");
            }
            println!("Iteration of all lessons took {}ms", stopwatch.split().split.as_millis());

            println!()
        } else {
            println!("Lessons are not supported by this account! Skipping.");
        }
    }

    #[tokio::test]
    async fn test_file_storage() {
        let account = create_account().await;

        if !account.is_supported(Feature::FileStorage) {
            println!("File Storage is not supported by this account! Skipping.");
            return;
        }

        print!("Getting root page... ");
        let mut stopwatch = StopWatch::start();
        let root_page = FileStoragePage::get_root(&account.client).await.unwrap();
        let ms = stopwatch.split().split.as_millis();
        println!("Took {} ms", ms);
        println!("Root page:\n{:#?}", root_page);
        println!();

        if let Some(node) = root_page.folder_nodes.get(0) {
            print!("Getting folder node page... ");
            let mut stopwatch = StopWatch::start();
            let first_page = FileStoragePage::get(node.id, &account.client).await.unwrap();
            let ms = stopwatch.split().split.as_millis();
            println!("Took {} ms", ms);
            println!("First page:\n{:#?}", first_page);
            println!();

            if let Some(node) = first_page.file_nodes.get(0) {
                let path = format!("/tmp/{}", node.name);
                print!("Downloading first file node to '{}'... ", path);
                let mut stopwatch = StopWatch::start();

                node.download(&path, &account.client).await.unwrap();

                let ms = stopwatch.split().split.as_millis();
                println!("Took {}ms", ms);

                print!("Deleting '{}'... ", path);
                let mut stopwatch = StopWatch::start();

                tokio::fs::remove_file(path).await.unwrap();

                let ms = stopwatch.split().split.as_millis();
                println!("Took {}ms", ms);
            }
        }

        println!();
    }

    #[tokio::test]
    async fn test_messages() {
        let account = create_account().await;

        print!("Getting root page of conversations... ");
        let mut stopwatch = StopWatch::start();
        let overviews = ConversationOverview::get_root(&account.client, &account.key_pair).await.unwrap();
        let ms = stopwatch.split().split.as_millis();
        println!("Took {}ms", ms);
        println!("Conversation overviews: {:#?}", overviews);

        for mut overview in overviews {
            println!("Current overview: {}", overview.subject);
            if overview.visible {
                println!("\tBefore: {}", overview.visible);
                print!("\tHiding conversation overview... ");
                let mut stopwatch = StopWatch::start();
                let result =  overview.hide(&account.client).await.unwrap();
                let ms = stopwatch.split().split.as_millis();
                println!("Took {}ms", ms);
                println!("\tResult: {}", result);

                println!("\tNow: {}", overview.visible);

                print!("\tShowing conversation overview... ");
                let mut stopwatch = StopWatch::start();
                let result =  overview.show(&account.client).await.unwrap();
                let ms = stopwatch.split().split.as_millis();
                println!("Took {}ms", ms);
                println!("\tResult: {}", result);
                println!("\tAfter: {}", overview.visible);
            } else {
                println!("\tBefore: {}", overview.visible);
                print!("\tShowing conversation overview... ");
                let mut stopwatch = StopWatch::start();
                let result =  overview.show(&account.client).await.unwrap();
                let ms = stopwatch.split().split.as_millis();
                println!("Took {}ms", ms);
                println!("\tResult: {}", result);

                println!("\tNow: {}", overview.visible);

                print!("\tHiding conversation overview... ");
                let mut stopwatch = StopWatch::start();
                let result =  overview.hide(&account.client).await.unwrap();
                let ms = stopwatch.split().split.as_millis();
                println!("Took {}ms", ms);
                println!("\tResult: {}", result);
                println!("\tAfter: {}", overview.visible);
            }
            println!();

            print!("\tGetting full conversation... ");
            let mut stopwatch = StopWatch::start();
            let conversation = overview.get(&account.client, &account.key_pair).await.unwrap();
            let ms = stopwatch.split().split.as_millis();
            println!("Took {}ms", ms);
            println!("{:#?}", conversation);
        }

        println!()
    }
}
