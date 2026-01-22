mod email;
mod estimates;
mod gap_analysis;
mod html_renderer;
mod scanner;
mod stats;
mod system_io;
mod types;

use chrono::Local;
use clap::Parser;
use colored::Colorize;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Line ID (A or B)
    #[arg(default_value = "B")]
    line_id: String,

    /// Output HTML instead of terminal colors
    #[arg(long, short = 'H')]
    html: bool,

    /// Generate full dashboard HTML for both lines (writes to file)
    #[arg(long, short = 'd', value_name = "FILE")]
    dashboard: Option<String>,

    /// Base directory containing Line A/B folders
    #[arg(long, short = 'b', default_value = "/data/storage/samba_share_cluster")]
    base_dir: String,

    /// Test email configuration by sending a test email
    #[arg(long)]
    test_email: bool,

    /// Minimum minutes a state must persist before sending email alert
    #[arg(long, default_value_t = 20)]
    alert_threshold: u64,
}

fn main() {
    let args = Args::parse();

    // Test email mode
    if args.test_email {
        test_email_config(&args.base_dir);
        return;
    }

    // Dashboard mode - generate both lines
    if let Some(output_file) = &args.dashboard {
        generate_dashboard(output_file, &args.base_dir, args.alert_threshold);
        return;
    }

    let line_id = args.line_id.to_uppercase();

    if line_id != "A" && line_id != "B" {
        eprintln!("Error: Invalid Line ID '{line_id}'. Use 'A' or 'B'.");
        std::process::exit(1);
    }

    let search_dir = format!("{}/Line {}", args.base_dir, line_id);
    let tiny_threshold = 1000;

    // Progress messages (only in terminal mode)
    if !args.html {
        println!(
            "{}",
            format!(
                "=== Audit Report for LINE {}: {} ===",
                line_id,
                Local::now().format("%Y-%m-%d %H:%M")
            )
            .cyan()
        );
        println!("Step 1/3: Calculating initial directory size...");
    }

    let size_t1 = scanner::get_total_size(&search_dir);

    if !args.html {
        println!("Step 2/3: Monitoring transfer speed (10s)...");
    }
    thread::sleep(Duration::from_secs(10));
    let size_t2 = scanner::get_total_size(&search_dir);

    if !args.html {
        println!("Step 3/3: Verifying file integrity and analyzing stats...");
    }
    let files = scanner::scan_files(&search_dir);
    let total_zip_files = files.len();

    // Speed Calc
    let delta_bytes = size_t2.saturating_sub(size_t1);
    let speed_bps = delta_bytes / 10;
    let speed_mib = speed_bps as f64 / 1_024.0 / 1_024.0;

    // State Logic
    let state_file = format!("{}/.transfer_state_{}", args.base_dir, line_id);
    let since_file = format!("{}/.transfer_since_{}", args.base_dir, line_id);

    let current_state = if speed_bps > 0 { "ACTIVE" } else { "IDLE" };
    let prev_state = fs::read_to_string(&state_file).unwrap_or_else(|_| "IDLE".to_string());
    let prev_state = prev_state.trim();

    // Timestamp Logic
    if current_state != prev_state || !std::path::Path::new(&since_file).exists() {
        let now_str = Local::now().format("%Y-%m-%d %H:%M").to_string();
        fs::write(&since_file, &now_str).ok();
    }
    let since_ts = fs::read_to_string(&since_file).unwrap_or_default();
    let since_ts = since_ts.trim().to_string();

    // Get recent files
    let recents = scanner::get_recent_files(&search_dir, 5);

    // Redundancy check (only in terminal mode for now)
    let redundancy_check = if speed_bps == 0 && !args.html {
        system_io::check_redundancy(&line_id);
        None
    } else {
        None
    };

    // Alerting (background, runs regardless of output mode)
    check_and_send_alerts(
        &args.base_dir,
        &line_id,
        current_state,
        prev_state,
        speed_mib,
        args.alert_threshold,
    );

    if current_state != prev_state && !args.html {
        println!(
            "\n{}",
            format!("-- State Change Detected ({prev_state} -> {current_state}) --").cyan()
        );
    }

    // Calculate all reports
    let integrity_stats = stats::calculate_integrity_stats(&files, tiny_threshold);
    let gap_report = gap_analysis::find_gaps(&files, &line_id);
    let estimates_report = estimates::calculate_estimates(
        &search_dir,
        &files,
        &line_id,
        size_t2,
        speed_bps,
        &args.base_dir,
    );
    let anomalies_report = stats::calculate_anomalies(&files);
    let bad_files_report = stats::collect_bad_files(&files, &line_id);

    // Output based on mode
    if args.html {
        let report = html_renderer::AuditReport {
            total_size: size_t2,
            total_files: total_zip_files,
            speed_bps,
            since_timestamp: since_ts,
            recent_files: recents,
            redundancy_check,
            integrity_stats,
            gap_report: Some(gap_report),
            estimates_report,
            anomaly_report: anomalies_report,
            bad_files_report,
        };

        println!("{}", html_renderer::render_full_report(&report));
    } else {
        // Terminal output (existing)
        println!(
            "Archive Status:  {} across {} zip files.",
            human_bytes::human_bytes(size_t2 as f64).green(),
            total_zip_files.to_string().green()
        );

        println!("\n{}", "=== Active Transfer Detection ===".cyan());

        if speed_bps > 0 {
            println!(
                "Status:                 {} (since {})",
                "ACTIVE TRANSFER DETECTED".green(),
                since_ts
            );
            println!(
                "Current Transfer Speed: {} MiB/s",
                format!("{speed_mib:.1}").green()
            );
        } else {
            println!(
                "Status:                 {} (since {})",
                "IDLE".yellow(),
                since_ts
            );
        }

        println!("{}", "Active/Recent File Writes (last 5m):".green());
        if recents.is_empty() {
            println!();
        } else {
            for r in recents.iter().take(3) {
                println!("{r}");
            }
            if recents.len() > 3 {
                println!("  ... and {} more files.", recents.len() - 3);
            }
        }

        estimates::print_estimates(&estimates_report);

        println!("\n{}", "=== File Integrity & Heuristics ===".cyan());
        stats::print_integrity_table(&integrity_stats);

        println!("\n{}", "=== Missing Daily Archives ===".cyan());
        gap_analysis::analyze_gaps(&files, &line_id);

        println!("\n{}", "=== Directory Size Anomalies ===".cyan());
        stats::print_anomalies(&anomalies_report);

        let bad_files_report = stats::collect_bad_files(&files, &line_id);
        stats::print_bad_files(&bad_files_report);

        println!("\n{}", "=== Audit Complete ===".cyan());
    }
}

fn generate_dashboard(output_file: &str, base_dir: &str, alert_threshold: u64) {
    // Acquire lock to prevent concurrent runs
    let lockfile = "/tmp/beam_audit_dashboard.lock";
    let _lock = match acquire_lock(lockfile) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Another instance is already running: {e}");
            std::process::exit(1);
        }
    };

    println!("Generating dashboard for both lines...");

    // Generate reports for both lines IN PARALLEL
    let result = std::panic::catch_unwind(|| {
        thread::scope(|s| {
            let handle_a = s.spawn(|| collect_audit_data("A", base_dir, true, alert_threshold));
            let handle_b = s.spawn(|| collect_audit_data("B", base_dir, true, alert_threshold));

            (handle_a.join().unwrap(), handle_b.join().unwrap())
        })
    });

    let (line_a_report, line_b_report) = if let Ok(reports) = result {
        reports
    } else {
        fs::remove_file(lockfile).ok();
        eprintln!("Error during audit data collection");
        std::process::exit(1);
    };

    // Render dashboard HTML
    let html = html_renderer::render_dashboard(&line_a_report, &line_b_report);

    // Write to file
    if let Err(e) = fs::write(output_file, html) {
        eprintln!("Error writing dashboard to {output_file}: {e}");
        fs::remove_file(lockfile).ok();
        std::process::exit(1);
    }

    println!("Dashboard written to: {output_file}");
    fs::remove_file(lockfile).ok();
}

fn acquire_lock(lockfile: &str) -> Result<fs::File, String> {
    use std::io::ErrorKind;

    // Try to create lockfile exclusively (fails if exists)
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(lockfile)
    {
        Ok(mut file) => {
            // Write PID to lockfile
            let pid = std::process::id();
            writeln!(file, "{pid}").ok();
            Ok(file)
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            Err("Lockfile already exists (another instance running)".to_string())
        }
        Err(e) => Err(format!("Failed to create lockfile: {e}")),
    }
}

fn collect_audit_data(
    line_id: &str,
    base_dir: &str,
    silent: bool,
    alert_threshold: u64,
) -> html_renderer::AuditReport {
    let search_dir = format!("{}/Line {}", base_dir, line_id);
    let tiny_threshold = 1000;

    if !silent {
        println!("Step 1/3: Calculating initial directory size...");
    }
    let size_t1 = scanner::get_total_size(&search_dir);

    if !silent {
        println!("Step 2/3: Monitoring transfer speed (10s)...");
    }
    thread::sleep(Duration::from_secs(10));
    let size_t2 = scanner::get_total_size(&search_dir);

    if !silent {
        println!("Step 3/3: Verifying file integrity and analyzing stats...");
    }
    let files = scanner::scan_files(&search_dir);
    let total_zip_files = files.len();

    let delta_bytes = size_t2.saturating_sub(size_t1);
    let speed_bps = delta_bytes / 10;

    let state_file = format!("{}/.transfer_state_{}", base_dir, line_id);
    let since_file = format!("{}/.transfer_since_{}", base_dir, line_id);

    let current_state = if speed_bps > 0 { "ACTIVE" } else { "IDLE" };
    let prev_state = fs::read_to_string(&state_file).unwrap_or_else(|_| "IDLE".to_string());
    let prev_state = prev_state.trim();

    if current_state != prev_state || !std::path::Path::new(&since_file).exists() {
        let now_str = Local::now().format("%Y-%m-%d %H:%M").to_string();
        fs::write(&since_file, &now_str).ok();
    }
    let since_ts = fs::read_to_string(&since_file).unwrap_or_default();
    let since_ts = since_ts.trim().to_string();

    let recents = scanner::get_recent_files(&search_dir, 5);

    // State change handling (email alerts) - use consolidated alert function
    let speed_mib = speed_bps as f64 / 1_024.0 / 1_024.0;
    check_and_send_alerts(
        base_dir,
        line_id,
        current_state,
        prev_state,
        speed_mib,
        alert_threshold,
    );

    let integrity_stats = stats::calculate_integrity_stats(&files, tiny_threshold);
    let gap_report = gap_analysis::find_gaps(&files, line_id);
    let estimates_report =
        estimates::calculate_estimates(&search_dir, &files, line_id, size_t2, speed_bps, base_dir);
    let anomalies_report = stats::calculate_anomalies(&files);
    let bad_files_report = stats::collect_bad_files(&files, line_id);

    // Return AuditReport
    html_renderer::AuditReport {
        total_size: size_t2,
        total_files: total_zip_files,
        speed_bps,
        since_timestamp: since_ts,
        recent_files: recents,
        redundancy_check: None,
        integrity_stats,
        gap_report: Some(gap_report),
        estimates_report,
        anomaly_report: anomalies_report,
        bad_files_report,
    }
}

fn log_state_change(
    base_dir: &str,
    line_id: &str,
    old_state: &str,
    new_state: &str,
    speed_mbps: f64,
) {
    let log_file = format!("{}/.transfer_interruptions_{}", base_dir, line_id);
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_entry = format!(
        "{},{},{},{:.1}\n",
        timestamp, old_state, new_state, speed_mbps
    );

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_file) {
        let _ = file.write_all(log_entry.as_bytes());
    }
}

// ============================================================================
// Pure functions for alert logic (testable without I/O)
// ============================================================================

#[derive(Debug, PartialEq)]
enum AlertAction {
    NoAction,         // State unchanged, no pending alert
    CreateTimestamp,  // State just changed, start tracking
    WaitForThreshold, // Waiting for threshold to pass
    SendAlert {
        // Time to send alert
        minutes_elapsed: i64,
    },
}

struct EmailContent {
    subject: String,
    body: String,
}

struct AlertFiles {
    state_file: String,
    timestamp_file: String,
}

fn get_alert_file_paths(base_dir: &str, line_id: &str) -> AlertFiles {
    AlertFiles {
        state_file: format!("{}/.transfer_state_{}", base_dir, line_id),
        timestamp_file: format!("{}/.transfer_state_changed_{}", base_dir, line_id),
    }
}

fn create_alert_email(
    line_id: &str,
    current_state: &str,
    speed_mib: f64,
    minutes_elapsed: i64,
    current_time: chrono::DateTime<Local>,
) -> EmailContent {
    let action = if current_state == "ACTIVE" {
        "RESUMED"
    } else {
        "STOPPED"
    };
    let action_lower = if current_state == "ACTIVE" {
        "resumed"
    } else {
        "stopped"
    };

    EmailContent {
        subject: format!("[Beam Alert] Transfer {} on Line {}", action, line_id),
        body: format!(
            "The transfer on Line {} has {}.\n\nCurrent Speed: {:.1} MiB/s\nState persisted for: {} minutes\nTime: {}",
            line_id, action_lower, speed_mib, minutes_elapsed, current_time
        ),
    }
}

fn determine_alert_action(
    current_state: &str,
    prev_state: &str,
    timestamp_file_content: Option<&str>,
    threshold_minutes: u64,
    current_time: chrono::DateTime<Local>,
) -> AlertAction {
    // State just changed - create timestamp to start tracking
    if current_state != prev_state {
        return AlertAction::CreateTimestamp;
    }

    // No timestamp file exists and state hasn't changed - nothing to do
    let Some(content) = timestamp_file_content else {
        return AlertAction::NoAction;
    };

    // Try to parse the timestamp and calculate elapsed time
    let Some(minutes_elapsed) = parse_timestamp_and_get_elapsed_minutes(content, current_time)
    else {
        // Invalid timestamp format - treat as no action
        return AlertAction::NoAction;
    };

    // Check if threshold has been met
    if minutes_elapsed >= threshold_minutes as i64 {
        AlertAction::SendAlert { minutes_elapsed }
    } else {
        AlertAction::WaitForThreshold
    }
}

/// Check and send email alerts based on state changes with threshold
fn check_and_send_alerts(
    base_dir: &str,
    line_id: &str,
    current_state: &str,
    prev_state: &str,
    speed_mib: f64,
    alert_threshold: u64,
) {
    let files = get_alert_file_paths(base_dir, line_id);
    let timestamp_content = fs::read_to_string(&files.timestamp_file).ok();

    // Use pure function to determine what action to take
    let action = determine_alert_action(
        current_state,
        prev_state,
        timestamp_content.as_deref(),
        alert_threshold,
        Local::now(),
    );

    // Execute the determined action (this is where I/O happens)
    match action {
        AlertAction::NoAction => {
            // State unchanged, no pending alert - just update state file
            fs::write(&files.state_file, current_state).ok();
        }
        AlertAction::CreateTimestamp => {
            // State just changed - log it and create timestamp
            log_state_change(base_dir, line_id, prev_state, current_state, speed_mib);
            let timestamp_str = Local::now().format("%Y-%m-%d %H:%M").to_string();
            fs::write(&files.timestamp_file, timestamp_str).ok();
            fs::write(&files.state_file, current_state).ok();
        }
        AlertAction::WaitForThreshold => {
            // Waiting for threshold - just update state file
            fs::write(&files.state_file, current_state).ok();
        }
        AlertAction::SendAlert { minutes_elapsed } => {
            // Threshold met - send alert and clean up
            if let Some(cfg) = email::EmailConfig::load(base_dir) {
                let email = create_alert_email(
                    line_id,
                    current_state,
                    speed_mib,
                    minutes_elapsed,
                    Local::now(),
                );
                email::send_alert(&email.subject, &email.body, &cfg);
            }
            fs::remove_file(&files.timestamp_file).ok();
        }
    }
}

fn test_email_config(base_dir: &str) {
    println!("Testing email configuration...");

    // Try to load the email config
    let config_path = format!("{}/.email_config", base_dir);
    println!("Looking for config at: {}", config_path);

    let config = match email::EmailConfig::load(base_dir) {
        Some(cfg) => {
            println!("✓ Email config loaded successfully");
            println!("  SMTP User: {}", cfg.smtp_user);
            println!("  Recipient: {}", cfg.recipient);
            cfg
        }
        None => {
            eprintln!("✗ Failed to load email config from .email_config");
            eprintln!("  Make sure the file exists at: {}", config_path);
            eprintln!("  Expected format:");
            eprintln!("    SMTP_USER=your-email@gmail.com");
            eprintln!("    SMTP_PASS=your-app-password");
            eprintln!("    RECIPIENT_EMAIL=recipient@example.com");
            std::process::exit(1);
        }
    };

    println!("\nSending test email...");
    let subject = "[Beam Audit] Test Email";
    let body = format!(
        "This is a test email from beam_audit.\n\nSent at: {}\n\nIf you received this, email alerts are working correctly!",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    email::send_alert(subject, &body, &config);
    println!("\nTest complete. Check your inbox at: {}", config.recipient);
}

/// Parse a human-readable timestamp and calculate minutes elapsed since then
fn parse_timestamp_and_get_elapsed_minutes(
    timestamp_str: &str,
    current_time: chrono::DateTime<Local>,
) -> Option<i64> {
    let change_time =
        chrono::NaiveDateTime::parse_from_str(timestamp_str.trim(), "%Y-%m-%d %H:%M").ok()?;

    match chrono::TimeZone::from_local_datetime(&Local, &change_time) {
        chrono::LocalResult::Single(change_datetime)
        | chrono::LocalResult::Ambiguous(change_datetime, _) => Some(
            current_time
                .signed_duration_since(change_datetime)
                .num_minutes(),
        ),
        chrono::LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    #[test]
    fn test_parse_timestamp_format() {
        // Test valid timestamp format
        let valid_timestamp = "2026-01-22 04:05";
        let result = chrono::NaiveDateTime::parse_from_str(valid_timestamp, "%Y-%m-%d %H:%M");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_timestamp_invalid_format() {
        // Test invalid formats
        let invalid_formats = vec![
            "2026-01-22",          // Missing time
            "04:05",               // Missing date
            "2026/01/22 04:05",    // Wrong separator
            "22-01-2026 04:05",    // Wrong date order
            "2026-01-22 04:05:30", // Has seconds (we don't use them)
        ];

        for invalid in invalid_formats {
            let result = chrono::NaiveDateTime::parse_from_str(invalid, "%Y-%m-%d %H:%M");
            assert!(result.is_err(), "Should reject format: {}", invalid);
        }
    }

    #[test]
    fn test_timestamp_elapsed_calculation() {
        // Create a timestamp from 30 minutes ago
        let now = Local::now();
        let past_time = now - Duration::minutes(30);
        let timestamp_str = past_time.format("%Y-%m-%d %H:%M").to_string();

        if let Some(elapsed) = parse_timestamp_and_get_elapsed_minutes(&timestamp_str, now) {
            // Should be approximately 30 minutes (allow small variance for test execution time)
            assert!(
                (29..=31).contains(&elapsed),
                "Expected ~30 minutes, got {}",
                elapsed
            );
        } else {
            panic!("Should successfully parse timestamp");
        }
    }

    #[test]
    fn test_timestamp_threshold_check() {
        // Test that 20-minute threshold logic works
        let threshold = 20i64;

        // 25 minutes ago - should trigger alert
        let now = Local::now();
        let past_time = now - Duration::minutes(25);
        let timestamp_str = past_time.format("%Y-%m-%d %H:%M").to_string();
        if let Some(elapsed) = parse_timestamp_and_get_elapsed_minutes(&timestamp_str, now) {
            assert!(
                elapsed >= threshold,
                "25 minutes should exceed 20-minute threshold"
            );
        }

        // 15 minutes ago - should NOT trigger alert
        let recent_time = now - Duration::minutes(15);
        let timestamp_str = recent_time.format("%Y-%m-%d %H:%M").to_string();
        if let Some(elapsed) = parse_timestamp_and_get_elapsed_minutes(&timestamp_str, now) {
            assert!(
                elapsed < threshold,
                "15 minutes should be under 20-minute threshold"
            );
        }
    }

    #[test]
    fn test_timestamp_format_consistency() {
        // Ensure writing and reading timestamps produces consistent format
        let now = Local::now();
        let written_format = now.format("%Y-%m-%d %H:%M").to_string();

        // Should be able to parse what we write
        let parsed = chrono::NaiveDateTime::parse_from_str(&written_format, "%Y-%m-%d %H:%M");
        assert!(parsed.is_ok());

        // Verify format matches expected pattern (YYYY-MM-DD HH:MM)
        assert_eq!(written_format.len(), 16); // "2026-01-22 04:05" is 16 chars
        assert_eq!(written_format.chars().nth(4), Some('-'));
        assert_eq!(written_format.chars().nth(7), Some('-'));
        assert_eq!(written_format.chars().nth(10), Some(' '));
        assert_eq!(written_format.chars().nth(13), Some(':'));
    }

    // Tests for pure alert logic functions

    #[test]
    fn test_get_alert_file_paths() {
        let files = get_alert_file_paths("/data/storage", "B");
        assert_eq!(files.state_file, "/data/storage/.transfer_state_B");
        assert_eq!(
            files.timestamp_file,
            "/data/storage/.transfer_state_changed_B"
        );
    }

    #[test]
    fn test_create_alert_email_stopped() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let email = create_alert_email("B", "IDLE", 0.0, 25, now);

        assert_eq!(email.subject, "[Beam Alert] Transfer STOPPED on Line B");
        assert!(email.body.contains("stopped"));
        assert!(email.body.contains("25 minutes"));
        assert!(email.body.contains("0.0 MiB/s"));
    }

    #[test]
    fn test_create_alert_email_resumed() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let email = create_alert_email("A", "ACTIVE", 125.5, 30, now);

        assert_eq!(email.subject, "[Beam Alert] Transfer RESUMED on Line A");
        assert!(email.body.contains("resumed"));
        assert!(email.body.contains("30 minutes"));
        assert!(email.body.contains("125.5 MiB/s"));
    }

    #[test]
    fn test_determine_alert_action_no_change_no_timestamp() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let action = determine_alert_action("IDLE", "IDLE", None, 20, now);
        assert_eq!(action, AlertAction::NoAction);
    }

    #[test]
    fn test_determine_alert_action_state_change() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let action = determine_alert_action("IDLE", "ACTIVE", None, 20, now);
        assert_eq!(action, AlertAction::CreateTimestamp);
    }

    #[test]
    fn test_determine_alert_action_under_threshold() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let timestamp = "2026-01-22 11:50"; // 10 minutes ago
        let action = determine_alert_action("IDLE", "IDLE", Some(timestamp), 20, now);
        assert_eq!(action, AlertAction::WaitForThreshold);
    }

    #[test]
    fn test_determine_alert_action_over_threshold() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let timestamp = "2026-01-22 11:30"; // 30 minutes ago
        let action = determine_alert_action("IDLE", "IDLE", Some(timestamp), 20, now);
        assert_eq!(
            action,
            AlertAction::SendAlert {
                minutes_elapsed: 30
            }
        );
    }

    #[test]
    fn test_determine_alert_action_exactly_at_threshold() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 20, 0).unwrap();
        let timestamp = "2026-01-22 12:00"; // Exactly 20 minutes ago
        let action = determine_alert_action("IDLE", "IDLE", Some(timestamp), 20, now);
        assert_eq!(
            action,
            AlertAction::SendAlert {
                minutes_elapsed: 20
            }
        );
    }

    #[test]
    fn test_determine_alert_action_invalid_timestamp() {
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let invalid_timestamp = "invalid-format";
        let action = determine_alert_action("IDLE", "IDLE", Some(invalid_timestamp), 20, now);
        assert_eq!(action, AlertAction::NoAction);
    }

    #[test]
    fn test_determine_alert_action_state_change_with_existing_timestamp() {
        // If state changes while a timestamp exists, should create new timestamp
        let now = Local.with_ymd_and_hms(2026, 1, 22, 12, 0, 0).unwrap();
        let old_timestamp = "2026-01-22 11:30"; // 30 minutes ago
        let action = determine_alert_action("ACTIVE", "IDLE", Some(old_timestamp), 20, now);
        assert_eq!(action, AlertAction::CreateTimestamp);
    }
}

// ============================================================================
// Integration tests - testing check_and_send_alerts() with real file I/O
// These tests use the ACTUAL production code path, ensuring no divergence
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;
    use chrono::TimeZone;
    use std::path::Path;

    /// Helper to create a test environment with temp directory
    /// Returns the TempDir (must keep alive) and base_dir path string
    fn setup_test_env() -> (tempfile::TempDir, String) {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_dir = temp_dir.path().to_str().unwrap().to_string();
        (temp_dir, base_dir)
    }

    #[test]
    fn test_no_action_when_state_unchanged() {
        let (_temp, base_dir) = setup_test_env();

        // Setup: Create initial IDLE state
        let state_file = format!("{}/.transfer_state_B", base_dir);
        fs::write(&state_file, "IDLE").unwrap();

        // Action: Call with unchanged state (IDLE -> IDLE)
        check_and_send_alerts(&base_dir, "B", "IDLE", "IDLE", 0.0, 20);

        // Assert: No timestamp file created
        let timestamp_file = format!("{}/.transfer_state_changed_B", base_dir);
        assert!(
            !Path::new(&timestamp_file).exists(),
            "Timestamp file should not exist for NoAction"
        );

        // Assert: No interruption log created
        let log_file = format!("{}/.transfer_interruptions_B", base_dir);
        assert!(
            !Path::new(&log_file).exists(),
            "Log file should not exist for NoAction"
        );

        // Assert: State file updated
        let content = fs::read_to_string(&state_file).unwrap();
        assert_eq!(content, "IDLE");
    }

    #[test]
    fn test_creates_timestamp_on_state_change() {
        let (_temp, base_dir) = setup_test_env();

        // Setup: Start with IDLE state
        let state_file = format!("{}/.transfer_state_B", base_dir);
        fs::write(&state_file, "IDLE").unwrap();

        // Action: State changes to ACTIVE
        check_and_send_alerts(&base_dir, "B", "ACTIVE", "IDLE", 125.5, 20);

        // Assert: Timestamp file was created
        let timestamp_file = format!("{}/.transfer_state_changed_B", base_dir);
        assert!(
            Path::new(&timestamp_file).exists(),
            "Timestamp file must be created on state change"
        );

        // Assert: Timestamp content is valid format
        let content = fs::read_to_string(&timestamp_file).unwrap();
        let parsed = chrono::NaiveDateTime::parse_from_str(content.trim(), "%Y-%m-%d %H:%M");
        assert!(
            parsed.is_ok(),
            "Timestamp should be in correct format: {}",
            content
        );

        // Assert: State file updated
        let state_content = fs::read_to_string(&state_file).unwrap();
        assert_eq!(state_content, "ACTIVE");

        // Assert: Interruption log was created
        let log_file = format!("{}/.transfer_interruptions_B", base_dir);
        assert!(
            Path::new(&log_file).exists(),
            "Interruption log must be created on state change"
        );
    }

    #[test]
    fn test_log_state_change_format() {
        let (_temp, base_dir) = setup_test_env();

        // Setup
        let state_file = format!("{}/.transfer_state_B", base_dir);
        fs::write(&state_file, "ACTIVE").unwrap();

        // Action: State change from ACTIVE to IDLE
        check_and_send_alerts(&base_dir, "B", "IDLE", "ACTIVE", 0.0, 20);

        // Assert: Log file has correct CSV format
        let log_file = format!("{}/.transfer_interruptions_B", base_dir);
        let log_content = fs::read_to_string(&log_file).unwrap();

        // Should have format: timestamp,old_state,new_state,speed
        // Example: 2026-01-22 10:15:30,ACTIVE,IDLE,0.0
        let lines: Vec<&str> = log_content.lines().collect();
        assert_eq!(lines.len(), 1, "Should have exactly one log entry");

        let parts: Vec<&str> = lines[0].split(',').collect();
        assert_eq!(
            parts.len(),
            4,
            "CSV should have 4 fields: timestamp,old,new,speed"
        );
        assert_eq!(parts[1], "ACTIVE", "Old state should be ACTIVE");
        assert_eq!(parts[2], "IDLE", "New state should be IDLE");
        assert_eq!(parts[3], "0.0", "Speed should be 0.0");

        // Verify timestamp format (YYYY-MM-DD HH:MM:SS)
        assert!(
            parts[0].contains('-') && parts[0].contains(':'),
            "Timestamp should contain date and time separators"
        );
    }

    #[test]
    fn test_waits_for_threshold() {
        let (_temp, base_dir) = setup_test_env();

        // Setup: Create timestamp from 10 minutes ago (under 20 min threshold)
        let timestamp_file = format!("{}/.transfer_state_changed_B", base_dir);
        let past_time = Local::now() - chrono::Duration::minutes(10);
        fs::write(
            &timestamp_file,
            past_time.format("%Y-%m-%d %H:%M").to_string(),
        )
        .unwrap();

        let state_file = format!("{}/.transfer_state_B", base_dir);
        fs::write(&state_file, "IDLE").unwrap();

        // Action: Run with same state (IDLE -> IDLE)
        check_and_send_alerts(&base_dir, "B", "IDLE", "IDLE", 0.0, 20);

        // Assert: Timestamp file still exists (not deleted, waiting for threshold)
        assert!(
            Path::new(&timestamp_file).exists(),
            "Timestamp file should remain while waiting"
        );

        // Assert: Original timestamp unchanged
        let content = fs::read_to_string(&timestamp_file).unwrap();
        let expected = past_time.format("%Y-%m-%d %H:%M").to_string();
        assert_eq!(
            content.trim(),
            expected.trim(),
            "Timestamp should not be modified"
        );
    }

    #[test]
    fn test_sends_alert_after_threshold() {
        let (_temp, base_dir) = setup_test_env();

        // Setup: Create timestamp from 25 minutes ago (over 20 min threshold)
        let timestamp_file = format!("{}/.transfer_state_changed_B", base_dir);
        let past_time = Local::now() - chrono::Duration::minutes(25);
        fs::write(
            &timestamp_file,
            past_time.format("%Y-%m-%d %H:%M").to_string(),
        )
        .unwrap();

        let state_file = format!("{}/.transfer_state_B", base_dir);
        fs::write(&state_file, "IDLE").unwrap();

        // Action: Run with same state (IDLE -> IDLE)
        // Note: Email won't actually send without .email_config, but file operations still happen
        check_and_send_alerts(&base_dir, "B", "IDLE", "IDLE", 0.0, 20);

        // Assert: Timestamp file was DELETED after alert
        assert!(
            !Path::new(&timestamp_file).exists(),
            "Timestamp file must be deleted after sending alert"
        );

        // Assert: State file still updated
        let state_content = fs::read_to_string(&state_file).unwrap();
        assert_eq!(state_content, "IDLE");
    }

    #[test]
    fn test_state_change_overrides_pending_alert() {
        let (_temp, base_dir) = setup_test_env();

        // Setup: Create old timestamp from 25 minutes ago (over threshold)
        let timestamp_file = format!("{}/.transfer_state_changed_B", base_dir);
        let old_time = Local::now() - chrono::Duration::minutes(25);
        fs::write(
            &timestamp_file,
            old_time.format("%Y-%m-%d %H:%M").to_string(),
        )
        .unwrap();
        let old_timestamp_content = fs::read_to_string(&timestamp_file).unwrap();

        let state_file = format!("{}/.transfer_state_B", base_dir);
        fs::write(&state_file, "IDLE").unwrap();

        // Action: State changes (IDLE -> ACTIVE) - should override pending alert
        check_and_send_alerts(&base_dir, "B", "ACTIVE", "IDLE", 120.0, 20);

        // Assert: Timestamp file still exists but with NEW timestamp
        assert!(
            Path::new(&timestamp_file).exists(),
            "Timestamp file should be recreated"
        );
        let new_timestamp_content = fs::read_to_string(&timestamp_file).unwrap();
        assert_ne!(
            old_timestamp_content.trim(),
            new_timestamp_content.trim(),
            "Timestamp should be updated to current time"
        );

        // Assert: New timestamp is recent (within last minute)
        let parsed =
            chrono::NaiveDateTime::parse_from_str(new_timestamp_content.trim(), "%Y-%m-%d %H:%M")
                .unwrap();
        let as_datetime = Local.from_local_datetime(&parsed).unwrap();
        let elapsed = Local::now()
            .signed_duration_since(as_datetime)
            .num_minutes();
        assert!(
            elapsed <= 1,
            "New timestamp should be very recent, got {} minutes",
            elapsed
        );
    }

    #[test]
    fn test_handles_corrupted_timestamp_file() {
        let (_temp, base_dir) = setup_test_env();

        // Setup: Create timestamp file with invalid content
        let timestamp_file = format!("{}/.transfer_state_changed_B", base_dir);
        fs::write(&timestamp_file, "CORRUPTED###DATA!!!").unwrap();

        let state_file = format!("{}/.transfer_state_B", base_dir);
        fs::write(&state_file, "IDLE").unwrap();

        // Action: Should handle gracefully (NoAction due to parse failure)
        check_and_send_alerts(&base_dir, "B", "IDLE", "IDLE", 0.0, 20);

        // Assert: Doesn't panic or crash
        // Assert: Timestamp file still exists (not cleaned up on parse error)
        assert!(
            Path::new(&timestamp_file).exists(),
            "Corrupted file should remain"
        );

        // Assert: State file still updated normally
        let state_content = fs::read_to_string(&state_file).unwrap();
        assert_eq!(state_content, "IDLE");
    }

    #[test]
    fn test_multiple_state_changes_log_accumulation() {
        let (_temp, base_dir) = setup_test_env();

        let state_file = format!("{}/.transfer_state_B", base_dir);

        // Change 1: IDLE -> ACTIVE
        fs::write(&state_file, "IDLE").unwrap();
        check_and_send_alerts(&base_dir, "B", "ACTIVE", "IDLE", 100.0, 20);

        // Change 2: ACTIVE -> IDLE
        check_and_send_alerts(&base_dir, "B", "IDLE", "ACTIVE", 0.0, 20);

        // Change 3: IDLE -> ACTIVE again
        check_and_send_alerts(&base_dir, "B", "ACTIVE", "IDLE", 150.0, 20);

        // Assert: Log file has 3 entries
        let log_file = format!("{}/.transfer_interruptions_B", base_dir);
        let log_content = fs::read_to_string(&log_file).unwrap();
        let lines: Vec<&str> = log_content.lines().collect();
        assert_eq!(
            lines.len(),
            3,
            "Should have 3 log entries for 3 state changes"
        );

        // Assert: Each entry is properly formatted
        for line in lines {
            let parts: Vec<&str> = line.split(',').collect();
            assert_eq!(parts.len(), 4, "Each log line should have 4 CSV fields");
        }
    }
}
