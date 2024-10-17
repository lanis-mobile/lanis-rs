mod base;
mod utils;
mod modules;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use std::env;
    use crate::base::account;
    use crate::base::schools::{get_school_id, get_schools, School};
    use crate::modules::lessons::{get_lessons};
    use super::*;
    use stopwatch::Stopwatch;

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

        let result = get_schools(client).await.unwrap();
        assert_eq!(result.get(0).unwrap().id, 3354)
    }

    // This test everything that's bound to student accounts
    #[tokio::test]
    async fn test_student_account() {
        let stopwatch = Stopwatch::start_new();
        let mut account = account::generate(
            {
                env::var("LANIS_SCHOOL_ID").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_SCHOOL_ID' in env?", e);
                    String::from("0")
                }).parse().expect("Couldn't parse 'LANIS_SCHOOL_ID'.\nDid you define SCHOOL_ID as an i32?")
            },
            {
                env::var("LANIS_USERNAME").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_USERNAME' in env?", e);
                    String::from("")
                })
            },
            {
                env::var("LANIS_PASSWORD").unwrap_or_else(|e| {
                    println!("Error ({})\nDid you define 'LANIS_PASSWORD' in env?", e);
                    String::from("")
                })
            },
        ).await.unwrap();
        println!("account::generate() took {}ms", stopwatch.elapsed_ms());

        let stopwatch = Stopwatch::start_new();
        account.prevent_logout().await.unwrap();
        println!("account.prevent_logout() took {}ms", stopwatch.elapsed_ms());

        let stopwatch = Stopwatch::start_new();
        let mut lessons = get_lessons(&account).await.unwrap();
        println!("get_lessons() took {}ms", stopwatch.elapsed_ms());


        let stopwatch = Stopwatch::start_new();
        for mut lesson in lessons.lessons.iter_mut() {
            println!("\tid: {}", lesson.id);
            println!("\turl: {}", lesson.url);
            println!("\tname: {}", lesson.name);
            println!("\tteacher: {}", lesson.teacher);
            println!("\tteacher_short: {:?}", lesson.teacher_short);
            println!("\tattendances: {:?}", lesson.attendances);
            println!("\tentry_latest: {:?}", lesson.entry_latest);
            let stopwatch = Stopwatch::start_new();
            lesson.set_entries(&account).await.unwrap();
            println!("\tlesson.set_entries() took {}ms", stopwatch.elapsed_ms());
            println!("\tentries:");
            for entry in lesson.entries.clone().unwrap() {
                println!("\t\t{:?}", entry)
            }
        }
        println!("Iteration of all lessons took {}ms", stopwatch.elapsed_ms());


        print!("\n");

        println!("Private Key:\n{}", account.key_pair.private_key_string);
        println!("Public Key:\n{}", account.key_pair.public_key_string);

        assert_eq!(account.data.is_some(), true);
    }
}
