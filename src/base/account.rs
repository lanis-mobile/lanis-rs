use crate::base::account::AccountType::{Student, Teacher};
use crate::utils::constants::URL;
use crate::utils::crypt::{KeyPair, generate_key_pair};

use reqwest::header::LOCATION;
use reqwest::{Client, StatusCode};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};
use scraper::{Html, Selector};
use std::collections::BTreeMap;
use std::string::String;
use std::sync::Arc;
use reqwest::redirect::Policy;

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
    pub account_type: Option<AccountType>,
    pub data: Option<BTreeMap<String, String>>,
    /// You can generate a new KeyPair by using the Ok result of [generate_key_pair()] <br> Make sure to not define anything larger than 151 (bits) as size
    pub key_pair: KeyPair,
    pub client: Client,
    pub cookie_store: Arc<CookieStoreMutex>,
}

impl Account {
    /**
     *  Takes an account and a 'reqwest' client and generates a new session for lanis <br>
     *  Needs to be run on every new 'reqwest' client
     */
    pub async fn create_session(&self) -> Result<(), String> {
        let params = [("user2", self.username.clone()), ("user", format!("{}.{}", self.school_id, self.username.clone())), ("password", self.password.clone())];
        let response = self.client.post(URL::LOGIN.to_owned() + &*format!("?i={}", self.school_id)).form(&params).send();
        match response.await {
            Ok(response) => {
                if response.status() == StatusCode::FOUND {
                    match self.client.get(URL::CONNECT).send().await {
                        Ok(response) => {
                            match response.headers().get(LOCATION).unwrap().to_str() {
                                Ok(location) => {
                                    match self.client.get(location).send().await {
                                        Ok(_) => Ok(()),
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
    pub async fn prevent_logout(&self) -> Result<(), String> {
        let sid: String = {
            let cs = self.cookie_store.lock().unwrap();
            let mut result = "NONE".to_string();
            for cookie in cs.iter_any() {
                if cookie.name() == "sid" {
                    result = cookie.value().to_string();
                }
            }
            result
        };
        let param = [("name", sid)];
        match self.client.get(URL::LOGIN_AJAX).form(&param).send().await {
            Ok(_) => {
                Ok(())
            }
            Err(e) => {
                Err(format!("Failed to refresh session: {}", e).to_string())
            }
        }
    }

    pub async fn fetch_account_data(&self) -> Result<BTreeMap<String, String>, String> {
        match self.client.get(URL::USER_DATA).query(&[("a", "userData")]).send().await {
            Ok(response) => {
                let document = Html::parse_document(&*response.text().await.unwrap());
                let user_data_table_body_selector = Selector::parse("div.col-md-12 table.table.table-striped tbody").unwrap();

                let row_selector = Selector::parse("tr").unwrap();
                let key_selector = Selector::parse("td").unwrap();

                let mut result = BTreeMap::new();

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

/// Creates a new account struct
pub async fn generate(school_id: i32, username: String, password: String) -> Result<Account, String> {
    let cookie_store = CookieStore::new(None);
    let cookie_store = CookieStoreMutex::new(cookie_store);
    let cookie_store = Arc::new(cookie_store);

    let client = Client::builder()
        .redirect(Policy::none())
        .cookie_provider(std::sync::Arc::clone(&cookie_store))
        .gzip(true)
        .build()
        .unwrap();

    let key_pair = generate_key_pair(128, &client).await;

    let mut account = Account {
        school_id,
        username,
        password,
        account_type: None,
        data: None,
        key_pair: key_pair?,
        client,
        cookie_store,
    };

    account.create_session().await?;
    account.data = Some(account.fetch_account_data().await?);
    account.account_type = Some(account.get_type().await?);

    Ok(account)
}
