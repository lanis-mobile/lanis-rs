mod base;
mod utils;
mod modules;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use std::env;
    use reqwest::redirect::Policy;
    use crate::base::account::Account;
    use crate::base::schools::{get_school_id, get_schools, School};
    use crate::modules::lessons::{get_lessons};
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[tokio::test]
    async fn test_schools_get_school_id() {
        let mut schools: Vec<School> = vec![];
        schools.push(School{
            id: 3120,
            name: String::from("The Almighty Rust School"),
            city: String::from("Rust City")
        });
        schools.push(School{
            id: 3920,
            name: String::from("The Almighty Rust School"),
            city: String::from("Rust City 2")
        });
        schools.push(School{
            id: 4031,
            name: String::from("The Almighty Rust School 2"),
            city: String::from("Rust City")
        });
        let result = get_school_id("The Almighty Rust School", "Rust City 2", &schools).await;
        assert_eq!(result, 3920);
    }

    #[tokio::test]
    async fn test_schools_get_schools() {
        let client = reqwest::Client::new();

        let result = get_schools(true, client).await;
        assert_eq!(result.get(0).unwrap().id, 3354)
    }

    // This test everything that's bound to student accounts
    #[tokio::test]
    async fn test_student_account() {
        let cookie_store = reqwest_cookie_store::CookieStore::new(None);
        let cookie_store = reqwest_cookie_store::CookieStoreMutex::new(cookie_store);
        let cookie_store = std::sync::Arc::new(cookie_store);

        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .cookie_provider(std::sync::Arc::clone(&cookie_store))
            .build()
            .unwrap();

        let mut account = Account {
            school_id: {
                env::var("LANIS_SCHOOL_ID").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_SCHOOL_ID' in env?", e);
                    String::from("0")
                }).parse().expect("Couldn't parse 'LANIS_SCHOOL_ID'.\nDid you define SCHOOL_ID as an i32?")
            },
            username: {
                env::var("LANIS_USERNAME").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_USERNAME' in env?", e);
                    String::from("")
                })
            },
            password:  {
                env::var("LANIS_PASSWORD").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_PASSWORD' in env?", e);
                    String::from("")
                })
            },
            type_a: None,
            data: None
        };

        if !account.create_session(&client).await.is_ok() {
            panic!("Wrong login credentials!")
        }

        account.prevent_logout(&client, &cookie_store).await.unwrap();

        account.data = Option::from(account.fetch_account_data(&client).await.unwrap());
        account.type_a = Option::from(account.get_type().await.unwrap());

        let lessons = get_lessons(&client).await.unwrap();

        assert_eq!(account.data.is_some(), true);
        assert_eq!(account.type_a.is_some(), true);

    }
}
