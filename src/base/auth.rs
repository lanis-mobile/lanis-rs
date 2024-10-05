use reqwest::{Client, StatusCode};
use std::string::String;
use reqwest::header::LOCATION;
use crate::utils::constants::URL;

pub struct Account {
    pub school_id: i32,
    pub username: String,
    pub password: String,
}

/**
 *  Takes an account and a 'reqwest' client and generates a new session for lanis <br>
 *  Needs to be run on every new 'reqwest' client
 */
pub async fn create_session(account: &Account, client: &Client) -> Result<(), String> {
    let params= [("user2", account.username.clone()), ("user", format!("{}.{}", account.school_id, account.username.clone())), ("password", account.password.clone())];
    let response = client.post(URL::LOGIN.to_owned() + &*format!("?i={}", account.school_id)).form(&params).send();
    match response.await {
        Ok(response) => {
            if response.status() == StatusCode::FOUND {
                match client.get(URL::CONNECT).send().await {
                    Ok(response) => {
                        match response.headers().get(LOCATION).unwrap().to_str() {
                            Ok(location) => {
                                 match client.get(location).send().await {
                                     Ok(_) => {
                                         Ok(())
                                     }
                                     Err(e) => {
                                         Err(format!("Error getting login URL header: {}", e))
                                     }
                                 }
                            }
                            Err(e) => {
                                Err(format!("Error getting login URL: {}", e))
                            }
                        }
                    }
                    Err(e) => {
                        Err(format!("{} {}", e, response.status()))
                    }
                }
            }
            else {
                Err(format!("Login failed with status code {}", response.status().as_u16()))
            }
        }
        Err(e) => {
            Err(format!("Failed to get response from '{}':\n{}", URL::LOGIN, e))
        }
    }
}