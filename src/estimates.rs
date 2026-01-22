use crate::types::FileEntry;
use chrono::{Datelike, Local, NaiveDate};
use colored::Colorize;
use std::fs;
use std::process::Command;

#[derive(Debug)]
pub struct EstimatesReport {
    pub current_copy_date: NaiveDate,
    pub daily_average_gib: u64,
    pub weekdays_remaining: u32,
    pub weekdays_completed: u32,
    pub total_weekdays: u32,
    pub estimated_data_left_tib: f64,
    pub free_space_tib: f64,
    pub disk_status_ok: bool,
    pub estimated_days_eta: Option<u64>,
    pub estimated_hours_eta: Option<u64>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[must_use]
pub fn calculate_estimates(
    search_dir: &str,
    files: &[FileEntry],
    line_id: &str,
    _total_bytes: u64,
    speed_bps: u64,
    base_dir: &str,
) -> Option<EstimatesReport> {
    // 1. Parse Config
    let config_path = format!("{}/Transfer.ps1", base_dir);
    let (start_date, end_date) = parse_config(&config_path)?;

    // 2. Identify Current Copy Date
    let current_copy_date = get_current_copy_date(files, line_id, search_dir).unwrap_or(start_date); // If no data, start from beginning (0% progress)

    // 3. Calculate median daily size (more robust than mean for outliers)
    // Group files by parent_dir and calculate size per folder
    let mut folder_sizes: Vec<u64> = {
        let mut size_map: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        for f in files {
            *size_map.entry(f.parent_dir.clone()).or_insert(0) += f.size;
        }
        size_map.values().copied().collect()
    };
    
    folder_sizes.sort_unstable();
    
    let median_bytes_per_day = if !folder_sizes.is_empty() {
        let count = folder_sizes.len();
        if count % 2 == 1 {
            folder_sizes[count / 2]
        } else {
            u64::midpoint(folder_sizes[count / 2 - 1], folder_sizes[count / 2])
        }
    } else {
        0
    };
    
    let daily_average_gib = median_bytes_per_day / 1_024 / 1_024 / 1_024;

    // 4. Calculate total weekdays from start to end
    let mut total_weekdays = 0;
    let mut iter_date = start_date;
    while iter_date <= end_date {
        let dow = iter_date.weekday().number_from_monday();
        if dow <= 5 {
            total_weekdays += 1;
        }
        match iter_date.succ_opt() {
            Some(next_date) => iter_date = next_date,
            None => break, // Reached max date, shouldn't happen with valid ranges
        }
    }

    // 5. Calculate weekdays completed (from start to current)
    let mut weekdays_completed = 0;
    let mut iter_date = start_date;
    while iter_date < current_copy_date {
        let dow = iter_date.weekday().number_from_monday();
        if dow <= 5 {
            weekdays_completed += 1;
        }
        match iter_date.succ_opt() {
            Some(next_date) => iter_date = next_date,
            None => break,
        }
    }

    // 6. Calculate Remaining
    let mut weekdays_remaining = 0;
    if let Some(mut iter_date) = current_copy_date.succ_opt() {
        while iter_date <= end_date {
            let dow = iter_date.weekday().number_from_monday();
            if dow <= 5 {
                weekdays_remaining += 1;
            }
            match iter_date.succ_opt() {
                Some(next_date) => iter_date = next_date,
                None => break,
            }
        }
    }

    // Even if weekdays_remaining is 0, we still want to show progress
    // So let's not return None - just set estimates to 0
    // if weekdays_remaining == 0 {
    //     return None; // Transfer complete
    // }

    let total_remaining_bytes = u64::from(weekdays_remaining) * median_bytes_per_day;
    let estimated_data_left_tib =
        total_remaining_bytes as f64 / 1_024.0 / 1_024.0 / 1_024.0 / 1_024.0;

    let avail_bytes = get_available_bytes(search_dir);
    let free_space_tib = avail_bytes as f64 / 1_024.0 / 1_024.0 / 1_024.0 / 1_024.0;
    let disk_status_ok = avail_bytes > total_remaining_bytes;

    let (estimated_days_eta, estimated_hours_eta) = if speed_bps > 0 {
        let seconds_left = total_remaining_bytes / speed_bps;
        let hours_left = seconds_left / 3600;
        let days_eta = hours_left / 24;
        (Some(days_eta), Some(hours_left))
    } else {
        (None, None)
    };

    Some(EstimatesReport {
        current_copy_date,
        daily_average_gib,
        weekdays_remaining,
        weekdays_completed,
        total_weekdays,
        estimated_data_left_tib,
        free_space_tib,
        disk_status_ok,
        estimated_days_eta,
        estimated_hours_eta,
        start_date,
        end_date,
    })
}

pub fn print_estimates(report: &Option<EstimatesReport>) {
    println!("\n{}", "=== Transfer Estimates ===".cyan());

    if let Some(r) = report {
        // Calculate progress based on completed weekdays vs total weekdays
        let progress_pct = if r.total_weekdays > 0 {
            ((r.weekdays_completed as f64 / r.total_weekdays as f64 * 100.0).min(100.0)) as u8
        } else {
            0
        };

        // Render progress bar (20 blocks total)
        let filled_blocks = (progress_pct as usize * 20) / 100;
        let empty_blocks = 20 - filled_blocks;
        let progress_bar = format!("{}{}", "▓".repeat(filled_blocks), "░".repeat(empty_blocks));

        println!(
            "Transfer Progress: {} {}%",
            progress_bar.green(),
            progress_pct
        );

        println!(
            "Current Progress:  Copying {}",
            r.current_copy_date.format("%Y-%m-%d").to_string().green()
        );
        
        // Format full project date range
        let date_range = format!(
            "{} - {}",
            r.start_date.format("%b %Y"),
            r.end_date.format("%b %Y")
        );
        
        println!(
            "Data to Copy:      {} daily archives ({})",
            r.weekdays_remaining, date_range
        );
        println!(
            "Est. Data Left:    {:.1} TiB (Free: {:.1} TiB)",
            r.estimated_data_left_tib, r.free_space_tib
        );

        if r.disk_status_ok {
            println!("Disk Status:       {}", "OK".green());
        } else {
            println!(
                "Disk Status:       {}",
                "CRITICAL - Insufficient Space!".red()
            );
        }

        if let (Some(days), Some(hours)) = (r.estimated_days_eta, r.estimated_hours_eta) {
            println!(
                "Time to Complete:  ~{} days ({} hours) at current speed",
                days.to_string().yellow(),
                hours
            );
        }
    } else {
        println!("Transfer appears complete.");
    }
}

fn parse_config(path: &str) -> Option<(NaiveDate, NaiveDate)> {
    let content = fs::read_to_string(path).ok()?;

    let mut start = None;
    let mut end = None;

    for line in content.lines() {
        if start.is_none() && line.contains("$startDate") {
            start = extract_date_from_ps1(line);
        } else if end.is_none() && line.contains("$endDate") {
            end = extract_date_from_ps1(line);
        }
    }

    match (start, end) {
        (Some(s), Some(e)) => Some((s, e)),
        _ => None,
    }
}

fn extract_date_from_ps1(line: &str) -> Option<NaiveDate> {
    // Look for content inside quotes: "2024-07-29"
    // Handles both simple quotes and Get-Date "..."
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 3 {
        // Try to parse the second part (between quotes)
        NaiveDate::parse_from_str(parts[1], "%Y-%m-%d").ok()
    } else {
        None
    }
}

fn get_folder_dates(files: &[FileEntry], line_id: &str) -> Vec<NaiveDate> {
    let prefix = format!("Archive_Beam_{line_id}_");
    let mut dates: Vec<NaiveDate> = files
        .iter()
        .filter_map(|f| {
            if f.parent_dir.starts_with(&prefix) {
                let date_str = &f.parent_dir[prefix.len()..];
                if date_str.len() >= 10 {
                    NaiveDate::parse_from_str(&date_str[0..10], "%Y-%m-%d").ok()
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    dates.sort();
    dates.dedup();
    dates
}

fn get_current_copy_date(
    files: &[FileEntry],
    line_id: &str,
    _search_dir: &str,
) -> Option<NaiveDate> {
    // 1. Try to get date from recent files (modified in last 5 mins)
    // Note: Since we passed `files` (All Zips), we can check their mod times.
    // Ideally we want the "Active" file.
    let now = Local::now();
    let recent_date = files
        .iter()
        .filter(|f| {
            let diff = now.signed_duration_since(f.modified);
            diff.num_minutes() < 5
        })
        // Extract date from parent folder of the recent file
        .filter_map(|f| {
            let prefix = format!("Archive_Beam_{line_id}_");
            if f.parent_dir.starts_with(&prefix) {
                let date_str = &f.parent_dir[prefix.len()..];
                if date_str.len() >= 10 {
                    NaiveDate::parse_from_str(&date_str[0..10], "%Y-%m-%d").ok()
                } else {
                    None
                }
            } else {
                None
            }
        })
        .next();

    if let Some(d) = recent_date {
        return Some(d);
    }

    // 2. Fallback: Latest existing folder date
    let dates = get_folder_dates(files, line_id);
    if let Some(last) = dates.last() {
        return Some(*last);
    }

    // 3. No data exists - return None so we can default to start_date (0% progress)
    None
}

fn get_available_bytes(path: &str) -> u64 {
    // Use `df` via Command since sysinfo Disks can be tricky with specific mount paths
    // df -k output:
    // Filesystem     1K-blocks      Used Available Use% Mounted on
    // /dev/sdb1      123456789  12345678 111111111  10% /data

    let output = Command::new("df").arg("-k").arg(path).output().ok();

    if let Some(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Skip header, take 2nd line, 4th column (Available)
        // Note: df columns: FS, Blocks, Used, Avail, ...
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4
                && let Ok(kb) = parts[3].parse::<u64>()
            {
                return kb * 1024;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_extract_date_from_ps1() {
        let line = "$startDate = \"2024-07-29\"";
        let date = extract_date_from_ps1(line);
        assert_eq!(
            date,
            NaiveDate::parse_from_str("2024-07-29", "%Y-%m-%d").ok()
        );

        let line_err = "$startDate = \"invalid-date\"";
        assert!(extract_date_from_ps1(line_err).is_none());
    }

    #[test]
    fn test_parse_config() {
        let file_path = "test_Transfer.ps1";
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(file, "$startDate = \"2024-07-29\"").unwrap();
        writeln!(file, "$endDate = \"2024-09-10\"").unwrap();

        let (start, end) = parse_config(file_path).unwrap();
        assert_eq!(
            start,
            NaiveDate::parse_from_str("2024-07-29", "%Y-%m-%d").unwrap()
        );
        assert_eq!(
            end,
            NaiveDate::parse_from_str("2024-09-10", "%Y-%m-%d").unwrap()
        );

        fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_get_current_copy_date() {
        // Case 1: Active Transfer (Recent file exists)
        let now = Local::now();
        let recent_time = now - chrono::Duration::minutes(2);

        let files_active = vec![
            FileEntry {
                name: "file1.zip".to_string(),
                size: 100,
                is_valid: true,
                invalid_reason: None,
                modified: recent_time,
                parent_dir: "Archive_Beam_B_2024-08-01".to_string(),
            },
            FileEntry {
                name: "file2.zip".to_string(),
                size: 100,
                is_valid: true,
                invalid_reason: None,
                modified: now - chrono::Duration::hours(1), // Old
                parent_dir: "Archive_Beam_B_2024-07-31".to_string(),
            },
        ];

        let date_active = get_current_copy_date(&files_active, "B", ".");
        assert_eq!(
            date_active,
            Some(NaiveDate::parse_from_str("2024-08-01", "%Y-%m-%d").unwrap())
        );

        // Case 2: No Active Transfer (Use latest folder)
        let files_idle = vec![
            FileEntry {
                name: "file1.zip".to_string(),
                size: 100,
                is_valid: true,
                invalid_reason: None,
                modified: now - chrono::Duration::hours(2),
                parent_dir: "Archive_Beam_B_2024-07-31".to_string(),
            },
            FileEntry {
                name: "file2.zip".to_string(),
                size: 100,
                is_valid: true,
                invalid_reason: None,
                modified: now - chrono::Duration::hours(3),
                parent_dir: "Archive_Beam_B_2024-08-02".to_string(),
            },
        ];

        let date_idle = get_current_copy_date(&files_idle, "B", ".");
        // Should pick 2024-08-02 as it is the latest folder date in the list
        assert_eq!(
            date_idle,
            Some(NaiveDate::parse_from_str("2024-08-02", "%Y-%m-%d").unwrap())
        );
    }

    #[test]
    fn test_parse_config_missing_dates() {
        let file_path = "test_Transfer_bad.ps1";
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(file, "some other content").unwrap();

        assert!(parse_config(file_path).is_none());

        fs::remove_file(file_path).unwrap();
    }
}
