use colored::Colorize;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct IoSample {
    pub disk_write_bytes: u64,
    pub net_recv_bytes: u64,
}

pub fn check_redundancy(line_id: &str) {
    println!(
        "\n{}",
        "=== Redundancy Check (System-Wide Activity) ===".cyan()
    );
    println!("Sampling system I/O (Disk & Network) for 5 seconds...");

    let s1 = get_sample();
    let start = Instant::now();
    thread::sleep(Duration::from_secs(5));
    let s2 = get_sample();
    let elapsed = start.elapsed().as_secs_f64();

    let disk_bps =
        ((s2.disk_write_bytes.saturating_sub(s1.disk_write_bytes)) as f64 / elapsed) as u64;
    let net_bps = ((s2.net_recv_bytes.saturating_sub(s1.net_recv_bytes)) as f64 / elapsed) as u64;

    let threshold = 1_024 * 1_024; // 1 MB/s

    if disk_bps > threshold {
        let mb = disk_bps as f64 / 1_024.0 / 1_024.0;
        let other_line = if line_id == "A" { "B" } else { "A" };

        if is_other_line_active(other_line) {
            println!(
                "{} High Disk Activity ({:.1} MB/s) detected.",
                "INFO:".green(),
                mb
            );
            println!(
                "Activity attributed to concurrent transfer on {}.",
                format!("Line {other_line}").cyan()
            );
        } else {
            println!(
                "{} High System-Wide Disk Activity Detected ({:.1} MB/s).",
                "WARNING:".yellow(),
                mb
            );
            println!(
                "The transfer might be active but buffering, or another process is writing to disk."
            );
        }
    } else if net_bps > threshold {
        let mb = net_bps as f64 / 1_024.0 / 1_024.0;
        let other_line = if line_id == "A" { "B" } else { "A" };

        if is_other_line_active(other_line) {
            println!(
                "{} High Network Activity ({:.1} MB/s) detected.",
                "INFO:".green(),
                mb
            );
            println!(
                "Activity attributed to concurrent transfer on {}.",
                format!("Line {other_line}").cyan()
            );
        } else {
            println!(
                "{} High Network Activity Detected ({:.1} MB/s).",
                "WARNING:".yellow(),
                mb
            );
            println!("Data is being received, but not yet written to the target folder.");
        }
    } else {
        println!(
            "{}: No significant system-wide Disk or Network activity detected.",
            "CONFIRMED IDLE".green()
        );
    }
}

fn get_sample() -> IoSample {
    let mut sample = IoSample::default();

    // 1. Disk Write Bytes (Sum of all sectors written * 512 bytes)
    // /proc/diskstats column 10 is sectors written
    if let Ok(content) = fs::read_to_string("/proc/diskstats") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10
                && let Ok(sectors) = parts[9].parse::<u64>()
            {
                sample.disk_write_bytes += sectors * 512;
            }
        }
    }

    // 2. Network Recv Bytes
    // /proc/net/dev column 2
    if let Ok(content) = fs::read_to_string("/proc/net/dev") {
        for line in content.lines().skip(2) {
            // Skip headers
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2
                && let Ok(bytes) = parts[1].parse::<u64>()
            {
                sample.net_recv_bytes += bytes;
            }
        }
    }

    sample
}

fn is_other_line_active(other_line: &str) -> bool {
    let path = format!("./Line {other_line}");
    // Check if any file modified in the last 1 minute
    crate::scanner::has_recent_activity(&path, 1)
}
