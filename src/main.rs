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
        generate_dashboard(output_file, &args.base_dir);
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
    let change_time_file = format!("{}/.transfer_state_changed_{}", args.base_dir, line_id);

    if current_state != prev_state {
        // State just changed - log it for statistics
        log_state_change(
            &args.base_dir,
            &line_id,
            prev_state,
            current_state,
            speed_mib,
        );

        if !args.html {
            println!(
                "\n{}",
                format!("-- State Change Detected ({prev_state} -> {current_state}) --").cyan()
            );
        }

        // Record timestamp of state change
        let now_timestamp = Local::now().timestamp();
        fs::write(&change_time_file, now_timestamp.to_string()).ok();
        fs::write(&state_file, current_state).ok();
    } else {
        // State unchanged - just update state file
        fs::write(&state_file, current_state).ok();
    }

    // Check if we should send alert (state changed AND been stable >= threshold)
    if current_state != prev_state
        && let Ok(content) = fs::read_to_string(&change_time_file)
        && let Ok(change_timestamp) = content.trim().parse::<i64>()
    {
        let now = Local::now().timestamp();
        let minutes_elapsed = (now - change_timestamp) / 60;

        if minutes_elapsed >= args.alert_threshold as i64 {
            // State has persisted long enough - send alert
            if let Some(cfg) = email::EmailConfig::load() {
                let subject = format!(
                    "[Beam Alert] Transfer {} on Line {}",
                    if current_state == "ACTIVE" {
                        "RESUMED"
                    } else {
                        "STOPPED"
                    },
                    line_id
                );
                let body = format!(
                    "The transfer on Line {} has {}.\n\nCurrent Speed: {:.1} MiB/s\nState persisted for: {} minutes\nTime: {}",
                    line_id,
                    if current_state == "ACTIVE" {
                        "resumed"
                    } else {
                        "stopped"
                    },
                    speed_mib,
                    minutes_elapsed,
                    Local::now()
                );
                email::send_alert(&subject, &body, &cfg);
            }

            // Clear timestamp after alerting
            fs::remove_file(&change_time_file).ok();
        }
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

fn generate_dashboard(output_file: &str, base_dir: &str) {
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
            let handle_a = s.spawn(|| collect_audit_data("A", base_dir, true));
            let handle_b = s.spawn(|| collect_audit_data("B", base_dir, true));

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

fn collect_audit_data(line_id: &str, base_dir: &str, silent: bool) -> html_renderer::AuditReport {
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

    // State change handling (email alerts)
    if current_state == prev_state {
        fs::write(&state_file, current_state).ok();
    } else {
        fs::write(&state_file, current_state).ok();

        if let Some(cfg) = email::EmailConfig::load() {
            let speed_mib = speed_bps as f64 / 1_024.0 / 1_024.0;
            let subject = format!(
                "[Beam Alert] Transfer {} on Line {}",
                if current_state == "ACTIVE" {
                    "RESUMED"
                } else {
                    "STOPPED"
                },
                line_id
            );
            let body = format!(
                "The transfer on Line {} has {}.\n\nCurrent Speed: {:.1} MiB/s\nTime: {}",
                line_id,
                if current_state == "ACTIVE" {
                    "resumed"
                } else {
                    "stopped"
                },
                speed_mib,
                Local::now()
            );
            email::send_alert(&subject, &body, &cfg);
        }
    }

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

fn test_email_config(base_dir: &str) {
    println!("Testing email configuration...");

    // Try to load the email config
    let config_path = format!("{}/.email_config", base_dir);
    println!("Looking for config at: {}", config_path);

    // Change to base_dir so relative path works
    if let Err(e) = std::env::set_current_dir(base_dir) {
        eprintln!("Error: Could not change to directory {}: {}", base_dir, e);
        std::process::exit(1);
    }

    let config = match email::EmailConfig::load() {
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
