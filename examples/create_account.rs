use std::process::Command;
use lanis_rs::base::account::{Account, AccountSecrets};
use lanis_rs::Error;

#[tokio::main]
async fn main() {
    let account = loop {
        // Get your credentials

        print!("Please enter your school id: ");
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

        print!("Please enter your username: ");
        let mut username = String::new();
        std::io::stdin().read_line(&mut username).unwrap();

        // Disable echoing
        let _ = Command::new("stty")
            .arg("-echo")
            .status()
            .unwrap();

        print!("Enter your password: ");
        let mut password = String::new();
        std::io::stdin().read_line(&mut password).unwrap();

        // Enable echoing
        let _ = Command::new("stty")
            .arg("echo")
            .status()
            .unwrap();

        let secrets = AccountSecrets::new(
            school_id,
            username,
            password,
        );


        // Perform login
        println!("Logging in...");
        match Account::new(secrets).await {
            Ok(account) => break account,
            Err(e) => {
                match e {
                    Error::Credentials(_) => println!("Invalid credentials! Please try again."),
                    Error::SchoolNotFound(_) => println!("School not found! Please try again."),
                    _ => println!("Something went wrong while trying to login. Please try again."),
                }
            }
        }
    };
    println!("Logged in successfully!");
    println!("Your account features that are also supported by lanis-rs: {:#?}", account.features)
}