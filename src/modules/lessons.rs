use reqwest::Client;

struct Lesson {
    title: String,
    date: String,
    subject_name: String,
    teacher: String,
    description: Option<String>,
    details: Option<String>,
    homework: Option<Homework>,
    attachments: Vec<Attachment>,
    attachments_url: String,
}

struct Attachment {
    name: String,
    url: String,
}

struct Homework {
    pub description: String,
    pub completed: bool,

}

pub async fn get_lessons(client: &Client) -> Result<Vec<(Lesson, String)>, String> {
    let vec: Vec<(Lesson, String)> = Vec::new();
    Ok(vec)
}