use crate::estimates::EstimatesReport;
use crate::gap_analysis::GapReport;
use crate::stats::{AnomalyReport, IntegrityStats};
use chrono::Local;

pub struct AuditReport {
    pub line_id: String,
    pub total_size: u64,
    pub total_files: usize,
    pub speed_bps: u64,
    pub since_timestamp: String,
    pub recent_files: Vec<String>,
    pub redundancy_check: Option<String>,
    pub integrity_stats: Option<IntegrityStats>,
    pub gap_report: Option<GapReport>,
    pub estimates_report: Option<EstimatesReport>,
    pub anomaly_report: Option<AnomalyReport>,
}

#[must_use]
pub fn render_dashboard(line_a_report: &AuditReport, line_b_report: &AuditReport) -> String {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("  <meta http-equiv=\"refresh\" content=\"60\">\n");
    html.push_str("  <title>Transfer Status</title>\n");
    html.push_str(&render_styles());
    html.push_str("</head>\n<body>\n");

    html.push_str("  <h1>Transfer Status</h1>\n");
    html.push_str(&format!(
        "  <h3>Last Sync: {}</h3>\n",
        Local::now().format("%Y-%m-%d %H:%M")
    ));

    html.push_str("  <div class=\"container\">\n");

    // Line B
    html.push_str("    <div class=\"column\">\n");
    html.push_str("      <h2>Line B</h2>\n");
    html.push_str(&render_full_report(line_b_report));
    html.push_str("    </div>\n");

    // Line A
    html.push_str("    <div class=\"column\">\n");
    html.push_str("      <h2>Line A</h2>\n");
    html.push_str(&render_full_report(line_a_report));
    html.push_str("    </div>\n");

    html.push_str("  </div>\n");
    html.push_str("</body>\n</html>\n");

    html
}

fn render_styles() -> String {
    r#"  <style>
    body { background-color: #0c0c0c; color: #d1d1d1; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; padding: 20px; margin: 0; }
    h1 { text-align: center; margin-bottom: 5px; color: #4CAF50; font-size: 2.2em; letter-spacing: 1px; }
    h3 { text-align: center; font-size: 0.9em; color: #777; margin-bottom: 40px; font-weight: normal; text-transform: uppercase; letter-spacing: 2px; }
    .container { display: flex; gap: 20px; justify-content: center; align-items: flex-start; flex-wrap: wrap; }
    .column { flex: 1; min-width: 300px; max-width: 1000px; border: 1px solid #333; padding: 25px; background-color: #161616; border-radius: 12px; box-shadow: 0 10px 30px rgba(0,0,0,0.5); overflow-x: auto; }
    h2 { text-align: center; color: #fff; border-bottom: 1px solid #333; padding-bottom: 15px; margin-top: 0; font-size: 1.5em; letter-spacing: 1px; }
    .section { margin: 20px 0; overflow-x: auto; }
    .section-title { color: #4CAF50; font-size: 1.1em; border-bottom: 1px solid #333; padding-bottom: 8px; margin-bottom: 12px; }
    .green { color: #4CAF50; }
    .yellow { color: #FFD700; font-weight: 500; }
    .red { color: #f44336; }
    .bold { font-weight: bold; }
    .data-table { width: 100%; border-collapse: collapse; margin: 10px 0; font-size: 0.9em; min-width: 600px; }
    .data-table th { background-color: #1a1a1a; padding: 10px; text-align: left; border-bottom: 2px solid #333; }
    .data-table td { padding: 8px; border-bottom: 1px solid #222; }
    .data-table tr.summary { border-top: 3px solid #4CAF50; background-color: #1a1a1a; font-weight: bold; }
    .anomaly-table { width: 100%; border-collapse: collapse; }
    .anomaly-table td { padding: 5px 10px; }
    .recent-files { list-style: none; padding-left: 0; font-size: 0.85em; }
    .recent-files li { padding: 2px 0; word-break: break-all; }
    .redundancy { margin: 10px 0; padding: 10px; background-color: #1a1a1a; border-left: 3px solid #FFC107; }
    @media (max-width: 768px) {
      body { padding: 10px; }
      h1 { font-size: 1.5em; }
      .column { padding: 15px; min-width: 100%; }
      .data-table { font-size: 0.75em; min-width: 500px; }
    }
  </style>
"#.to_string()
}

#[must_use]
pub fn render_full_report(report: &AuditReport) -> String {
    let mut html = String::new();

    // Archive Status
    html.push_str(&render_archive_status(report));

    // Active Transfer Detection
    html.push_str(&render_transfer_status(report));

    // File Integrity & Heuristics
    html.push_str(&render_integrity_section(report));

    // Gap Analysis
    html.push_str(&render_gap_section(report));

    // Transfer Estimates
    html.push_str(&render_estimates_section(report));

    // Directory Size Anomalies
    html.push_str(&render_anomalies_section(report));

    html
}

fn render_archive_status(report: &AuditReport) -> String {
    format!(
        r#"<div class="section">
<p><strong>Archive Status:</strong> <span class="green">{}</span> across <span class="green">{}</span> files.</p>
</div>
"#,
        human_bytes::human_bytes(report.total_size as f64),
        report.total_files
    )
}

fn render_transfer_status(report: &AuditReport) -> String {
    let mut html = String::new();
    html.push_str(
        r#"<div class="section"><h3 class="section-title">Active Transfer Detection</h3>"#,
    );

    let speed_mib = report.speed_bps as f64 / 1_024.0 / 1_024.0;

    if report.speed_bps > 0 {
        html.push_str(&format!(
            r#"<p><strong>Status:</strong> <span class="green">ACTIVE TRANSFER DETECTED</span> (since {})</p>
<p><strong>Current Transfer Speed:</strong> <span class="green">{:.1} MiB/s</span></p>"#,
            report.since_timestamp,
            speed_mib
        ));
    } else {
        html.push_str(&format!(
            r#"<p><strong>Status:</strong> <span class="yellow">IDLE</span> (since {})</p>"#,
            report.since_timestamp
        ));

        if let Some(redundancy) = &report.redundancy_check {
            html.push_str(&format!(r#"<div class="redundancy">{redundancy}</div>"#));
        }
    }

    // Recent Files
    if !report.recent_files.is_empty() {
        html.push_str(r#"<p class="green"><strong>Active/Recent File Writes (last 5m):</strong></p><ul class="recent-files">"#);
        for (i, file) in report.recent_files.iter().enumerate() {
            if i < 3 {
                html.push_str(&format!(r"<li>{}</li>", escape_html(file)));
            }
        }
        if report.recent_files.len() > 3 {
            html.push_str(&format!(
                r"<li>... and {} more files.</li>",
                report.recent_files.len() - 3
            ));
        }
        html.push_str("</ul>");
    }

    html.push_str("</div>\n");
    html
}

fn render_integrity_section(report: &AuditReport) -> String {
    let mut html = String::new();
    html.push_str(
        r#"<div class="section"><h3 class="section-title">File Integrity &amp; Heuristics</h3>"#,
    );

    if let Some(stats) = &report.integrity_stats {
        html.push_str(r#"<table class="data-table"><thead><tr>"#);
        html.push_str("<th>Device Type</th><th>Total</th><th>Empty</th><th>Bad</th>");
        html.push_str("<th>Min</th><th>Max</th><th>Median</th><th>StdDev</th>");
        html.push_str("</tr></thead><tbody>");

        for row in &stats.rows {
            let empty_class = if row.empty > 0 {
                " class=\"yellow\""
            } else {
                ""
            };
            let bad_class = if row.bad > 0 { " class=\"red\"" } else { "" };

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

            html.push_str(&format!(
                r"<tr><td>{}</td><td>{}</td><td{}>{}</td><td{}>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&row.name),
                row.total,
                empty_class, row.empty,
                bad_class, row.bad,
                min_s, max_s, median_s, std_s
            ));
        }

        // Summary Row
        let empty_class = if stats.grand_empty > 0 {
            " class=\"yellow bold\""
        } else {
            " class=\"bold\""
        };
        let bad_class = if stats.grand_bad > 0 {
            " class=\"red bold\""
        } else {
            " class=\"bold\""
        };

        html.push_str(&format!(
            r#"<tr class="summary"><td class="bold">TOTALS / SUMMARY</td><td class="bold">{}</td><td{}>{}</td><td{}>{}{}</td><td class="bold">{}</td><td class="bold">{}</td><td class="bold">{}</td><td class="bold">{}</td></tr>"#,
            stats.grand_total,
            empty_class, stats.grand_empty,
            bad_class, 
            if stats.grand_bad > 0 { "⚠️ " } else { "" },
            stats.grand_bad,
            human_bytes::human_bytes(stats.grand_min as f64),
            human_bytes::human_bytes(stats.grand_max as f64),
            human_bytes::human_bytes(stats.grand_median as f64),
            human_bytes::human_bytes(stats.grand_std_dev)
        ));

        html.push_str("</tbody></table>");
    } else {
        html.push_str("<p>No zip files found.</p>");
    }

    html.push_str("</div>\n");
    html
}

fn render_gap_section(report: &AuditReport) -> String {
    let mut html = String::new();
    html.push_str(
        r#"<div class="section"><h3 class="section-title">Gap Analysis (Weekdays Only)</h3>"#,
    );

    if let Some(gap) = &report.gap_report {
        if gap.is_empty {
            html.push_str("<p>No dated folders found for gap analysis.</p>");
        } else {
            for missing in &gap.missing_weekdays {
                html.push_str(&format!(
                    r#"<p class="red"><strong>⚠️ CRITICAL:</strong> Missing Weekday - {missing}</p>"#
                ));
            }

            html.push_str(&format!(
                r"<p>Range Analyzed: {} to {}</p>",
                gap.start_date, gap.end_date
            ));

            if gap.missing_weekdays.is_empty() {
                html.push_str(&format!(
                    r#"<p class="green">No weekday gaps found.</p> <p>({} weekends skipped)</p>"#,
                    gap.skipped_weekends
                ));
            } else {
                html.push_str(&format!(
                    r#"<p class="red">Found {} unexpected weekday gaps.</p>"#,
                    gap.missing_weekdays.len()
                ));
            }
        }
    } else {
        html.push_str("<p>No gap analysis available.</p>");
    }

    html.push_str("</div>\n");
    html
}

fn render_estimates_section(report: &AuditReport) -> String {
    let mut html = String::new();
    html.push_str(r#"<div class="section"><h3 class="section-title">Transfer Estimates</h3>"#);

    if let Some(est) = &report.estimates_report {
        html.push_str(&format!(
            r#"<p><strong>Current Progress:</strong> Copying <span class="green">{}</span></p>"#,
            est.current_copy_date.format("%Y-%m-%d")
        ));
        html.push_str(&format!(
            r"<p><strong>Daily Average:</strong> {} GiB/day</p>",
            est.daily_average_gib
        ));
        html.push_str(&format!(
            r"<p><strong>Days Remaining:</strong> {} weekdays</p>",
            est.weekdays_remaining
        ));
        html.push_str(&format!(
            r"<p><strong>Est. Data Left:</strong> {:.1} TiB (Free: {:.1} TiB)</p>",
            est.estimated_data_left_tib, est.free_space_tib
        ));

        if est.disk_status_ok {
            html.push_str(r#"<p><strong>Disk Status:</strong> <span class="green">OK</span></p>"#);
        } else {
            html.push_str(r#"<p><strong>Disk Status:</strong> <span class="red">CRITICAL - Insufficient Space!</span></p>"#);
        }

        if let (Some(days), Some(hours)) = (est.estimated_days_eta, est.estimated_hours_eta) {
            html.push_str(&format!(
                r#"<p><strong>Estimated Time:</strong> <span class="yellow">{days} days</span> ({hours} hours) at current speed.</p>"#
            ));
        }
    } else {
        html.push_str("<p>Transfer appears complete.</p>");
    }

    html.push_str("</div>\n");
    html
}

fn render_anomalies_section(report: &AuditReport) -> String {
    let mut html = String::new();
    html.push_str(
        r#"<div class="section"><h3 class="section-title">Directory Size Anomalies</h3>"#,
    );

    if let Some(anom) = &report.anomaly_report {
        html.push_str(&format!(
            r"<p><strong>Median Daily Size:</strong> {}</p>",
            human_bytes::human_bytes(anom.median_daily_size as f64)
        ));

        if anom.anomalies.is_empty() {
            html.push_str("<p>No significant size anomalies found.</p>");
        } else {
            html.push_str(r#"<table class="anomaly-table"><tbody>"#);
            for a in &anom.anomalies {
                let color_class = if a.category == "Too Small" {
                    "red"
                } else {
                    "yellow"
                };
                html.push_str(&format!(
                    r#"<tr><td>⚠️ {}</td><td class="{}">{}</td><td>({})</td></tr>"#,
                    escape_html(&a.name),
                    color_class,
                    human_bytes::human_bytes(a.size as f64),
                    a.category
                ));
            }
            html.push_str("</tbody></table>");
        }
    } else {
        html.push_str("<p>No completed directories.</p>");
    }

    html.push_str("</div>\n");
    html
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
