use crate::base::account::AccountType::{Student, Teacher};
use crate::base::schools::{get_school, get_schools, School};
use crate::utils::constants::URL;
use crate::utils::crypt::{
    decrypt_any, encrypt_any, generate_lanis_key_pair, CryptorError, LanisKeyPair,
};
use crate::utils::datetime::date_string_to_naivedate;
use crate::Error;
use crate::Feature;
use chrono::NaiveDate;
use reqwest::header::LOCATION;
use reqwest::redirect::Policy;
use reqwest::{Client, StatusCode};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::string::String;
use std::sync::Arc;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum AccountType {
    Student,
    Teacher,
    Parent,
    Unknown,
}

impl Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Student => write!(f, "Student"),
            Teacher => write!(f, "Teacher"),
            AccountType::Parent => write!(f, "Parent"),
            AccountType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Stores everything that is needed at Runtime and related to the Account
#[derive(Clone, Debug)]
pub struct Account {
    pub school: School,
    pub secrets: AccountSecrets,
    pub account_type: AccountType,
    pub features: Vec<Feature>,
    pub info: AccountInfo,
    /// You can generate a new KeyPair by using the Ok result of [generate_lanis_key_pair()] <br> Make sure to not define anything larger than 151 (bits) as size
    pub key_pair: LanisKeyPair,
    pub client: Client,
    pub cookie_store: Arc<CookieStoreMutex>,
}

/// The account info
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AccountInfo {
    pub firstname: String,
    pub lastname: String,
    pub username: String,
    pub birthdate: NaiveDate,
    pub gender: Gender,
    /// Should be Some if the Account is of type Student so safe to call unwrap on
    pub student: Option<AccountInfoStudent>,
    /// Should be Some if the Account is of type Teacher so safe to call unwrap on
    pub teacher: Option<AccountInfoTeacher>,
}

impl AccountInfo {
    pub fn empty() -> Self {
        Self {
            firstname: String::new(),
            lastname: String::new(),
            username: String::new(),
            birthdate: NaiveDate::MIN,
            gender: Gender::Unknown,
            student: None,
            teacher: None,
        }
    }
}

/// Student specifc infos. There is no gurantee for all fields to be filled (they may be empty)
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AccountInfoStudent {
    pub grade: String,
    pub class: String,
}

/// Teacher specifc infos. There is no gurantee for all fields to be filled (they may be empty)
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AccountInfoTeacher {
    pub personal_number: String,
    /// The "Klassenleitungen" list
    pub classes: Vec<String>,
    /// The "Stellvertretende Klassenleitungen" list
    pub classes_sub: Vec<String>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
    Diverse,
    Unknown,
}

impl Account {
    /// Creates a new [Account] from a school_id, username and password <br>
    /// When using [new] a session gets automatically created and all fields will be set
    pub async fn new(secrets: AccountSecrets) -> Result<Account, Error> {
        let cookie_store = CookieStore::new(None);
        let cookie_store = CookieStoreMutex::new(cookie_store);
        let cookie_store = Arc::new(cookie_store);

        let client = Client::builder()
            .redirect(Policy::none())
            .cookie_provider(std::sync::Arc::clone(&cookie_store))
            .gzip(true)
            .build()
            .unwrap();

        let key_pair = generate_lanis_key_pair(128, &client).await?;

        let schools = get_schools(&client).await?;
        let school = get_school(&secrets.school_id, &schools).await?;

        let mut account = Account {
            school,
            secrets,
            account_type: AccountType::Unknown,
            info: AccountInfo::empty(),
            features: Vec::new(),
            key_pair,
            client,
            cookie_store,
        };

        account.create_session().await?;
        (account.info, account.account_type) = account.fetch_account_info().await?;
        account.features = account.get_features().await?;

        Ok(account)
    }

    /**
     * Takes an account and a 'reqwest' client and generates a new session for lanis <br>
     * Needs to be run on every new 'reqwest' client <br>
     * Doesn't need to be run if [new] was used
     */
    pub async fn create_session(&self) -> Result<(), Error> {
        let params = [
            ("user2", self.secrets.username.clone()),
            (
                "user",
                format!("{}.{}", self.school.id, self.secrets.username.clone()),
            ),
            ("password", self.secrets.password.clone()),
        ];
        let response = self
            .client
            .post(URL::LOGIN.to_owned() + &*format!("?i={}", self.school.id))
            .form(&params)
            .send();
        match response.await {
            Ok(response) => {
                let response_status = response.status();

                let text = response.text().await.map_err(|e| {
                    Error::Parsing(format!("Failed to parse response as text: {}", e))
                })?;
                let html = Html::parse_document(&text);

                let timeout_selector = Selector::parse("#authErrorLocktime").unwrap();
                if let Some(timeout) = html.select(&timeout_selector).nth(0) {
                    return Err(Error::LoginTimeout(
                        timeout
                            .text()
                            .collect::<String>()
                            .trim()
                            .parse()
                            .map_err(|e| {
                                Error::Parsing(format!(
                                    "Failed to parse timeout from response as u32: {}",
                                    e
                                ))
                            })?,
                    ));
                }

                if response_status == StatusCode::FOUND {
                    match self.client.get(URL::CONNECT).send().await {
                        Ok(response) => match response.headers().get(LOCATION) {
                            Some(location) => {
                                let location = location.to_str();
                                if location.is_err() {
                                    return Err(Error::Parsing(
                                        "failed to parse location header to str".to_string(),
                                    ));
                                }
                                let location = location.unwrap();

                                match self.client.get(location).send().await {
                                    Ok(_) => Ok(()),
                                    Err(e) => Err(Error::Network(format!(
                                        "error getting login URL header: {}",
                                        e
                                    ))),
                                }
                            }
                            None => Err(Error::Network("error getting login URL".to_string())),
                        },
                        Err(e) => Err(Error::Network(format!("{}", e))),
                    }
                } else {
                    Err(Error::Credentials("Wrong credentials!".to_string()))
                }
            }
            Err(e) => Err(Error::Network(e.to_string())),
        }
    }

    /**
     *  Refreshes the session to prevent getting logged out
     *  <br> Needs to be called periodically e.g. every 10 seconds
     */
    pub async fn prevent_logout(&self) -> Result<(), Error> {
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
            Ok(_) => Ok(()),
            Err(e) => Err(Error::Network(
                format!("failed to refresh session: {}", e).to_string(),
            )),
        }
    }

    pub async fn fetch_account_info(&self) -> Result<(AccountInfo, AccountType), Error> {
        match self
            .client
            .get(URL::USER_DATA)
            .query(&[("a", "userData")])
            .send()
            .await
        {
            Ok(response) => {
                let document = Html::parse_document(&response.text().await.unwrap());
                let user_data_table_body_selector =
                    Selector::parse("div.col-md-12 table.table.table-striped tbody").unwrap();

                let row_selector = Selector::parse("tr").unwrap();
                let key_selector = Selector::parse("td").unwrap();

                let mut result = BTreeMap::new();

                if let Some(user_data_table_body) =
                    document.select(&user_data_table_body_selector).next()
                {
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

                let firstname = result.get("vorname").unwrap_or(&String::new()).to_owned();
                let lastname = result.get("nachname").unwrap_or(&String::new()).to_owned();
                let username = result.get("login").unwrap_or(&String::new()).to_owned();
                let birthdate = {
                    let s = result
                        .get("geburtsdatum")
                        .unwrap_or(&String::from("01.01.1970"))
                        .to_owned();
                    date_string_to_naivedate(&s).map_err(|e| {
                        Error::DateTime(format!("failed to convert date to DateTime '{:?}'", e))
                    })?
                };
                let gender = {
                    let s = result
                        .get("geschlecht")
                        .unwrap_or(&String::new())
                        .to_owned();
                    match s.as_str() {
                        "männlich" => Gender::Male,
                        "weiblich" => Gender::Female,
                        "divers" => Gender::Diverse,
                        _ => Gender::Unknown,
                    }
                };
                let account_type = if result.contains_key("stufe") {
                    AccountType::Student
                } else if result.contains_key("personalnummer") {
                    AccountType::Teacher
                } else {
                    AccountType::Unknown
                };

                let info = match account_type {
                    AccountType::Student => {
                        let grade = result.get("stufe").unwrap_or(&String::new()).to_owned();
                        let class = result.get("klasse").unwrap_or(&String::new()).to_owned();

                        let student = Some(AccountInfoStudent { grade, class });

                        AccountInfo {
                            firstname,
                            lastname,
                            username,
                            birthdate,
                            gender,
                            student,
                            teacher: None,
                        }
                    }
                    AccountType::Teacher => {
                        let personal_number = result
                            .get("personalnummer")
                            .unwrap_or(&String::new())
                            .to_owned();
                        let classes = Vec::new(); // TODO: Parse bullet point list
                        let classes_sub = Vec::new(); // TODO: Parse bullet point list
                        let teacher = Some(AccountInfoTeacher {
                            personal_number,
                            classes,
                            classes_sub,
                        });

                        AccountInfo {
                            firstname,
                            lastname,
                            username,
                            birthdate,
                            gender,
                            student: None,
                            teacher,
                        }
                    }
                    AccountType::Parent => {
                        // TODO: Fetch Parent specifc info
                        AccountInfo {
                            firstname,
                            lastname,
                            username,
                            birthdate,
                            gender,
                            student: None,
                            teacher: None,
                        }
                    }
                    AccountType::Unknown => AccountInfo {
                        firstname,
                        lastname,
                        username,
                        birthdate,
                        gender,
                        student: None,
                        teacher: None,
                    },
                };

                Ok((info, account_type))
            }
            Err(e) => Err(Error::Network(
                format!("failed to fetch account data: {}", e).to_string(),
            )),
        }
    }

    pub async fn get_type(&self) -> AccountType {
        self.account_type.to_owned()
    }

    /// Returns a vector of supported features (for the [Account])
    pub async fn get_features(&self) -> Result<Vec<Feature>, Error> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        struct Entry {
            link: String,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        struct Entries {
            entrys: Vec<Entry>,
        }

        match self
            .client
            .get(URL::START)
            .query(&[("a", "ajax"), ("f", "apps")])
            .send()
            .await
        {
            Ok(response) => {
                let text = response.text().await.unwrap();
                let entries = serde_json::from_str::<Entries>(&text).unwrap();

                let mut features = Vec::new();

                for entry in entries.entrys {
                    match entry.link.trim() {
                        "meinunterricht.php" => features.push(Feature::MeinUnttericht),
                        "stundenplan.php" => features.push(Feature::LanisTimetable),
                        "dateispeicher.php" => features.push(Feature::FileStorage),
                        "nachrichten.php" => features.push(Feature::MessagesBeta),
                        "kalender.php" => features.push(Feature::Calendar),
                        _ => continue,
                    }
                }

                Ok(features)
            }
            Err(e) => Err(Error::Network(e.to_string())),
        }
    }

    pub fn is_supported(&self, feature: Feature) -> bool {
        if self.features.contains(&feature) {
            true
        } else {
            false
        }
    }
}

/// Contains the account secrets for Lanis and maybe Untis <br>
/// This will be used for re-login. <br>
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct AccountSecrets {
    pub school_id: i32,
    pub username: String,
    pub password: String,
    pub untis_secrets: Option<UntisSecrets>,
}

impl AccountSecrets {
    pub fn new(school_id: i32, username: String, password: String) -> AccountSecrets {
        Self {
            school_id,
            username,
            password,
            untis_secrets: None,
        }
    }

    pub async fn from_encrypted(
        data: &[u8],
        key: &[u8; 32],
    ) -> Result<AccountSecrets, CryptorError> {
        decrypt_any(data, key).await
    }

    pub async fn encrypt(&self, key: &[u8; 32]) -> Result<Vec<u8>, CryptorError> {
        encrypt_any(&self, key).await
    }
}

/// Contains the account secrets for Untis <br>
/// This will be used for re-login. <br>
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct UntisSecrets {
    /// The internal school name from Untis and not the display name
    pub school_name: String,
    pub username: String,
    pub password: String,
}

impl UntisSecrets {
    pub fn new(school_name: String, username: String, password: String) -> UntisSecrets {
        Self {
            school_name,
            username,
            password,
        }
    }
}
