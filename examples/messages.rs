//! This example shows how to send messages and create conversations

use std::process::Command;
use reqwest::Client;
use lanis_rs::base::account::{Account, AccountSecrets};
use lanis_rs::Error;
use lanis_rs::modules::messages::{Conversation, ConversationOverview};
use lanis_rs::utils::crypt::LanisKeyPair;

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
        println!("Loading conversations...");
        // First get all ConversationOverview's from the root page
        let overviews = ConversationOverview::get_root(&account.client, &account.key_pair).await.unwrap();
        // Let's show all conversations
        println!("Conversations:");
        for (i, overview) in overviews.iter().enumerate() {
            // Print hidden behind conversation if it is not visible
            match overview.visible {
                true => println!("{i}: {} (Hidden)", overview.subject),
                false => println!("{i}: {}", overview.subject)
            }
        }
        println!("{}:? Create new conversation (Action)", overviews.len());

        // Select specific conversation to display or select to create new conversation
        let index = loop {
            println!("What conversation do you want to see?:");
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

            if index <= overviews.len() {
                break index;
            } else {
                println!("Your input is out of bounds! Please use an input inside the bounds!");
                continue;
            };
        };

        if index == overviews.len() { // Create a new conversation
            todo!()
        } else { // Participate in a conversation
            // Now display the chosen conversation
            // For this we need to get the complete conversation
            let conversation = overviews.get(index).unwrap().get(&account.client, &account.key_pair).await.unwrap();
            interact(conversation, &account.client, &account.key_pair).await;
        }
    }

}

async fn interact(mut conversation: Conversation, client: &Client, lanis_key_pair: &LanisKeyPair) {
    async fn send_message(conversation: &Conversation, client: &Client, lanis_key_pair: &LanisKeyPair) {
        // Send a new message (if allowed)
        if conversation.can_reply {
            println!("Write your message (Press Enter to send)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            println!("Sending message...");
            // Now send the message. This returns the UID of the new message
            let uid = conversation.reply(&input, client, lanis_key_pair).await.unwrap();
            if uid.is_none() {
                println!("Failed to send message.");
            }
        } else {
            println!("You have no permission to write messages here :(")
        }
    }

    loop {
        // Now lets display the conversation
        // First print all messages
        for message in &conversation.messages {
            if !message.own {
                println!("{} on {}: {}", message.author.name, message.date.naive_local(), message.content);
            } else {
                println!("YOU on {}: {}", message.date.naive_local(), message.content);
            }
        }

        println!();
        println!("------------- END MESSAGES -------------");
        println!();

        // Print the subject
        println!("Subject: {}", conversation.subject);
        // Print when the conversation was created (in local time)
        println!("Created: {}", conversation.date_time.naive_local());

        // Print all the participants of this conversation and the amount
        print!("Participants ({}): ", conversation.amount_participants);
        for participant in &conversation.participants {
            print!("{} & ", participant.name);
        }
        println!();
        println!("Students: {}", conversation.amount_students);
        println!("Teachers: {}", conversation.amount_teachers);
        println!("Parents: {}", conversation.amount_parents);

        println!("What do you want to do now? (Select action by typing the number)");
        println!("0: Refresh");
        println!("1: Send message");
        println!("2: Exit");

        let index = loop {
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

            if index <= 2 {
                break index;
            } else {
                println!("Your input is out of bounds! Please use an input inside the bounds!");
                continue;
            };
        };

        if index == 1 {
            send_message(&conversation, &client, &lanis_key_pair).await;
        } else if index == 2 {
            break;
        }

        // Refresh the conversation
        println!("Refreshing...");
        conversation.refresh(client, lanis_key_pair).await.unwrap();
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