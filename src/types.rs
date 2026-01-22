use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub is_valid: bool,
    pub invalid_reason: Option<String>,
    #[allow(dead_code)]
    pub modified: DateTime<Local>,
    pub parent_dir: String,
}
