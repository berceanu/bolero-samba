use crate::gap_analysis::GapReport;
use crate::stats::AnomalyReport;
use crate::types::FileEntry;
use chrono::{Datelike, Local, NaiveDate};
use comfy_table::{Attribute, Cell, Color, Table};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct MonthlyMetrics {
    pub year: i32,
    pub month: u32,
    pub missing_days: usize,
    pub anomaly_count: usize,
    pub invalid_files: usize,
    pub empty_files: usize,
    pub expected_weekdays: usize,
    pub actual_archives: usize,
    pub health_score: f64,
    pub is_complete: bool,
}

#[derive(Debug)]
pub struct MonthlyRankingReport {
    pub months: Vec<MonthlyMetrics>,
    pub best_month: Option<MonthlyMetrics>,
    pub worst_month: Option<MonthlyMetrics>,
    pub average_score: f64,
    #[allow(dead_code)]
    pub line_id: String,
}

/// Extract date from directory name (e.g., "Archive_Beam_B_2024-10-15" -> 2024-10-15)
fn extract_date_from_dir(parent_dir: &str, line_id: &str) -> Option<NaiveDate> {
    let prefix = format!("Archive_Beam_{}_", line_id);
    if parent_dir.starts_with(&prefix) {
        let date_str = &parent_dir[prefix.len()..];
        if date_str.len() >= 10 {
            return NaiveDate::parse_from_str(&date_str[0..10], "%Y-%m-%d").ok();
        }
    }
    None
}

/// Count expected weekdays in a month, clamped to the date range and today
fn count_expected_weekdays_in_month(
    year: i32,
    month: u32,
    range_start: NaiveDate,
    range_end: NaiveDate,
    today: NaiveDate,
) -> usize {
    // Get first and last day of the month
    let month_start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let month_end = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
            .unwrap()
            .pred_opt()
            .unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
            .unwrap()
            .pred_opt()
            .unwrap()
    };

    // Clamp to the configured date range and today
    let effective_start = month_start.max(range_start);
    let effective_end = month_end.min(range_end).min(today);

    if effective_start > effective_end {
        return 0;
    }

    // Count weekdays
    let mut count = 0;
    let mut curr = effective_start;
    while curr <= effective_end {
        let dow = curr.weekday().number_from_monday();
        if dow <= 5 {
            count += 1;
        }
        match curr.succ_opt() {
            Some(next) => curr = next,
            None => break,
        }
    }
    count
}

/// Find files that are ALWAYS empty (same file is empty in ALL archives)
fn find_always_empty_files(files: &[FileEntry], tiny_threshold: u64) -> HashSet<String> {
    let mut file_counts: HashMap<String, (usize, usize)> = HashMap::new(); // (total, empty)
    for f in files {
        let entry = file_counts.entry(f.name.clone()).or_insert((0, 0));
        entry.0 += 1;
        if f.size < tiny_threshold {
            entry.1 += 1;
        }
    }
    // Files where empty_count == total_count are "always empty"
    file_counts
        .into_iter()
        .filter(|(_, (total, empty))| total == empty && *total > 0)
        .map(|(name, _)| name)
        .collect()
}

/// Calculate health score based on monthly metrics
fn calculate_health_score(metrics: &MonthlyMetrics) -> f64 {
    if metrics.expected_weekdays == 0 {
        return 0.0;
    }

    // Weighted penalties:
    // - missing_days * 10 (total data loss - highest severity)
    // - invalid_files * 3 (corrupted data - high severity)
    // - anomaly_count * 2 (size issues - medium severity)
    // - empty_files * 1 (sometimes empty - low severity)
    let penalty = (metrics.missing_days * 10
        + metrics.invalid_files * 3
        + metrics.anomaly_count * 2
        + metrics.empty_files) as f64;

    // max_penalty = expected_weekdays * 10 (all days missing scenario)
    let max_penalty = (metrics.expected_weekdays * 10) as f64;

    let score = 100.0 * (1.0 - penalty / max_penalty);
    score.max(0.0).min(100.0)
}

/// Calculate monthly performance rankings
#[must_use]
pub fn calculate_monthly_rankings(
    files: &[FileEntry],
    gap_report: Option<&GapReport>,
    anomaly_report: Option<&AnomalyReport>,
    line_id: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    tiny_threshold: u64,
) -> MonthlyRankingReport {
    let today = Local::now().date_naive();

    // Find always-empty files to exclude from scoring
    let always_empty = find_always_empty_files(files, tiny_threshold);

    // Group files by (year, month)
    let mut monthly_files: HashMap<(i32, u32), Vec<&FileEntry>> = HashMap::new();
    let mut monthly_archive_dates: HashMap<(i32, u32), HashSet<NaiveDate>> = HashMap::new();

    for file in files {
        if let Some(date) = extract_date_from_dir(&file.parent_dir, line_id) {
            let key = (date.year(), date.month());
            monthly_files.entry(key).or_default().push(file);
            monthly_archive_dates.entry(key).or_default().insert(date);
        }
    }

    // Build set of missing weekdays per month from gap report
    let mut missing_by_month: HashMap<(i32, u32), usize> = HashMap::new();
    if let Some(report) = gap_report {
        for missing_date in &report.missing_weekdays {
            let key = (missing_date.year(), missing_date.month());
            *missing_by_month.entry(key).or_insert(0) += 1;
        }
    }

    // Build set of anomalies per month from anomaly report
    let mut anomalies_by_month: HashMap<(i32, u32), usize> = HashMap::new();
    if let Some(report) = anomaly_report {
        for anomaly in &report.anomalies {
            // Anomaly.name is the directory name, extract date from it
            if let Some(date) = extract_date_from_dir(&anomaly.name, line_id) {
                let key = (date.year(), date.month());
                *anomalies_by_month.entry(key).or_insert(0) += 1;
            }
        }
    }

    // Collect all months that have any data
    let mut all_months: HashSet<(i32, u32)> = monthly_files.keys().copied().collect();
    all_months.extend(missing_by_month.keys().copied());

    // Calculate metrics for each month
    let mut months: Vec<MonthlyMetrics> = all_months
        .into_iter()
        .filter_map(|(year, month)| {
            let expected_weekdays =
                count_expected_weekdays_in_month(year, month, start_date, end_date, today);

            // Skip months with no expected weekdays (future months or outside range)
            if expected_weekdays == 0 {
                return None;
            }

            let files_in_month = monthly_files.get(&(year, month));
            let archive_dates = monthly_archive_dates.get(&(year, month));

            // Count invalid files
            let invalid_files = files_in_month
                .map(|f| f.iter().filter(|e| !e.is_valid).count())
                .unwrap_or(0);

            // Count empty files (excluding always-empty files)
            let empty_files = files_in_month
                .map(|f| {
                    f.iter()
                        .filter(|e| e.size < tiny_threshold && !always_empty.contains(&e.name))
                        .count()
                })
                .unwrap_or(0);

            let actual_archives = archive_dates.map(|d| d.len()).unwrap_or(0);
            let missing_days = missing_by_month.get(&(year, month)).copied().unwrap_or(0);
            let anomaly_count = anomalies_by_month.get(&(year, month)).copied().unwrap_or(0);

            // Determine if month is complete
            let month_end = if month == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1)
                    .unwrap()
                    .pred_opt()
                    .unwrap()
            } else {
                NaiveDate::from_ymd_opt(year, month + 1, 1)
                    .unwrap()
                    .pred_opt()
                    .unwrap()
            };
            let is_complete = today > month_end;

            let mut metrics = MonthlyMetrics {
                year,
                month,
                missing_days,
                anomaly_count,
                invalid_files,
                empty_files,
                expected_weekdays,
                actual_archives,
                health_score: 0.0, // Calculated below
                is_complete,
            };

            metrics.health_score = calculate_health_score(&metrics);
            Some(metrics)
        })
        .collect();

    // Sort by health score descending (best first)
    months.sort_by(|a, b| {
        b.health_score
            .partial_cmp(&a.health_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Find best and worst months (only among complete months if possible)
    let complete_months: Vec<_> = months.iter().filter(|m| m.is_complete).collect();
    let (best_month, worst_month) = if complete_months.is_empty() {
        (months.first().cloned(), months.last().cloned())
    } else {
        (
            complete_months.first().cloned().cloned(),
            complete_months.last().cloned().cloned(),
        )
    };

    // Calculate average score
    let average_score = if months.is_empty() {
        0.0
    } else {
        months.iter().map(|m| m.health_score).sum::<f64>() / months.len() as f64
    };

    MonthlyRankingReport {
        months,
        best_month,
        worst_month,
        average_score,
        line_id: line_id.to_string(),
    }
}

/// Print monthly rankings to terminal
pub fn print_monthly_rankings(report: &MonthlyRankingReport) {
    if report.months.is_empty() {
        println!("No monthly data available for ranking.");
        return;
    }

    // Print best/worst summary
    if let Some(ref best) = report.best_month {
        println!(
            "Best Month:  {}-{:02} (Score: {:.1}%)",
            best.year, best.month, best.health_score
        );
    }
    if let Some(ref worst) = report.worst_month {
        println!(
            "Worst Month: {}-{:02} (Score: {:.1}%)",
            worst.year, worst.month, worst.health_score
        );
    }
    println!("Average Score: {:.1}%", report.average_score);

    // Create ranking table
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_HORIZONTAL_ONLY);
    table.set_header(vec![
        "Month", "Score", "Missing", "Anomalies", "Invalid", "Empty", "Archives", "Status",
    ]);

    for metrics in &report.months {
        let month_str = format!("{}-{:02}", metrics.year, metrics.month);
        let score_str = format!("{:.1}%", metrics.health_score);
        let status = if metrics.is_complete {
            "Complete"
        } else {
            "Partial"
        };

        // Color code the score
        let score_color = if metrics.health_score >= 90.0 {
            Color::Green
        } else if metrics.health_score >= 70.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let row = vec![
            Cell::new(&month_str),
            Cell::new(&score_str).fg(score_color).add_attribute(Attribute::Bold),
            Cell::new(metrics.missing_days).fg(if metrics.missing_days > 0 {
                Color::Yellow
            } else {
                Color::White
            }),
            Cell::new(metrics.anomaly_count).fg(if metrics.anomaly_count > 0 {
                Color::Yellow
            } else {
                Color::White
            }),
            Cell::new(metrics.invalid_files).fg(if metrics.invalid_files > 0 {
                Color::Red
            } else {
                Color::White
            }),
            Cell::new(metrics.empty_files).fg(if metrics.empty_files > 0 {
                Color::Yellow
            } else {
                Color::White
            }),
            Cell::new(format!("{}/{}", metrics.actual_archives, metrics.expected_weekdays)),
            Cell::new(status).fg(if metrics.is_complete {
                Color::White
            } else {
                Color::Cyan
            }),
        ];
        table.add_row(row);
    }

    println!("\n{table}");
}

/// Combined metrics for a month across both lines
#[derive(Debug, Clone)]
pub struct CombinedMonthlyMetrics {
    pub year: i32,
    pub month: u32,
    pub line_a_score: Option<f64>,
    pub line_b_score: Option<f64>,
    pub combined_score: f64,
    pub line_a_metrics: Option<MonthlyMetrics>,
    pub line_b_metrics: Option<MonthlyMetrics>,
}

/// Combined ranking report for both lines
#[derive(Debug)]
pub struct CombinedRankingReport {
    pub months: Vec<CombinedMonthlyMetrics>,
    pub best_month: Option<CombinedMonthlyMetrics>,
    pub worst_month: Option<CombinedMonthlyMetrics>,
    pub line_a_average: f64,
    pub line_b_average: f64,
    pub combined_average: f64,
}

/// Combine rankings from both lines into a unified report
#[must_use]
pub fn combine_rankings(
    line_a: &MonthlyRankingReport,
    line_b: &MonthlyRankingReport,
) -> CombinedRankingReport {
    // Build lookup maps for each line
    let line_a_map: HashMap<(i32, u32), &MonthlyMetrics> = line_a
        .months
        .iter()
        .map(|m| ((m.year, m.month), m))
        .collect();

    let line_b_map: HashMap<(i32, u32), &MonthlyMetrics> = line_b
        .months
        .iter()
        .map(|m| ((m.year, m.month), m))
        .collect();

    // Collect all unique months
    let mut all_months: HashSet<(i32, u32)> = line_a_map.keys().copied().collect();
    all_months.extend(line_b_map.keys().copied());

    // Build combined metrics
    let mut months: Vec<CombinedMonthlyMetrics> = all_months
        .into_iter()
        .map(|(year, month)| {
            let a_metrics = line_a_map.get(&(year, month)).copied().cloned();
            let b_metrics = line_b_map.get(&(year, month)).copied().cloned();

            let a_score = a_metrics.as_ref().map(|m| m.health_score);
            let b_score = b_metrics.as_ref().map(|m| m.health_score);

            // Combined score: average of available scores
            let combined_score = match (a_score, b_score) {
                (Some(a), Some(b)) => (a + b) / 2.0,
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => 0.0,
            };

            CombinedMonthlyMetrics {
                year,
                month,
                line_a_score: a_score,
                line_b_score: b_score,
                combined_score,
                line_a_metrics: a_metrics,
                line_b_metrics: b_metrics,
            }
        })
        .collect();

    // Sort by combined score descending (best first)
    months.sort_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Find best and worst (only among months with data from both lines, if possible)
    let both_lines: Vec<_> = months
        .iter()
        .filter(|m| m.line_a_score.is_some() && m.line_b_score.is_some())
        .collect();

    let (best_month, worst_month) = if both_lines.is_empty() {
        (months.first().cloned(), months.last().cloned())
    } else {
        (
            both_lines.first().cloned().cloned(),
            both_lines.last().cloned().cloned(),
        )
    };

    CombinedRankingReport {
        months,
        best_month,
        worst_month,
        line_a_average: line_a.average_score,
        line_b_average: line_b.average_score,
        combined_average: (line_a.average_score + line_b.average_score) / 2.0,
    }
}

/// Print combined rankings to terminal
pub fn print_combined_rankings(report: &CombinedRankingReport) {
    if report.months.is_empty() {
        println!("No monthly data available for combined ranking.");
        return;
    }

    // Print summary
    println!("Line A Average: {:.1}%", report.line_a_average);
    println!("Line B Average: {:.1}%", report.line_b_average);
    println!("Combined Average: {:.1}%", report.combined_average);

    if let Some(ref best) = report.best_month {
        println!(
            "\nBest Month (Combined):  {}-{:02} (Score: {:.1}%)",
            best.year, best.month, best.combined_score
        );
    }
    if let Some(ref worst) = report.worst_month {
        println!(
            "Worst Month (Combined): {}-{:02} (Score: {:.1}%)",
            worst.year, worst.month, worst.combined_score
        );
    }

    // Create combined table
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_HORIZONTAL_ONLY);
    table.set_header(vec![
        "Month",
        "Combined",
        "Line A",
        "Line B",
        "A Invalid",
        "B Invalid",
        "A Missing",
        "B Missing",
    ]);

    for m in &report.months {
        let month_str = format!("{}-{:02}", m.year, m.month);

        let combined_str = format!("{:.1}%", m.combined_score);
        let a_str = m
            .line_a_score
            .map(|s| format!("{:.1}%", s))
            .unwrap_or_else(|| "-".to_string());
        let b_str = m
            .line_b_score
            .map(|s| format!("{:.1}%", s))
            .unwrap_or_else(|| "-".to_string());

        let a_invalid = m
            .line_a_metrics
            .as_ref()
            .map(|m| m.invalid_files.to_string())
            .unwrap_or_else(|| "-".to_string());
        let b_invalid = m
            .line_b_metrics
            .as_ref()
            .map(|m| m.invalid_files.to_string())
            .unwrap_or_else(|| "-".to_string());
        let a_missing = m
            .line_a_metrics
            .as_ref()
            .map(|m| m.missing_days.to_string())
            .unwrap_or_else(|| "-".to_string());
        let b_missing = m
            .line_b_metrics
            .as_ref()
            .map(|m| m.missing_days.to_string())
            .unwrap_or_else(|| "-".to_string());

        // Color code the combined score
        let combined_color = if m.combined_score >= 90.0 {
            Color::Green
        } else if m.combined_score >= 70.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let a_color = m.line_a_score.map_or(Color::White, |s| {
            if s >= 90.0 {
                Color::Green
            } else if s >= 70.0 {
                Color::Yellow
            } else {
                Color::Red
            }
        });

        let b_color = m.line_b_score.map_or(Color::White, |s| {
            if s >= 90.0 {
                Color::Green
            } else if s >= 70.0 {
                Color::Yellow
            } else {
                Color::Red
            }
        });

        let row = vec![
            Cell::new(&month_str),
            Cell::new(&combined_str)
                .fg(combined_color)
                .add_attribute(Attribute::Bold),
            Cell::new(&a_str).fg(a_color),
            Cell::new(&b_str).fg(b_color),
            Cell::new(&a_invalid).fg(
                if m.line_a_metrics
                    .as_ref()
                    .map_or(false, |m| m.invalid_files > 0)
                {
                    Color::Red
                } else {
                    Color::White
                },
            ),
            Cell::new(&b_invalid).fg(
                if m.line_b_metrics
                    .as_ref()
                    .map_or(false, |m| m.invalid_files > 0)
                {
                    Color::Red
                } else {
                    Color::White
                },
            ),
            Cell::new(&a_missing).fg(
                if m.line_a_metrics
                    .as_ref()
                    .map_or(false, |m| m.missing_days > 0)
                {
                    Color::Yellow
                } else {
                    Color::White
                },
            ),
            Cell::new(&b_missing).fg(
                if m.line_b_metrics
                    .as_ref()
                    .map_or(false, |m| m.missing_days > 0)
                {
                    Color::Yellow
                } else {
                    Color::White
                },
            ),
        ];
        table.add_row(row);
    }

    println!("\n{table}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, Utc};

    fn make_test_entry(
        date: &str,
        line_id: &str,
        name: &str,
        is_valid: bool,
        size: u64,
    ) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            size,
            is_valid,
            invalid_reason: if is_valid {
                None
            } else {
                Some("Test error".to_string())
            },
            modified: Utc::now().with_timezone(&Local),
            parent_dir: format!("Archive_Beam_{}_{}", line_id, date),
        }
    }

    #[test]
    fn test_extract_date_from_dir() {
        assert_eq!(
            extract_date_from_dir("Archive_Beam_B_2024-10-15", "B"),
            Some(NaiveDate::from_ymd_opt(2024, 10, 15).unwrap())
        );
        assert_eq!(
            extract_date_from_dir("Archive_Beam_A_2024-07-29", "A"),
            Some(NaiveDate::from_ymd_opt(2024, 7, 29).unwrap())
        );
        // Wrong line ID
        assert_eq!(extract_date_from_dir("Archive_Beam_B_2024-10-15", "A"), None);
        // Invalid format
        assert_eq!(extract_date_from_dir("SomeOtherDir", "B"), None);
    }

    #[test]
    fn test_count_expected_weekdays_full_month() {
        // October 2024 has 23 weekdays (full month)
        let start = NaiveDate::from_ymd_opt(2024, 10, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 10, 31).unwrap();
        let today = NaiveDate::from_ymd_opt(2024, 11, 1).unwrap();
        assert_eq!(
            count_expected_weekdays_in_month(2024, 10, start, end, today),
            23
        );
    }

    #[test]
    fn test_count_expected_weekdays_partial_start() {
        // July 2024 starting from 29th (Mon) to 31st (Wed) = 3 weekdays
        let start = NaiveDate::from_ymd_opt(2024, 7, 29).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let today = NaiveDate::from_ymd_opt(2024, 8, 15).unwrap();
        assert_eq!(
            count_expected_weekdays_in_month(2024, 7, start, end, today),
            3
        );
    }

    #[test]
    fn test_count_expected_weekdays_current_month() {
        // If today is mid-month, only count up to today
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        // Today is Jan 15, 2024 (Monday)
        let today = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        // Jan 1-15, 2024: Jan 1 (Mon), 2 (Tue), 3 (Wed), 4 (Thu), 5 (Fri),
        // 8 (Mon), 9 (Tue), 10 (Wed), 11 (Thu), 12 (Fri), 15 (Mon) = 11 weekdays
        assert_eq!(
            count_expected_weekdays_in_month(2024, 1, start, end, today),
            11
        );
    }

    #[test]
    fn test_count_expected_weekdays_future_month() {
        // Future month should return 0
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let today = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
        assert_eq!(
            count_expected_weekdays_in_month(2024, 12, start, end, today),
            0
        );
    }

    #[test]
    fn test_find_always_empty_files() {
        let files = vec![
            make_test_entry("2024-10-01", "B", "normal.zip", true, 5000),
            make_test_entry("2024-10-01", "B", "always_empty.zip", true, 22),
            make_test_entry("2024-10-02", "B", "normal.zip", true, 5100),
            make_test_entry("2024-10-02", "B", "always_empty.zip", true, 22),
            make_test_entry("2024-10-01", "B", "sometimes_empty.zip", true, 22),
            make_test_entry("2024-10-02", "B", "sometimes_empty.zip", true, 5000),
        ];

        let always_empty = find_always_empty_files(&files, 1000);
        assert!(always_empty.contains("always_empty.zip"));
        assert!(!always_empty.contains("normal.zip"));
        assert!(!always_empty.contains("sometimes_empty.zip"));
    }

    #[test]
    fn test_perfect_month_score() {
        let metrics = MonthlyMetrics {
            year: 2024,
            month: 8,
            missing_days: 0,
            anomaly_count: 0,
            invalid_files: 0,
            empty_files: 0,
            expected_weekdays: 22,
            actual_archives: 22,
            health_score: 0.0,
            is_complete: true,
        };

        let score = calculate_health_score(&metrics);
        assert!((score - 100.0).abs() < 0.001, "Perfect month should score 100.0");
    }

    #[test]
    fn test_all_missing_month_score() {
        let metrics = MonthlyMetrics {
            year: 2024,
            month: 12,
            missing_days: 22,
            anomaly_count: 0,
            invalid_files: 0,
            empty_files: 0,
            expected_weekdays: 22,
            actual_archives: 0,
            health_score: 0.0,
            is_complete: true,
        };

        let score = calculate_health_score(&metrics);
        assert!(score <= 0.0, "All missing month should score 0.0 or less");
    }

    #[test]
    fn test_partial_issues_score() {
        // Month with some issues should score between 0 and 100
        let metrics = MonthlyMetrics {
            year: 2024,
            month: 10,
            missing_days: 2,
            anomaly_count: 3,
            invalid_files: 5,
            empty_files: 4,
            expected_weekdays: 23,
            actual_archives: 21,
            health_score: 0.0,
            is_complete: true,
        };

        let score = calculate_health_score(&metrics);
        // penalty = 2*10 + 5*3 + 3*2 + 4*1 = 20 + 15 + 6 + 4 = 45
        // max_penalty = 23 * 10 = 230
        // score = 100 * (1 - 45/230) = 100 * 0.804 = 80.4
        assert!(score > 0.0 && score < 100.0, "Partial issues should score between 0 and 100");
        assert!((score - 80.4).abs() < 1.0, "Expected score around 80.4, got {}", score);
    }

    #[test]
    fn test_always_empty_files_excluded_from_scoring() {
        // Create files where one file is always empty and another sometimes empty
        let files = vec![
            // Archive 1
            make_test_entry("2024-10-01", "B", "DeviceCptr26.zip", true, 22),
            make_test_entry("2024-10-01", "B", "normal.zip", true, 5000),
            make_test_entry("2024-10-01", "B", "sometimes_empty.zip", true, 22),
            // Archive 2
            make_test_entry("2024-10-02", "B", "DeviceCptr26.zip", true, 22),
            make_test_entry("2024-10-02", "B", "normal.zip", true, 5100),
            make_test_entry("2024-10-02", "B", "sometimes_empty.zip", true, 5000),
        ];

        let start = NaiveDate::from_ymd_opt(2024, 10, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 10, 2).unwrap();

        let report = calculate_monthly_rankings(&files, None, None, "B", start, end, 1000);

        // Should have October 2024
        assert_eq!(report.months.len(), 1);
        let oct = &report.months[0];
        assert_eq!(oct.year, 2024);
        assert_eq!(oct.month, 10);

        // DeviceCptr26.zip is always empty (2 files, both empty) - should be excluded
        // sometimes_empty.zip is empty once but not always - should count as 1 empty
        assert_eq!(oct.empty_files, 1, "Only sometimes_empty.zip should count as empty");
    }

    #[test]
    fn test_monthly_ranking_calculation() {
        let files = vec![
            // October 2024 - 2 archives, 1 invalid file
            make_test_entry("2024-10-01", "B", "test.zip", true, 5000),
            make_test_entry("2024-10-01", "B", "bad.zip", false, 100),
            make_test_entry("2024-10-02", "B", "test.zip", true, 5000),
            // November 2024 - 1 archive, clean
            make_test_entry("2024-11-01", "B", "test.zip", true, 5000),
        ];

        let start = NaiveDate::from_ymd_opt(2024, 10, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 11, 30).unwrap();

        let report = calculate_monthly_rankings(&files, None, None, "B", start, end, 1000);

        assert_eq!(report.months.len(), 2);
        assert_eq!(report.line_id, "B");

        // November should rank higher (no invalid files)
        let nov = report.months.iter().find(|m| m.month == 11).unwrap();
        let oct = report.months.iter().find(|m| m.month == 10).unwrap();
        assert!(nov.health_score > oct.health_score);
    }

    #[test]
    fn test_combine_rankings() {
        // Create Line A report with Oct=80%, Nov=90%
        let line_a = MonthlyRankingReport {
            months: vec![
                MonthlyMetrics {
                    year: 2024,
                    month: 10,
                    missing_days: 0,
                    anomaly_count: 0,
                    invalid_files: 2,
                    empty_files: 0,
                    expected_weekdays: 23,
                    actual_archives: 23,
                    health_score: 80.0,
                    is_complete: true,
                },
                MonthlyMetrics {
                    year: 2024,
                    month: 11,
                    missing_days: 0,
                    anomaly_count: 0,
                    invalid_files: 0,
                    empty_files: 0,
                    expected_weekdays: 21,
                    actual_archives: 21,
                    health_score: 90.0,
                    is_complete: true,
                },
            ],
            best_month: None,
            worst_month: None,
            average_score: 85.0,
            line_id: "A".to_string(),
        };

        // Create Line B report with Oct=70%, Nov=100%
        let line_b = MonthlyRankingReport {
            months: vec![
                MonthlyMetrics {
                    year: 2024,
                    month: 10,
                    missing_days: 0,
                    anomaly_count: 0,
                    invalid_files: 5,
                    empty_files: 0,
                    expected_weekdays: 23,
                    actual_archives: 23,
                    health_score: 70.0,
                    is_complete: true,
                },
                MonthlyMetrics {
                    year: 2024,
                    month: 11,
                    missing_days: 0,
                    anomaly_count: 0,
                    invalid_files: 0,
                    empty_files: 0,
                    expected_weekdays: 21,
                    actual_archives: 21,
                    health_score: 100.0,
                    is_complete: true,
                },
            ],
            best_month: None,
            worst_month: None,
            average_score: 85.0,
            line_id: "B".to_string(),
        };

        let combined = combine_rankings(&line_a, &line_b);

        assert_eq!(combined.months.len(), 2);

        // November combined = (90 + 100) / 2 = 95
        let nov = combined.months.iter().find(|m| m.month == 11).unwrap();
        assert!((nov.combined_score - 95.0).abs() < 0.1);

        // October combined = (80 + 70) / 2 = 75
        let oct = combined.months.iter().find(|m| m.month == 10).unwrap();
        assert!((oct.combined_score - 75.0).abs() < 0.1);

        // Best month should be November
        assert!(combined.best_month.is_some());
        assert_eq!(combined.best_month.as_ref().unwrap().month, 11);

        // Worst month should be October
        assert!(combined.worst_month.is_some());
        assert_eq!(combined.worst_month.as_ref().unwrap().month, 10);
    }

    #[test]
    fn test_combine_rankings_partial_data() {
        // Line A has Oct and Nov, Line B only has Nov
        let line_a = MonthlyRankingReport {
            months: vec![
                MonthlyMetrics {
                    year: 2024,
                    month: 10,
                    missing_days: 0,
                    anomaly_count: 0,
                    invalid_files: 0,
                    empty_files: 0,
                    expected_weekdays: 23,
                    actual_archives: 23,
                    health_score: 100.0,
                    is_complete: true,
                },
                MonthlyMetrics {
                    year: 2024,
                    month: 11,
                    missing_days: 0,
                    anomaly_count: 0,
                    invalid_files: 0,
                    empty_files: 0,
                    expected_weekdays: 21,
                    actual_archives: 21,
                    health_score: 90.0,
                    is_complete: true,
                },
            ],
            best_month: None,
            worst_month: None,
            average_score: 95.0,
            line_id: "A".to_string(),
        };

        let line_b = MonthlyRankingReport {
            months: vec![MonthlyMetrics {
                year: 2024,
                month: 11,
                missing_days: 0,
                anomaly_count: 0,
                invalid_files: 0,
                empty_files: 0,
                expected_weekdays: 21,
                actual_archives: 21,
                health_score: 80.0,
                is_complete: true,
            }],
            best_month: None,
            worst_month: None,
            average_score: 80.0,
            line_id: "B".to_string(),
        };

        let combined = combine_rankings(&line_a, &line_b);

        assert_eq!(combined.months.len(), 2);

        // October only has Line A data, so combined = 100
        let oct = combined.months.iter().find(|m| m.month == 10).unwrap();
        assert!(oct.line_a_score.is_some());
        assert!(oct.line_b_score.is_none());
        assert!((oct.combined_score - 100.0).abs() < 0.1);

        // November has both, combined = (90 + 80) / 2 = 85
        let nov = combined.months.iter().find(|m| m.month == 11).unwrap();
        assert!(nov.line_a_score.is_some());
        assert!(nov.line_b_score.is_some());
        assert!((nov.combined_score - 85.0).abs() < 0.1);
    }
}
