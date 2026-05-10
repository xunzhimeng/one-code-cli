use chrono::Utc;
use uuid::Uuid;

pub fn run_id() -> String {
    format!("run_{}_{}", timestamp(), short_random())
}

pub fn session_id() -> String {
    format!("sess_{}_{}", timestamp(), short_random())
}

fn timestamp() -> String {
    Utc::now().format("%Y%m%d_%H%M%S").to_string()
}

fn short_random() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(4)
        .collect()
}
