use std::{fs};
use std::io::{Read, Write};
use reqwest::Client;
use serde::{Deserialize};
use crate::utils::constants::URL;


#[derive(Debug)]
pub struct School {
    pub id: i32,
    pub name: String,
    pub city: String,
}

/// Returns the ID of a school based on name and city and takes a HashMap of all schools <br> Returns -1 if no school was found
pub async fn get_school_id(name: &str, city: &str, schools: &Vec<School>) -> i32 {
    for school in schools {
        if school.city == city {
            if school.name == name {
                return school.id
            }
        }
    }
    -1
}

/// If school.json already exists and <code>force_refresh</code> is <code>true</code> school.json will be overwritten
pub async fn get_schools(force_refresh: bool, client: Client) -> Vec<School> {
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct JsonSchool {
        id: String,
        name: String,
        ort: String,
    }
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct JsonSchools {
        schulen: Vec<JsonSchool>
    }
    let mut schools: Vec<School> = vec![];
    if fs::exists("/tmp/lanis-rs/schools.json").unwrap() && force_refresh {
        fs::remove_file("/tmp/lanis-rs/schools.json").unwrap();
    } else if !fs::exists("/tmp/lanis-rs").unwrap(){
        fs::create_dir("/tmp/lanis-rs").unwrap();
    }

    if !fs::exists("/tmp/lanis-rs/schools.json").unwrap() {
        let response = client.get(URL::SCHOOLS).query(&[("a", "schoollist")]).send();
        match response.await {
            Ok(response) => {
                match response.text().await {
                    Ok(response) => {
                        let file = fs::File::create("/tmp/lanis-rs/schools.json");
                        match file {
                            Ok(mut file) => {
                                match file.write_all(response.as_bytes()) {
                                    Ok(_) => {
                                        file.flush().unwrap();
                                    }
                                    Err(e) => {
                                        println!("Failed to write school list: {e}");
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Failed to create file '/tmp/lanis-rs/schools.json':\n{e}");
                            }
                        }
                    }
                    Err(e) => {
                        println!("Failed to parse json:\n{:?}", e);

                    }
                }
            }
            Err(e) => {
                println!("Failed to get school list:\n{}", e);
            }
        }
    }

    if fs::exists("/tmp/lanis-rs/schools.json").unwrap() {
        let mut file = fs::File::open("/tmp/lanis-rs/schools.json").unwrap();
        let mut data = String::new();
        let result = file.read_to_string(&mut data);
        match result {
            Ok(_) => {
                let json_schools: Vec<JsonSchools> = serde_json::from_str(&*data).expect("Failed to parse schools.json");
                for json_school in json_schools {
                    for school in json_school.schulen {
                        let id = school.id.parse();
                        match id {
                            Ok(id) => {
                                schools.push(School{
                                    id,
                                    name: school.name,
                                    city: school.ort,
                                });
                            }
                            Err(e) => {
                                println!("Failed to parse id of school '{}'/'{}': {e}", school.ort,school.name);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("Failed to read school list:\n{}", e);
            }
        }
        schools
    } else {
        schools
    }

}