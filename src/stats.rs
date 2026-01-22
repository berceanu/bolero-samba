use crate::types::FileEntry;
use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, Table};
use std::collections::HashMap;

#[derive(Debug)]
pub struct IntegrityRow {
    pub name: String,
    pub total: usize,
    pub empty: usize,
    pub bad: usize,
    pub min_size: u64,
    pub max_size: u64,
    pub median_size: u64,
    pub std_dev: f64,
    pub valid_stats: bool,
}

#[derive(Debug)]
pub struct IntegrityStats {
    pub rows: Vec<IntegrityRow>,
    pub grand_total: usize,
    pub grand_empty: usize,
    pub grand_bad: usize,
    pub grand_min: u64,
    pub grand_max: u64,
    pub grand_median: u64,
    pub grand_std_dev: f64,
}

#[derive(Debug)]
pub struct Anomaly {
    pub name: String,
    pub size: u64,
    pub category: String, // "Too Small" or "Too Large"
}

#[derive(Debug)]
pub struct AnomalyReport {
    pub median_daily_size: u64,
    pub anomalies: Vec<Anomaly>,
}

#[derive(Debug, Clone)]
pub struct BadFile {
    pub relative_path: String,
    pub size: u64,
    pub reason: String,
}

#[derive(Debug)]
pub struct BadFilesReport {
    pub total_count: usize,
    pub files_by_folder: Vec<(String, Vec<BadFile>, usize)>, // (folder_name, displayed_files, total_in_folder)
}

#[must_use]
pub fn calculate_integrity_stats(
    files: &[FileEntry],
    tiny_threshold: u64,
) -> Option<IntegrityStats> {
    if files.is_empty() {
        return None;
    }

    // Files are already filtered to exclude growing directories
    let mut groups: HashMap<String, Vec<&FileEntry>> = HashMap::new();
    for f in files {
        groups.entry(f.name.clone()).or_default().push(f);
    }

    let mut sorted_keys: Vec<_> = groups.keys().cloned().collect();
    sorted_keys.sort();

    let mut rows = Vec::new();
    let mut grand_total = 0;
    let mut grand_empty = 0;
    let mut grand_bad = 0;
    let mut grand_min = u64::MAX;
    let mut grand_max = 0;

    let mut all_medians = Vec::new();
    let mut all_stddevs = Vec::new();
    let mut valid_groups = 0;

    for name in sorted_keys {
        let entries = groups.get(&name).unwrap();
        let total = entries.len();
        let empty = entries.iter().filter(|e| e.size < tiny_threshold).count();
        let bad = entries.iter().filter(|e| !e.is_valid).count();

        let mut sizes: Vec<f64> = entries
            .iter()
            .filter(|e| e.size >= tiny_threshold)
            .map(|e| e.size as f64)
            .collect();
        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());

        grand_total += total;
        grand_empty += empty;
        grand_bad += bad;

        let row = if sizes.is_empty() {
            IntegrityRow {
                name,
                total,
                empty,
                bad,
                min_size: 0,
                max_size: 0,
                median_size: 0,
                std_dev: 0.0,
                valid_stats: false,
            }
        } else {
            let min = sizes[0];
            let max = *sizes.last().unwrap();
            let count = sizes.len();
            let sum: f64 = sizes.iter().sum();
            let mean = sum / count as f64;

            let median = if count % 2 == 1 {
                sizes[count / 2]
            } else {
                f64::midpoint(sizes[count / 2 - 1], sizes[count / 2])
            };

            let variance: f64 =
                sizes.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / count as f64;
            let std_dev = variance.sqrt();

            if min < grand_min as f64 {
                grand_min = min as u64;
            }
            if max > grand_max as f64 {
                grand_max = max as u64;
            }

            all_medians.push(median);
            all_stddevs.push(std_dev);
            valid_groups += 1;

            IntegrityRow {
                name,
                total,
                empty,
                bad,
                min_size: min as u64,
                max_size: max as u64,
                median_size: median as u64,
                std_dev,
                valid_stats: true,
            }
        };
        rows.push(row);
    }

    // Grand Stats
    let (grand_median, grand_std_dev) = if valid_groups > 0 {
        all_medians.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_stddevs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let gm = if valid_groups % 2 == 1 {
            all_medians[valid_groups / 2]
        } else {
            f64::midpoint(
                all_medians[valid_groups / 2 - 1],
                all_medians[valid_groups / 2],
            )
        };

        let gsd = if valid_groups % 2 == 1 {
            all_stddevs[valid_groups / 2]
        } else {
            f64::midpoint(
                all_stddevs[valid_groups / 2 - 1],
                all_stddevs[valid_groups / 2],
            )
        };
        (gm as u64, gsd)
    } else {
        (0, 0.0)
    };

    if grand_min == u64::MAX {
        grand_min = 0;
    }

    Some(IntegrityStats {
        rows,
        grand_total,
        grand_empty,
        grand_bad,
        grand_min,
        grand_max,
        grand_median,
        grand_std_dev,
    })
}

pub fn print_integrity_table(stats_opt: &Option<IntegrityStats>) {
    let stats = if let Some(s) = stats_opt {
        s
    } else {
        println!("No zip files found.");
        return;
    };

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_HORIZONTAL_ONLY);
    table.set_header(vec![
        "Filename", "Total", "Empty", "Bad", "Min", "Max", "Median", "StdDev",
    ]);

    for row in &stats.rows {
        let (min_s, max_s, median_s, std_s) = if row.valid_stats {
            (
                human_bytes::human_bytes(row.min_size as f64),
                human_bytes::human_bytes(row.max_size as f64),
                human_bytes::human_bytes(row.median_size as f64),
                human_bytes::human_bytes(row.std_dev),
            )
        } else {
            (
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
            )
        };

        let t_row = vec![
            Cell::new(&row.name),
            Cell::new(row.total),
            Cell::new(row.empty).fg(if row.empty > 0 {
                Color::Yellow
            } else {
                Color::White
            }),
            Cell::new(row.bad).fg(if row.bad > 0 {
                Color::Red
            } else {
                Color::White
            }),
            Cell::new(min_s),
            Cell::new(max_s),
            Cell::new(median_s),
            Cell::new(std_s),
        ];
        table.add_row(t_row);
    }

    // Summary Row
    table.add_row(vec![
        Cell::new("TOTALS / SUMMARY").add_attribute(Attribute::Bold),
        Cell::new(stats.grand_total).add_attribute(Attribute::Bold),
        Cell::new(stats.grand_empty)
            .fg(if stats.grand_empty > 0 {
                Color::Yellow
            } else {
                Color::White
            })
            .add_attribute(Attribute::Bold),
        Cell::new(if stats.grand_bad > 0 {
            format!("⚠️ {}", stats.grand_bad)
        } else {
            stats.grand_bad.to_string()
        })
        .fg(if stats.grand_bad > 0 {
            Color::Red
        } else {
            Color::White
        })
        .add_attribute(Attribute::Bold),
        Cell::new(human_bytes::human_bytes(stats.grand_min as f64)).add_attribute(Attribute::Bold),
        Cell::new(human_bytes::human_bytes(stats.grand_max as f64)).add_attribute(Attribute::Bold),
        Cell::new(human_bytes::human_bytes(stats.grand_median as f64))
            .add_attribute(Attribute::Bold),
        Cell::new(human_bytes::human_bytes(stats.grand_std_dev)).add_attribute(Attribute::Bold),
    ]);

    println!("{table}");
}

#[must_use]
pub fn calculate_anomalies(dirs: &HashMap<String, u64>, threshold: f64) -> Option<AnomalyReport> {
    let mut sizes: Vec<u64> = dirs.values().copied().collect();
    if sizes.is_empty() {
        return None;
    }
    sizes.sort_unstable();

    let count = sizes.len();
    let median = if count % 2 == 1 {
        sizes[count / 2]
    } else {
        u64::midpoint(sizes[count / 2 - 1], sizes[count / 2])
    };

    let mut anomalies = Vec::new();
    let mut sorted_dirs: Vec<_> = dirs.iter().collect();
    sorted_dirs.sort_by_key(|k| k.0.clone());

    for (name, size) in sorted_dirs {
        let s = *size as f64;
        let m = median as f64;

        if s < m * threshold {
            anomalies.push(Anomaly {
                name: name.clone(),
                size: *size,
                category: "Too Small".to_string(),
            });
        } else if s > m * 1.2 {
            anomalies.push(Anomaly {
                name: name.clone(),
                size: *size,
                category: "Too Large".to_string(),
            });
        }
    }

    Some(AnomalyReport {
        median_daily_size: median,
        anomalies,
    })
}

pub fn print_anomalies(report: &Option<AnomalyReport>) {
    let r = if let Some(r) = report {
        r
    } else {
        println!("No completed directories.");
        return;
    };

    println!(
        "Median Size: {}",
        human_bytes::human_bytes(r.median_daily_size as f64)
    );
    println!("-------------------------------------------------------------------------------");

    if r.anomalies.is_empty() {
        println!("No significant size anomalies found.");
    } else {
        for a in &r.anomalies {
            let _color = if a.category == "Too Small" {
                Color::Yellow
            } else {
                Color::Red
            }; // Match logic: Small=Yellow? No wait, Bash used Red for small.
            // Bash: Small=Red, Large=Yellow. Let's match Bash.
            // Bash script: if (sizes[i] < median * 0.8) red... else if > 1.2 yellow

            let _color_code = if a.category == "Too Small" {
                "red"
            } else {
                "yellow"
            }; // Reserved for future HTML styling

            let size_str = human_bytes::human_bytes(a.size as f64);
            // We use println! directly, can't easily pass Color enum to format! macro without crate
            // Let's just use colored crate
            if a.category == "Too Small" {
                println!(
                    "⚠️ {:<35} | {:<10} | ({})",
                    a.name,
                    size_str.red(),
                    a.category
                );
            } else {
                println!(
                    "⚠️ {:<35} | {:<10} | ({})",
                    a.name,
                    size_str.yellow(),
                    a.category
                );
            }
        }
    }
}

#[must_use]
pub fn collect_bad_files(
    files: &[FileEntry],
    line_id: &str,
    max_per_archive: usize,
) -> Option<BadFilesReport> {
    // Files are already filtered to exclude growing directories
    let bad_files: Vec<&FileEntry> = files.iter().filter(|f| !f.is_valid).collect();

    if bad_files.is_empty() {
        return None;
    }

    let total_count = bad_files.len();

    // Group by parent_dir (folder) with all bad files
    let mut by_folder: HashMap<String, Vec<BadFile>> = HashMap::new();

    for file in bad_files {
        let relative_path = format!("Line {}/{}/{}", line_id, file.parent_dir, file.name);
        let reason = file
            .invalid_reason
            .clone()
            .unwrap_or_else(|| "Unknown error".to_string());

        let bad_file = BadFile {
            relative_path,
            size: file.size,
            reason,
        };

        by_folder
            .entry(file.parent_dir.clone())
            .or_default()
            .push(bad_file);
    }

    // Convert to sorted vec and apply per-directory truncation
    let mut files_by_folder: Vec<(String, Vec<BadFile>, usize)> = by_folder
        .into_iter()
        .map(|(dir, mut files)| {
            let total_in_dir = files.len();
            // Sort lexicographically by relative_path
            files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
            // Truncate to max_per_archive
            files.truncate(max_per_archive);
            (dir, files, total_in_dir)
        })
        .collect();
    // Sort directories lexicographically
    files_by_folder.sort_by(|a, b| a.0.cmp(&b.0));

    Some(BadFilesReport {
        total_count,
        files_by_folder,
    })
}

pub fn print_bad_files(report: &Option<BadFilesReport>, threshold: usize) {
    if let Some(r) = report {
        println!("\n{}", "=== Bad ZIP Files ===".cyan());

        let archive_count = r.files_by_folder.len();
        println!(
            "Found {} bad ZIP files across {} archives (showing archives with >{} bad files):",
            r.total_count, archive_count, threshold
        );

        // Filter to show only archives with more than threshold bad files
        for (folder, files, total_in_dir) in r.files_by_folder.iter().filter(|(_, _, count)| *count > threshold) {
            // Display count based on whether truncation occurred
            if *total_in_dir > files.len() {
                println!(
                    "\n{} ({} bad files, showing first {})",
                    folder.yellow(),
                    total_in_dir,
                    files.len()
                );
            } else {
                println!("\n{} ({} bad files)", folder.yellow(), total_in_dir);
            }

            for file in files {
                println!("  {} {}", "⚠️".yellow(), file.relative_path);
                println!("     Size: {}", human_bytes::human_bytes(file.size as f64));
                println!("     Reason: {}", file.reason.red());
            }

            // Show truncation message if needed
            if *total_in_dir > files.len() {
                let remaining = total_in_dir - files.len();
                println!("  ... {} more bad files in this archive", remaining);
            }
        }
    }
}
