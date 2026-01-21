use crate::types::FileEntry;
use chrono::{DateTime, Local};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::SystemTime;
use walkdir::WalkDir;

pub fn get_total_size(path: &str) -> u64 {
    // Use blocks() (512-byte blocks) instead of len() for accurate disk usage
    // This matches du behavior and detects active transfers immediately
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(std::fs::Metadata::is_file)
        .map(|m| m.blocks() * 512) // blocks() is in 512-byte units
        .sum()
}

#[must_use]
pub fn scan_files(path: &str) -> Vec<FileEntry> {
    // 1. Collect all ZIP files into a vector (Sequential Walk)
    let entries: Vec<_> = WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        })
        .collect();

    let total_files = entries.len();

    // 2. Process metadata and integrity sequentially
    let results: Vec<FileEntry> = entries
        .into_iter()
        .enumerate()
        .map(|(i, entry)| {
            if i % 50 == 0 || i == total_files - 1 {
                print!("\r  Verifying files: {}/{}:..", i + 1, total_files);
                std::io::stdout().flush().ok();
            }

            let p = entry.path();

            // Metadata access
            let metadata = entry
                .metadata()
                .unwrap_or_else(|_| std::fs::metadata(p).unwrap());
            let size = metadata.len();
            let name = entry.file_name().to_string_lossy().to_string();

            let parent = p
                .parent()
                .and_then(|parent_path| parent_path.file_name())
                .map_or_else(
                    || "Unknown".to_string(),
                    |name| name.to_string_lossy().to_string(),
                );

            // Integrity check
            let (is_valid, invalid_reason) = is_zip_valid(p);

            // Use UNIX_EPOCH as fallback instead of now() to avoid falsely marking
            // files as "recent" when we can't read their modification time
            let modified: DateTime<Local> =
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH).into();

            FileEntry {
                name,
                size,
                is_valid,
                invalid_reason,
                modified,
                parent_dir: parent,
            }
        })
        .collect();

    println!(); // New line after progress
    results
}

fn is_zip_valid(path: &Path) -> (bool, Option<String>) {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return (false, Some("Cannot open file".to_string())),
    };

    // Quick check: valid zip must be at least 22 bytes (EOCD size)
    let len = match file.metadata() {
        Ok(m) => m.len(),
        Err(_) => return (false, Some("Cannot read file metadata".to_string())),
    };
    if len < 22 {
        return (false, Some(format!("File too small ({} bytes, minimum 22 bytes required)", len)));
    }

    // Search last 64KB + 22 bytes for EOCD signature (0x06054b50)
    let search_len = std::cmp::min(len, 65535 + 22) as i64;

    if file.seek(SeekFrom::End(-search_len)).is_err() {
        return (false, Some("Cannot seek to end of file".to_string()));
    }

    let mut buffer = Vec::with_capacity(search_len as usize);
    if file.read_to_end(&mut buffer).is_err() {
        return (false, Some("Cannot read file contents".to_string()));
    }

    // Search for signature: 0x06054b50 => [0x50, 0x4b, 0x05, 0x06]
    let signature = [0x50, 0x4b, 0x05, 0x06];

    for i in (0..buffer.len().saturating_sub(3)).rev() {
        if buffer[i] == signature[0]
            && buffer[i + 1] == signature[1]
            && buffer[i + 2] == signature[2]
            && buffer[i + 3] == signature[3]
        {
            return (true, None);
        }
    }

    (false, Some("Missing ZIP signature (corrupted or incomplete transfer)".to_string()))
}

#[must_use]
pub fn get_recent_files(path: &str, minutes: i64) -> Vec<String> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            let mod_time: DateTime<Local> = m.modified().ok()?.into();
            let now = Local::now();
            let diff = now.signed_duration_since(mod_time);

            if diff.num_minutes() < minutes {
                let full_path = e.path().to_string_lossy();
                // Extract path starting from "Line " onwards
                let display_path = if let Some(idx) = full_path.find("Line ") {
                    &full_path[idx..]
                } else {
                    &full_path
                };

                Some(format!(
                    "  - {} ({}) at {}",
                    display_path,
                    human_bytes::human_bytes(m.len() as f64),
                    mod_time.format("%Y-%m-%d %H:%M")
                ))
            } else {
                None
            }
        })
        .collect()
}

#[must_use]
pub fn has_recent_activity(path: &str, minutes: i64) -> bool {
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .any(|e| {
            if let Ok(m) = e.metadata()
                && let Ok(mod_time) = m.modified()
            {
                let mod_time: DateTime<Local> = mod_time.into();
                let now = Local::now();
                return now.signed_duration_since(mod_time).num_minutes() < minutes;
            }
            false
        })
}
