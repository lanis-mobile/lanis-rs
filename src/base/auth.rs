use reqwest::{Client, StatusCode};
use std::string::String;
use crate::utils::constants::URL;

pub struct Account {
    pub school_id: i32,
    pub username: String,
    pub password: String,
}

/// Returns true if everything worked
pub(crate) async fn create_new_session_sub_step_1(account: &Account, client: &Client) -> bool {
    let mut result = false;
    let params= [("user2", account.username.clone()), ("user", format!("{}.{}", account.school_id, account.username.clone())), ("password", account.password.clone())];
    let response = client.post(URL::LOGIN.to_owned() + &*format!("?i={}", account.school_id)).form(&params).send();
    match response.await {
        Ok(response) => {
            if response.status() == StatusCode::FOUND {
                result = true;
            }
            else {
                println!("Login failed with status code {}", response.status().as_u16());
            }
        }
        Err(e) => {
            println!("Failed to check credentials:\n{}", e);
        }
    }
    result
}