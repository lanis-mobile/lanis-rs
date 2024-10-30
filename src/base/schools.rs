use reqwest::Client;
use serde::{Deserialize};
use crate::utils::constants::URL;


#[derive(Debug, Clone)]
pub struct School {
    pub id: i32,
    pub name: String,
    pub city: String,
}

/// Returns the ID of a school based on name and city and takes a Vector of all schools <br> Returns -1 if no school was found
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
pub async fn get_schools(client: Client) -> Result<Vec<School>, String> {
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
        let response = client.get(URL::SCHOOLS).query(&[("a", "schoollist")]).send();
        match response.await {
            Ok(response) => {
                match response.text().await {
                    Ok(response) => {
                        let mut schools: Vec<School> = vec![];
                        let json_schools: serde_json::error::Result<Vec<JsonSchools>> = serde_json::from_str(&response);
                        if json_schools.is_err() {
                            return Err("Failed to parse school json!".to_string());
                        }
                        let json_schools = json_schools.unwrap();

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
                                        return Err(format!("Failed to parse id of school '{}'/'{}': {e}", school.ort,school.name))
                                    }
                                }
                            }
                        }
                        Ok(schools)
                    }
                    Err(e) => {
                        Err(format!("Failed to parse json:\n{:?}", e))

                    }
                }
            }
            Err(e) => {
                Err(format!("Failed to get school list:\n{}", e))
            }
        }
}