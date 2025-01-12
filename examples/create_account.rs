use std::process::Command;
use lanis_rs::base::account::{Account, AccountSecrets};
use lanis_rs::Error;

#[tokio::main]
async fn main() {
    let account = loop {
        // Get your credentials
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
    println!("Your account features that are also supported by lanis-rs: {:#?}", account.features);

    // Encrypt account secrets for later to save them for example in system keyring
    let key = b"ILikeToast12!EncryptionIsSoNice!";
    let encrypted_secrets = account.secrets.encrypt(key).await.unwrap();

    // Decrypt encrypted secrets and re-login
    let secrets = AccountSecrets::from_encrypted(&encrypted_secrets, key).await.unwrap();

    // Perform login
    println!("Logging in...");
    let account = match Account::new(secrets).await {
        Ok(account) => account,
        Err(e) => {
            panic!("Error creating account: {}", e);
        }
    };
    println!("Logged in successfully!");
    println!("Your account features that are also supported by lanis-rs: {:#?}", account.features);
}