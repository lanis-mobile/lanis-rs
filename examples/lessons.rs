//! This example shows how to get Lessons and there full overviews.
//! It also shows how to interact with lessons entries etc

use std::process::Command;
use lanis_rs::base::account::{Account, AccountSecrets};
use lanis_rs::Error;
use lanis_rs::modules::lessons::{get_lessons, Attachment, LessonUpload};

#[tokio::main]
async fn main() {
    // Login
    let account = account().await;
    let account_keep_alive = account.clone();

    // Keep session alive
    tokio::spawn(async move{
        loop {
            account_keep_alive.prevent_logout().await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });

    loop {
        println!("Loading lessons...");
        let lessons = get_lessons(&account).await.unwrap(); // Get all Lessons (their overview)

        println!("Your Lessons:");
        for (i, lesson) in lessons.iter().enumerate() {
            println!("{i}: {}", lesson.name)
        }

        println!("Select your lesson");
        let index = index_selector(lessons.len());
        loop {
            println!("Loading lesson...");
            let mut lesson = lessons.get(index).unwrap().to_owned();
            // Set all data of the lesson
            lesson.set_data(&account).await.unwrap();

            // Print all entries
            for (i, entry) in lesson.entries.as_ref().unwrap_or(&Vec::new()).iter().enumerate() {
                println!("{:03}: {} - {}", i, entry.date.naive_local().date(), entry.title);
                if let Some(homework) = &entry.homework {
                    let completed = match homework.completed {
                        true => "Completed",
                        false => "Uncompleted",
                    };
                    println!("                  Homework: {}", completed);
                }
            }
            println!("{:03}: Go back", lesson.entries.as_ref().unwrap().len());

            println!("Please select your entry:");
            let i = index_selector(lesson.entries.as_ref().unwrap().len() + 1);

            if i == lesson.entries.as_ref().unwrap().len() { break; }

            let entry = lesson.entries.as_mut().unwrap().get_mut(i).unwrap();
            loop {
                let mut i = 0;
                println!("\nTitle: {}", entry.title);
                println!("Date: {}", entry.date.naive_local().date());
                if let Some(homework) = &entry.homework {
                    let completed = match homework.completed {
                        true => "Completed",
                        false => "Uncompleted",
                    };
                    println!("\n---- HOMEWORK ({}) ----\n{}\n---- HOMEWORK END ----\n", completed, homework.description);
                    println!("{}: Mark homework as completed/uncompleted", i);
                    i += 1;
                } else {
                    println!("{}: Mark homework as completed/uncompleted (Unavailable)", i);
                    i += 1;
                }

                if let Some(uploads) = &entry.uploads {
                    if !uploads.is_empty() {
                        println!("{}: View uploads", i);
                        i += 1;
                    } else {
                        println!("{}: View uploads (Unavailable)", i);
                        i += 1;
                    }
                } else {
                    println!("{}: View uploads (Unavailable)", i);
                    i += 1;
                }

                if let Some(attachments) = &entry.attachments {
                    if !attachments.is_empty() {
                        println!("{}: View attachments", i);
                        i += 1;
                    } else {
                        println!("{}: View attachments (Unavailable)", i);
                        i += 1;
                    }
                } else {
                    println!("{}: View attachments (Unavailable)", i);
                    i += 1;
                }
                println!("{}: Go back", i);
                i += 1;

                println!("What do you want to do now?");
                let i = index_selector(i + 1);

                if i == 0 {
                    if let Some(homework) = &mut entry.homework {
                        homework.set_homework(!homework.completed, lesson.id, entry.id, &account.client).await.unwrap() // Flip homework completion status
                    }
                } else if i == 1 {
                    if let Some(uploads) = &mut entry.uploads {
                        interact_uploads(uploads).await;
                    }
                } else if i == 2 {
                    if let Some(attachments) = &entry.attachments {
                        interact_attachments(attachments).await;
                    }
                } else {
                    break;
                }
            }
        }
    }
}

async fn interact_uploads(uploads: &Vec<LessonUpload>) {
    todo!()
}

async fn interact_attachments(attachments: &Vec<Attachment>) {
    loop {
        println!("Attachments:");
        for (i, attachment) in attachments.iter().enumerate() {
            println!("{:02}: {} ({})", i, attachment.name, attachment.url);
        }
    }
}

fn index_selector(max_index: usize) -> usize {
    loop {
        let mut index = String::new();
        std::io::stdin().read_line(&mut index).unwrap();
        let index = match index.trim().parse::<usize>() {
            Ok(usize) => {
                usize
            }
            Err(_) => {
                println!("Your input is not a number. Please enter a number!");
                continue;
            }
        };

        if index < max_index {
            break index;
        } else {
            println!("Your input is out of bounds! Please use an input inside the bounds!");
            continue;
        };
    }
}

async fn account() -> Account {
    loop {
        println!("Please enter your school id: ");
        let mut school_id = String::new();
        std::io::stdin().read_line(&mut school_id).unwrap();
        let school_id = match school_id.trim().parse::<i32>() {
            Ok(i32) => {
                i32
            }
            Err(_) => {
                println!("Your input is not a number. Please enter a number!");
                continue;
            }
        };

        println!("Please enter your username: ");
        let mut username = String::new();
        std::io::stdin().read_line(&mut username).unwrap();

        // Disable echoing
        let _ = Command::new("stty")
            .arg("-echo")
            .status()
            .unwrap();

        println!("Enter your password: ");
        let mut password = String::new();
        std::io::stdin().read_line(&mut password).unwrap();

        // Enable echoing
        let _ = Command::new("stty")
            .arg("echo")
            .status()
            .unwrap();

        let secrets = AccountSecrets::new(
            school_id,
            username.trim().to_string(), // Make sure to trim when using terminal input
            password.trim().to_string(),
        );


        println!("Logging in...");
        match Account::new(secrets).await {
            Ok(account) => break account,
            Err(e) => {
                match e {
                    Error::Credentials(_) => println!("Invalid credentials! Please try again."),
                    Error::SchoolNotFound(_) => println!("School not found! Please try again."),
                    Error::LoginTimeout(t) => println!("Login timeout! Please try again in {t}s."),
                    _ => println!("Something went wrong while trying to login. Please try again. {e}"),
                }
            }
        }
    }
}