use crate::base::account::AccountType::{Student, Teacher};
use crate::utils::constants::URL;
use reqwest::header::LOCATION;
use reqwest::{Client, StatusCode};
use reqwest_cookie_store::CookieStoreMutex;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::string::String;

#[derive(Debug)]
pub enum AccountType {
    Student,
    Teacher,
    Parent,
}

#[derive(Debug)]
pub struct Account {
    pub school_id: i32,
    pub username: String,
    pub password: String,
    pub type_a: Option<AccountType>,
    pub data: Option<HashMap<String, String>>,
}

impl Account {
    /**
     *  Takes an account and a 'reqwest' client and generates a new session for lanis <br>
     *  Needs to be run on every new 'reqwest' client
     */
    pub async fn create_session(&self, client: &Client) -> Result<(), String> {
        let params = [("user2", self.username.clone()), ("user", format!("{}.{}", self.school_id, self.username.clone())), ("password", self.password.clone())];
        let response = client.post(URL::LOGIN.to_owned() + &*format!("?i={}", self.school_id)).form(&params).send();
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
                } else {
                    Err(format!("Login failed with status code {}", response.status().as_u16()))
                }
            }
            Err(e) => {
                Err(format!("Failed to get response from '{}':\n{}", URL::LOGIN, e))
            }
        }
    }

    /**
     *  Refreshes the session to prevent getting logged out
     *  <br> Needs to be called periodically e.g. every 10 seconds
     */
    pub async fn prevent_logout(&self, client: &Client, cookie_store: &CookieStoreMutex) -> Result<(), String> {
        let sid: String = {
            let cs = cookie_store.lock().unwrap();
            let mut result = "NONE".to_string();
            for cookie in cs.iter_any() {
                if cookie.name() == "sid" {
                    result = cookie.value().to_string();
                }
            }
            result
        };
        let param = [("name", sid)];
        match client.get(URL::LOGIN_AJAX).form(&param).send().await {
            Ok(_) => {
                Ok(())
            }
            Err(e) => {
                Err(format!("Failed to refresh session: {}", e).to_string())
            }
        }
    }

    pub async fn fetch_account_data(&self, client: &Client) -> Result<HashMap<String, String>, String> {
        match client.get(URL::USER_DATA).query(&[("a", "userData")]).send().await {
            Ok(response) => {
                let document = Html::parse_document(&*response.text().await.unwrap());
                let user_data_table_body_selector = Selector::parse("div.col-md-12 table.table.table-striped tbody").unwrap();

                let row_selector = Selector::parse("tr").unwrap();
                let key_selector = Selector::parse("td").unwrap();

                let mut result = HashMap::new();

                if let Some(user_data_table_body) = document.select(&user_data_table_body_selector).next() {
                    for row in user_data_table_body.select(&row_selector) {
                        let cells: Vec<_> = row.select(&key_selector).collect();
                        if cells.len() >= 2 {
                            let key = cells[0].text().collect::<String>().trim().to_string();
                            let value = cells[1].text().collect::<String>().trim().to_string();
                            let key = key[..key.len() - 1].to_lowercase();
                            result.insert(key, value);
                        }
                    }
                }
                Ok(result)
            }
            Err(e) => {
                Err(format!("Failed to fetch account data: {}", e).to_string())
            }
        }
    }

    pub async fn get_type(&self) -> Result<AccountType, String> {
        match &self.data {
            None => {
                Err("No account data found!".to_string())
            }
            Some(account_data) => {
                if account_data.contains_key("klasse") {
                    Ok(Student)
                } else {
                    Ok(Teacher)
                }
            }
        }
    }
}
