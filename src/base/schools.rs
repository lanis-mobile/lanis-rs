use std::collections::HashMap;
use std::future::Future;
use reqwest::{Error, Response};
use crate::utils::constants::URL;

pub struct School {
    pub id: i16,
    pub name: String,
    pub city: String,
}

/// Returns the ID of a school based on name and city and takes a HashMap of all schools <br> Returns -1 if no school was found
pub fn get_school_id(name: &str, city: &str, schools: &HashMap<i16, School>) -> i16 {
    for (_, school) in schools {
        if school.city == city {
            if school.name == name {
                return school.id
            }
        }
    }
    -1
}
pub fn get_schools() -> i8 {
    let client = reqwest::Client::new();
    let response = client.get(URL::SCHOOLS).query(&[("a", "schoolist")]).send();
    match response {
        Ok(response) => {
            println!("{}", response);
            0
        }
        Err(e) => {
            println!("Failed to get school list:\n{}", e);
            1
        }
    }

}