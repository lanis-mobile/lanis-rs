//! This example shows how to send messages and create conversations

use lanis_rs::base::account::{Account, AccountSecrets};
use lanis_rs::modules::messages::{
    create_conversation, search_receiver, Conversation, ConversationOverview,
};
use lanis_rs::utils::crypt::LanisKeyPair;
use lanis_rs::Error;
use reqwest::Client;
use std::process::Command;

#[tokio::main]
async fn main() {
    // Login
    let account = account().await;
    let account_keep_alive = account.clone();

    // Keep session alive
    tokio::spawn(async move {
        loop {
            account_keep_alive.prevent_logout().await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });

    loop {
        println!("Loading conversations...");
        // First get all ConversationOverview's from the root page
        let overviews = ConversationOverview::get_root(&account.client, &account.key_pair)
            .await
            .unwrap();
        // Let's show all conversations
        println!("Conversations:");
        for (i, overview) in overviews.iter().enumerate() {
            // Print hidden behind conversation if it is not visible
            match overview.visible {
                true => println!("{i}: {}", overview.subject),
                false => println!("{i}: {} (Hidden)", overview.subject),
            }
        }
        println!("{}:? Create new conversation (Action)", overviews.len());
        println!("{}:? Show/Hide conversation (Action)", overviews.len() + 1);

        // Select specific conversation to display or select to create new conversation
        let index = loop {
            println!("What conversation do you want to see?:");
            let mut index = String::new();
            std::io::stdin().read_line(&mut index).unwrap();
            let index = match index.trim().parse::<usize>() {
                Ok(usize) => usize,
                Err(_) => {
                    println!("Your input is not a number. Please enter a number!");
                    continue;
                }
            };

            if index <= overviews.len() + 1 {
                break index;
            } else {
                println!("Your input is out of bounds! Please use an input inside the bounds!");
                continue;
            };
        };

        if index == overviews.len() {
            // Create a new conversation
            let mut receivers = Vec::new();
            loop {
                println!("What person do you want to message?");
                let mut query = String::new();
                std::io::stdin().read_line(&mut query).unwrap();
                // Search for receivers based on the query
                let results = search_receiver(query.trim(), &account.client)
                    .await
                    .unwrap();
                for (i, result) in results.iter().enumerate() {
                    println!("{}: {} ({})", i, result.name, result.account_type)
                }
                println!("{}: Retry", results.len());

                let index = loop {
                    println!("Please select your person:");
                    let mut index = String::new();
                    std::io::stdin().read_line(&mut index).unwrap();
                    let index = match index.trim().parse::<usize>() {
                        Ok(usize) => usize,
                        Err(_) => {
                            println!("Your input is not a number. Please enter a number!");
                            continue;
                        }
                    };

                    if index <= results.len() {
                        break index;
                    } else {
                        println!(
                            "Your input is out of bounds! Please use an input inside the bounds!"
                        );
                        continue;
                    };
                };

                if index != results.len() {
                    receivers.push(results.get(index).unwrap().to_owned());
                }

                println!("Do you want add another person? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                if input.trim() == "y" || input.trim() == "Y" {
                } else {
                    break;
                }
            }

            println!("What subject do you want for your message?");
            let mut subject = String::new();
            std::io::stdin().read_line(&mut subject).unwrap();
            let subject = subject.trim();

            println!("What do you want to write?");
            let mut text = String::new();
            std::io::stdin().read_line(&mut text).unwrap();
            let text = text.trim();

            println!("Creating conversation...");
            let uid = create_conversation(
                &receivers,
                subject,
                text,
                &account.client,
                &account.key_pair,
            )
            .await
            .unwrap();
            if uid.is_some() {
                println!("Creating of conversation failed!")
            } else {
                println!("Successfully created conversation!")
            }
        } else if index == overviews.len() + 1 {
            // Hide / Show conversation
            // Select specific conversation to hide / show
            let index = loop {
                println!("What conversation do you want to show/hide?");
                let mut index = String::new();
                std::io::stdin().read_line(&mut index).unwrap();
                let index = match index.trim().parse::<usize>() {
                    Ok(usize) => usize,
                    Err(_) => {
                        println!("Your input is not a number. Please enter a number!");
                        continue;
                    }
                };

                if index < overviews.len() {
                    break index;
                } else {
                    println!("Your input is out of bounds! Please use an input inside the bounds!");
                    continue;
                };
            };
            println!("Performing action...");
            let mut overview = overviews.get(index).unwrap().to_owned();
            let result = match overview.visible {
                true => overview.hide(&account.client).await.unwrap(), // hide the conversation
                false => overview.show(&account.client).await.unwrap(), // show the conversation
            };
            if result {
                println!("Success!")
            } else {
                println!("Failed!")
            }
        } else {
            // Participate in a conversation
            // Now display the chosen conversation
            // For this we need to get the complete conversation
            let conversation = overviews
                .get(index)
                .unwrap()
                .get(&account.client, &account.key_pair)
                .await
                .unwrap();
            interact(conversation, &account.client, &account.key_pair).await;
        }
    }
}

async fn interact(mut conversation: Conversation, client: &Client, lanis_key_pair: &LanisKeyPair) {
    async fn send_message(
        conversation: &Conversation,
        client: &Client,
        lanis_key_pair: &LanisKeyPair,
    ) {
        // Send a new message (if allowed)
        if conversation.can_reply {
            println!("Write your message (Press Enter to send)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            println!("Sending message...");
            // Now send the message. This returns the UID of the new message
            let uid = conversation
                .reply(&input, client, lanis_key_pair)
                .await
                .unwrap();
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
                println!(
                    "{} on {}: {}",
                    message.author.name,
                    message.date.naive_local(),
                    message.content
                );
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
                Ok(usize) => usize,
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
            Ok(i32) => i32,
            Err(_) => {
                println!("Your input is not a number. Please enter a number!");
                continue;
            }
        };

        println!("Please enter your username: ");
        let mut username = String::new();
        std::io::stdin().read_line(&mut username).unwrap();

        // Disable echoing
        let _ = Command::new("stty").arg("-echo").status().unwrap();

        println!("Enter your password: ");
        let mut password = String::new();
        std::io::stdin().read_line(&mut password).unwrap();

        // Enable echoing
        let _ = Command::new("stty").arg("echo").status().unwrap();

        let secrets = AccountSecrets::new(
            school_id,
            username.trim().to_string(), // Make sure to trim when using terminal input
            password.trim().to_string(),
        );

        println!("Logging in...");
        match Account::new(secrets).await {
            Ok(account) => break account,
            Err(e) => match e {
                Error::Credentials(_) => println!("Invalid credentials! Please try again."),
                Error::SchoolNotFound(_) => println!("School not found! Please try again."),
                Error::LoginTimeout(t) => println!("Login timeout! Please try again in {t}s."),
                _ => println!("Something went wrong while trying to login. Please try again. {e}"),
            },
        }
    }
}

