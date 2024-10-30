use crate::base::account::AccountType::{Student, Teacher};
use crate::utils::crypt::{KeyPair, generate_key_pair};

use reqwest::header::LOCATION;
use reqwest::{Client, StatusCode};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};
use scraper::{Html, Selector};
use std::collections::BTreeMap;
use std::string::String;
use std::sync::Arc;
use reqwest::redirect::Policy;
use serde::Deserialize;
use crate::Feature;
use crate::utils::constants::URL;

#[derive(Debug, Clone)]
pub enum AccountType {
    Student,
    Teacher,
    Parent,
}

/// # Example
/// ### Automatic Creation
/// ```no_run
/// # use lanis_rs::base::account;
/// #
/// # use lanis_rs::base::account::AccountError;
///
/// async fn run() -> Result<(), AccountError>{
/// let account = account::new(
///     9999,
///     "rust.example".to_string(),
///     "ILoveRust1234!".to_string())
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Account {
    pub school_id: i32,
    pub username: String,
    pub password: String,
    pub account_type: Option<AccountType>,
    pub features: Option<Vec<Feature>>,
    pub data: Option<BTreeMap<String, String>>,
    /// You can generate a new KeyPair by using the Ok result of [generate_key_pair()] <br> Make sure to not define anything larger than 151 (bits) as size
    pub key_pair: KeyPair,
    pub client: Client,
    pub cookie_store: Arc<CookieStoreMutex>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountError {
    /// Can happen when doing a request to lanis
    Network(String),
    /// Happens if anything goes wrong with logging in
    Login(String),
    /// Happens if the "feature" field in [Account] is None
    FeaturesInit,
    /// Happens if the "data" field in [Account] is None
    DataInit,
    /// Happens if key_pair generation fails
    KeyPair,
    /// Happens if anything goes wrong with parsing <br>
    /// For example: `"a &str" as i32`
    Parsing(String),
}

impl Account {
    /**
      * Takes an account and a 'reqwest' client and generates a new session for lanis <br>
      * Needs to be run on every new 'reqwest' client <br>
      * Doesn't need to be run if [new] was used
     */
    pub async fn create_session(&self) -> Result<(), AccountError> {
        let params = [("user2", self.username.clone()), ("user", format!("{}.{}", self.school_id, self.username.clone())), ("password", self.password.clone())];
        let response = self.client.post(URL::LOGIN.to_owned() + &*format!("?i={}", self.school_id)).form(&params).send();
        match response.await {
            Ok(response) => {
                if response.status() == StatusCode::FOUND {
                    match self.client.get(URL::CONNECT).send().await {
                        Ok(response) => {
                            match response.headers().get(LOCATION) {
                                Some(location) => {
                                    let location = location.to_str();
                                    if  location.is_err() {
                                        return Err(AccountError::Parsing("failed to parse location header to str".to_string()))
                                    }
                                    let location = location.unwrap();

                                    match self.client.get(location).send().await {
                                        Ok(_) => Ok(()),
                                        Err(e) => {
                                            Err(AccountError::Network(format!("error getting login URL header: {}", e)))
                                        }
                                    }
                                }
                                None => {
                                    Err(AccountError::Login("error getting login URL".to_string()))
                                }
                            }
                        }
                        Err(e) => {
                            Err(AccountError::Network(format!("{}", e)))
                        }
                    }
                } else {
                    Err(AccountError::Login(format!("login failed with status code {}", response.status().as_u16())))
                }
            }
            Err(e) => {
                Err(AccountError::Network(e.to_string()))
            }
        }
    }

    /**
     *  Refreshes the session to prevent getting logged out
     *  <br> Needs to be called periodically e.g. every 10 seconds
     */
    pub async fn prevent_logout(&self) -> Result<(), AccountError> {
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
                Err(AccountError::Network(format!("failed to refresh session: {}", e).to_string()))
            }
        }
    }

    pub async fn fetch_account_data(&self) -> Result<BTreeMap<String, String>, AccountError> {
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
                Err(AccountError::Network(format!("failed to fetch account data: {}", e).to_string()))
            }
        }
    }

    pub async fn get_type(&self) -> Result<AccountType, AccountError> {
        match &self.data {
            None => {
                Err(AccountError::DataInit)
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

    /// Returns a vector of features
    pub async fn get_features(&self) -> Result<Vec<Feature>, AccountError> {

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        struct Entry {
            link: String,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        struct Entries {
            entrys: Vec<Entry>
        }

        match self.client.get(URL::START).query(&[("a", "ajax"), ("f", "apps")]).send().await {
            Ok(response) => {
                let text = response.text().await.unwrap();
                let entries = serde_json::from_str::<Entries>(&text).unwrap();

                let mut features = Vec::new();

                for entry in entries.entrys {
                    match entry.link.as_str() {
                        "meinunterricht.php" => features.push(Feature::MeinUnttericht),
                        _ => continue,
                    }
                }

                Ok(vec![Feature::MeinUnttericht])
            }
            Err(e) => Err(AccountError::Network(e.to_string()))
        }
    }

    pub async fn is_supported(&self, feature: Feature) -> Result<bool, AccountError> {
        match &self.features {
            Some(features) => {
                if features.contains(&feature) {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Err(AccountError::FeaturesInit)
        }
    }
}

/// Creates a new [Account] from a school_id, username and password <br>
/// When using [new] a session gets automatically created and all fields will be set
pub async fn new(school_id: i32, username: String, password: String) -> Result<Account, AccountError> {
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

    if key_pair.is_err() {
        return Err(AccountError::KeyPair);
    }

    let mut account = Account {
        school_id,
        username,
        password,
        account_type: None,
        data: None,
        features: None,
        key_pair: key_pair.unwrap(),
        client,
        cookie_store,
    };

    account.create_session().await?;
    account.data = Some(account.fetch_account_data().await?);
    account.account_type = Some(account.get_type().await?);
    account.features = Some(account.get_features().await?);

    Ok(account)
}
