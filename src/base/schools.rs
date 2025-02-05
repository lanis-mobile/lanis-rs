use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::utils::constants::URL;
use crate::Error;


#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
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

/// Returns all schools matching the query or an empty vec if there are too many results.
pub async fn search_untis_school(query: &str) -> Result<Vec<untis::School>, Error> {
    untis::schools::search(query).map_err(|e| Error::UntisAPI(e.to_string()))
}

/// Returns a [School] based on the provided ID
pub async fn get_school(id: &i32, schools: &Vec<School>) -> Result<School, Error> {
    for school in schools {
        if school.id == *id {
            return Ok(school.clone())
        }
    }
    Err(Error::SchoolNotFound(format!("No school with id {} found", id)))
}

pub async fn get_schools(client: &Client) -> Result<Vec<School>, Error> {
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
                            return Err(Error::Parsing("Failed to parse school json!".to_string()));
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
                                        return Err(Error::Parsing(format!("Failed to parse id of school '{}'/'{}': {e}", school.ort,school.name)))
                                    }
                                }
                            }
                        }
                        Ok(schools)
                    }
                    Err(e) => {
                        Err(Error::Parsing(format!("Failed to parse json:\n{:?}", e)))

                    }
                }
            }
            Err(e) => {
                Err(Error::Network(format!("Failed to get school list:\n{}", e)))
            }
        }
}