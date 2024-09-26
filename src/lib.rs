mod base;
mod utils;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::result;
    use crate::base::schools::{get_school_id, get_schools, School};
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_get_school_id() {
        let mut schools = HashMap::new();
        schools.insert(3120,School{
            id: 3120,
            name: String::from("The Almighty Rust School"),
            city: String::from("Rust City")
        });
        schools.insert(3920,School{
            id: 3920,
            name: String::from("The Almighty Rust School"),
            city: String::from("Rust City 2")
        });
        schools.insert(4031,School{
            id: 4031,
            name: String::from("The Almighty Rust School 2"),
            city: String::from("Rust City")
        });
        let result = get_school_id("The Almighty Rust School", "Rust City 2", &schools);
        assert_eq!(result, 3920);
    }

    #[test]
    fn test_get_schools() {
        let result: i8 = get_schools();
        assert_eq!(result, 0)
    }
}
