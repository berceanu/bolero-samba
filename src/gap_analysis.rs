use crate::types::FileEntry;
use chrono::{Datelike, NaiveDate};
use colored::Colorize;
use std::collections::HashSet;

#[derive(Debug, PartialEq)]
pub struct GapReport {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub missing_weekdays: Vec<NaiveDate>,
    pub skipped_weekends: u32,
    pub is_empty: bool,
}

pub fn analyze_gaps(files: &[FileEntry], line_id: &str) {
    let report = find_gaps(files, line_id);

    if report.is_empty {
        println!("No dated folders found for gap analysis.");
        return;
    }

    for missing in &report.missing_weekdays {
        let day_name = missing.format("%A").to_string(); // Monday, Tuesday, etc.
        println!(
            "{} {} ({}) - Archive not found",
            "⚠️".yellow(),
            missing,
            day_name
        );
    }

    println!(
        "Range checked: {} to {}",
        report.start_date, report.end_date
    );
    if report.missing_weekdays.is_empty() {
        println!(
            "{} ({} weekends skipped)",
            "No weekday gaps found.".green(),
            report.skipped_weekends
        );
    }
}

#[must_use]
pub fn find_gaps(files: &[FileEntry], line_id: &str) -> GapReport {
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

    if dates.is_empty() {
        return GapReport {
            start_date: NaiveDate::default(),
            end_date: NaiveDate::default(),
            missing_weekdays: vec![],
            skipped_weekends: 0,
            is_empty: true,
        };
    }

    let start = *dates.first().unwrap();
    let end = *dates.last().unwrap();
    let existing_set: HashSet<NaiveDate> = dates.iter().copied().collect();

    let mut missing_weekdays = Vec::new();
    let mut skipped_weekends = 0;
    let mut curr = start;

    while curr < end {
        if !existing_set.contains(&curr) {
            let dow = curr.weekday().number_from_monday();
            if dow <= 5 {
                missing_weekdays.push(curr);
            } else {
                skipped_weekends += 1;
            }
        }
        match curr.succ_opt() {
            Some(next_date) => curr = next_date,
            None => break, // Shouldn't happen with valid date ranges
        }
    }

    GapReport {
        start_date: start,
        end_date: end,
        missing_weekdays,
        skipped_weekends,
        is_empty: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, Utc};

    // Helper to create dummy entry
    fn make_entry(date_str: &str, line_id: &str) -> FileEntry {
        FileEntry {
            name: "dummy.zip".to_string(),
            size: 0,
            is_valid: true,
            invalid_reason: None,
            modified: Utc::now().with_timezone(&Local),
            parent_dir: format!("Archive_Beam_{}_{}", line_id, date_str),
        }
    }

    #[test]
    fn test_find_gaps_clean_week() {
        // Mon 29th, Tue 30th, Wed 31st (No gaps)
        let files = vec![
            make_entry("2024-07-29", "B"),
            make_entry("2024-07-30", "B"),
            make_entry("2024-07-31", "B"),
        ];

        let report = find_gaps(&files, "B");
        assert_eq!(report.missing_weekdays.len(), 0);
        assert_eq!(report.skipped_weekends, 0);
        assert_eq!(
            report.start_date,
            NaiveDate::from_ymd_opt(2024, 7, 29).unwrap()
        );
        assert_eq!(
            report.end_date,
            NaiveDate::from_ymd_opt(2024, 7, 31).unwrap()
        );
    }

    #[test]
    fn test_find_gaps_missing_tuesday() {
        // Mon 29th, Wed 31st (Tue missing)
        let files = vec![
            make_entry("2024-07-29", "B"), // Mon
            make_entry("2024-07-31", "B"), // Wed
        ];

        let report = find_gaps(&files, "B");
        assert_eq!(report.missing_weekdays.len(), 1);
        assert_eq!(
            report.missing_weekdays[0],
            NaiveDate::from_ymd_opt(2024, 7, 30).unwrap()
        );
    }

    #[test]
    fn test_find_gaps_skip_weekend() {
        // Fri 2nd, Mon 5th (Sat 3rd, Sun 4th missing but acceptable)
        let files = vec![
            make_entry("2024-08-02", "B"), // Fri
            make_entry("2024-08-05", "B"), // Mon
        ];

        let report = find_gaps(&files, "B");
        assert_eq!(report.missing_weekdays.len(), 0);
        assert_eq!(report.skipped_weekends, 2); // Sat, Sun
    }
}
