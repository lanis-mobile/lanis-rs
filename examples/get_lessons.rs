//! This example shows how to get Lessons and there full overviews

use std::process::Command;
use lanis_rs::base::account::{Account, AccountSecrets};
use lanis_rs::Error;
use lanis_rs::modules::lessons::get_lessons;

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

    println!("Loading lessons...");
    let lessons = get_lessons(&account).await.unwrap();

    println!("Your Lessons:");
    for (i, lesson) in lessons.iter().enumerate() {
        println!("{i}: {}", lesson.name)
    }

    let index = loop {
        println!("What lesson do you want to see?:");
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

        if lessons.get(index).is_some() {
            break index;
        } else {
            println!("Your input is out of bounds! Please use an input inside the bounds!");
            continue;
        };
    };

    let mut lesson = lessons.get(index).unwrap().to_owned();
    // Set all data of the lesson
    let _ = lesson.set_data(&account).await.unwrap();
    // Display entire raw lesson (You may do other things with this
    println!("Full lesson: {:#?}", lesson);
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