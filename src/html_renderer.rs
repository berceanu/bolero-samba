use crate::estimates::EstimatesReport;
use crate::gap_analysis::GapReport;
use crate::stats::{AnomalyReport, BadFilesReport, IntegrityStats};
use chrono::Local;

pub struct AuditReport {
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
    pub bad_files_report: Option<BadFilesReport>,
}

#[must_use]
pub fn render_dashboard(line_a_report: &AuditReport, line_b_report: &AuditReport) -> String {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("  <meta http-equiv=\"refresh\" content=\"60\">\n");
    html.push_str("  <link rel=\"icon\" href=\"data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><text y=%22.9em%22 font-size=%2290%22>⇄</text></svg>\">\n");
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

    // Footer with Ferris
    html.push_str("  <div class=\"footer\">\n");
    html.push_str("    <div>Built with Rust</div>\n");
    html.push_str(r#"    <svg width="48" height="32" viewBox="0 0 1200 800" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><defs><linearGradient id="g1" x1="0" y1="0" x2="1" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(1,0,1.38778e-17,-1,0,-0.000650515)"><stop offset="0" style="stop-color:rgb(247,76,0)"/><stop offset="0.33" style="stop-color:rgb(247,76,0)"/><stop offset="1" style="stop-color:rgb(244,150,0)"/></linearGradient><linearGradient id="g2" x1="0" y1="0" x2="1" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(1,0,0,-1,0,1.23438e-06)"><stop offset="0" style="stop-color:rgb(204,58,0)"/><stop offset="0.15" style="stop-color:rgb(204,58,0)"/><stop offset="0.74" style="stop-color:rgb(247,76,0)"/><stop offset="1" style="stop-color:rgb(247,76,0)"/></linearGradient><linearGradient id="g3" x1="0" y1="0" x2="1" y2="0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(1,1.32349e-23,1.32349e-23,-1,0,-9.1568e-07)"><stop offset="0" style="stop-color:rgb(204,58,0)"/><stop offset="0.15" style="stop-color:rgb(204,58,0)"/><stop offset="0.74" style="stop-color:rgb(247,76,0)"/><stop offset="1" style="stop-color:rgb(247,76,0)"/></linearGradient></defs><g><g transform="matrix(1,0,0,1,597.344,637.02)"><path d="M0,-279.559C-121.238,-279.559 -231.39,-264.983 -312.939,-241.23L-312.939,-38.329C-231.39,-14.575 -121.238,0 0,0C138.76,0 262.987,-19.092 346.431,-49.186L346.431,-230.37C262.987,-260.465 138.76,-279.559 0,-279.559" style="fill:rgb(165,43,0)"/></g><g transform="matrix(1,0,0,1,1068.75,575.642)"><path d="M0,-53.32L-14.211,-82.761C-14.138,-83.879 -14.08,-84.998 -14.08,-86.121C-14.08,-119.496 -48.786,-150.256 -107.177,-174.883L-107.177,2.643C-79.932,-8.849 -57.829,-21.674 -42.021,-35.482C-46.673,-16.775 -62.585,21.071 -75.271,47.686C-96.121,85.752 -103.671,118.889 -102.703,120.53C-102.086,121.563 -94.973,110.59 -84.484,92.809C-60.074,58.028 -13.82,-8.373 -4.575,-25.287C5.897,-44.461 0,-53.32 0,-53.32" style="fill:rgb(165,43,0)"/></g><g transform="matrix(1,0,0,1,149.064,591.421)"><path d="M0,-99.954C0,-93.526 1.293,-87.194 3.788,-80.985L-4.723,-65.835C-4.723,-65.835 -11.541,-56.989 0.465,-38.327C11.055,-21.872 64.1,42.54 92.097,76.271C104.123,93.564 112.276,104.216 112.99,103.187C114.114,101.554 105.514,69.087 81.631,32.046C70.487,12.151 57.177,-14.206 49.189,-33.675C71.492,-19.559 100.672,-6.755 135.341,4.265L135.341,-204.17C51.797,-177.622 0,-140.737 0,-99.954" style="fill:rgb(165,43,0)"/></g><g transform="matrix(-65.8097,-752.207,-752.207,65.8097,621.707,796.312)"><path d="M0.991,-0.034L0.933,0.008C0.933,0.014 0.933,0.02 0.933,0.026L0.99,0.069C0.996,0.073 0.999,0.08 0.998,0.087C0.997,0.094 0.992,0.1 0.986,0.103L0.92,0.133C0.919,0.139 0.918,0.145 0.916,0.15L0.964,0.203C0.968,0.208 0.97,0.216 0.968,0.222C0.965,0.229 0.96,0.234 0.953,0.236L0.882,0.254C0.88,0.259 0.877,0.264 0.875,0.27L0.91,0.33C0.914,0.336 0.914,0.344 0.91,0.35C0.907,0.356 0.9,0.36 0.893,0.361L0.82,0.365C0.817,0.369 0.813,0.374 0.81,0.379L0.832,0.445C0.835,0.452 0.833,0.459 0.828,0.465C0.824,0.47 0.816,0.473 0.809,0.472L0.737,0.462C0.733,0.466 0.729,0.47 0.724,0.474L0.733,0.544C0.734,0.551 0.731,0.558 0.725,0.562C0.719,0.566 0.711,0.568 0.704,0.565L0.636,0.542C0.631,0.546 0.626,0.549 0.621,0.552L0.615,0.621C0.615,0.629 0.61,0.635 0.604,0.638C0.597,0.641 0.589,0.641 0.583,0.638L0.521,0.602C0.52,0.603 0.519,0.603 0.518,0.603L0.406,0.729C0.406,0.729 0.394,0.747 0.359,0.725C0.329,0.705 0.206,0.599 0.141,0.543C0.109,0.52 0.089,0.504 0.09,0.502C0.093,0.499 0.149,0.509 0.217,0.554C0.278,0.588 0.371,0.631 0.38,0.619C0.38,0.619 0.396,0.604 0.406,0.575C0.406,0.575 0.406,0.575 0.406,0.575C0.407,0.576 0.407,0.576 0.406,0.575C0.406,0.575 0.091,0.024 0.305,-0.531C0.311,-0.593 0.275,-0.627 0.275,-0.627C0.266,-0.639 0.178,-0.598 0.12,-0.566C0.055,-0.523 0.002,-0.513 0,-0.516C-0.001,-0.518 0.018,-0.533 0.049,-0.555C0.11,-0.608 0.227,-0.707 0.256,-0.726C0.289,-0.748 0.301,-0.73 0.301,-0.73L0.402,-0.615C0.406,-0.614 0.41,-0.613 0.415,-0.613L0.47,-0.658C0.475,-0.663 0.483,-0.664 0.49,-0.662C0.497,-0.66 0.502,-0.655 0.504,-0.648L0.522,-0.58C0.527,-0.578 0.533,-0.576 0.538,-0.574L0.602,-0.608C0.608,-0.612 0.616,-0.612 0.623,-0.608C0.629,-0.605 0.633,-0.599 0.633,-0.592L0.637,-0.522C0.642,-0.519 0.647,-0.515 0.652,-0.512L0.721,-0.534C0.728,-0.536 0.736,-0.535 0.741,-0.531C0.747,-0.526 0.75,-0.519 0.749,-0.512L0.738,-0.443C0.742,-0.439 0.746,-0.435 0.751,-0.431L0.823,-0.439C0.83,-0.44 0.837,-0.437 0.842,-0.432C0.847,-0.426 0.848,-0.419 0.845,-0.412L0.821,-0.347C0.824,-0.342 0.828,-0.337 0.831,-0.332L0.903,-0.327C0.911,-0.327 0.917,-0.322 0.92,-0.316C0.924,-0.31 0.924,-0.302 0.92,-0.296L0.883,-0.236C0.885,-0.231 0.887,-0.226 0.889,-0.22L0.959,-0.202C0.966,-0.2 0.972,-0.195 0.974,-0.188C0.976,-0.181 0.974,-0.174 0.969,-0.168L0.92,-0.116C0.921,-0.111 0.923,-0.105 0.924,-0.099L0.988,-0.068C0.995,-0.065 0.999,-0.059 1,-0.052C1.001,-0.045 0.997,-0.038 0.991,-0.034ZM0.406,0.575C0.406,0.575 0.406,0.575 0.406,0.575C0.406,0.575 0.406,0.575 0.406,0.575Z" style="fill:url(#g1)"/></g><g transform="matrix(1,0,0,1,450.328,483.629)"><path d="M0,167.33C-1.664,165.91 -2.536,165.068 -2.536,165.068L140.006,153.391C23.733,0 -69.418,122.193 -79.333,135.855L-79.333,167.33L0,167.33Z"/></g><g transform="matrix(1,0,0,1,747.12,477.333)"><path d="M0,171.974C1.663,170.554 2.536,169.71 2.536,169.71L-134.448,159.687C-18.12,0 69.421,126.835 79.335,140.497L79.335,171.974L0,171.974Z"/></g><g transform="matrix(-1.53e-05,-267.211,-267.211,1.53e-05,809.465,764.23)"><path d="M1,-0.586C1,-0.586 0.768,-0.528 0.524,-0.165L0.5,-0.064C0.5,-0.064 1.1,0.265 0.424,0.731C0.424,0.731 0.508,0.586 0.405,0.197C0.405,0.197 0.131,0.376 0.14,0.736C0.14,0.736 -0.275,0.391 0.324,-0.135C0.324,-0.135 0.539,-0.691 1,-0.736L1,-0.586Z" style="fill:url(#g2)"/></g><g transform="matrix(1,0,0,1,677.392,509.61)"><path d="M0,-92.063C0,-92.063 43.486,-139.678 86.974,-92.063C86.974,-92.063 121.144,-28.571 86.974,3.171C86.974,3.171 31.062,47.615 0,3.171C0,3.171 -37.275,-31.75 0,-92.063"/></g><g transform="matrix(1,0,0,1,727.738,435.209)"><path d="M0,0.002C0,18.543 -10.93,33.574 -24.408,33.574C-37.885,33.574 -48.814,18.543 -48.814,0.002C-48.814,-18.539 -37.885,-33.572 -24.408,-33.572C-10.93,-33.572 0,-18.539 0,0.002" style="fill:white"/></g><g transform="matrix(1,0,0,1,483.3,502.984)"><path d="M0,-98.439C0,-98.439 74.596,-131.467 94.956,-57.748C94.956,-57.748 116.283,28.178 33.697,33.028C33.697,33.028 -71.613,12.745 0,-98.439"/></g><g transform="matrix(1,0,0,1,520.766,436.428)"><path d="M0,0C0,19.119 -11.27,34.627 -25.173,34.627C-39.071,34.627 -50.344,19.119 -50.344,0C-50.344,-19.124 -39.071,-34.627 -25.173,-34.627C-11.27,-34.627 0,-19.124 0,0" style="fill:white"/></g><g transform="matrix(-1.53e-05,-239.021,-239.021,1.53e-05,402.161,775.388)"><path d="M0.367,0.129C-0.364,-0.441 0.223,-0.711 0.223,-0.711C0.259,-0.391 0.472,-0.164 0.472,-0.164C0.521,-0.548 0.525,-0.77 0.525,-0.77C1.203,-0.256 0.589,0.161 0.589,0.161C0.627,0.265 0.772,0.372 0.906,0.451L1,0.77C0.376,0.403 0.367,0.129 0.367,0.129Z" style="fill:url(#g3)"/></g></g></svg>"#);
    html.push_str("\n  </div>\n");

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
    progress { appearance: none; border: none; border-radius: 4px; }
    progress::-webkit-progress-bar { background-color: #2a2a2a; border-radius: 4px; }
    progress::-webkit-progress-value { background: linear-gradient(90deg, #4CAF50, #8BC34A); border-radius: 4px; }
    progress::-moz-progress-bar { background: linear-gradient(90deg, #4CAF50, #8BC34A); border-radius: 4px; }
    .footer { text-align: center; margin-top: 40px; padding: 30px 20px; color: #666; font-size: 0.9em; }
    .footer svg { display: block; margin: 15px auto 0; opacity: 0.8; transition: opacity 0.3s; }
    .footer svg:hover { opacity: 1; }
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

    // Transfer Estimates
    html.push_str(&render_estimates_section(report));

    // File Integrity & Heuristics
    html.push_str(&render_integrity_section(report));

    // Gap Analysis
    html.push_str(&render_gap_section(report));

    // Directory Size Anomalies
    html.push_str(&render_anomalies_section(report));

    // Bad ZIP Files
    html.push_str(&render_bad_files_section(report));

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
        html.push_str("<th>Filename</th><th>Total</th><th>Empty</th><th>Bad</th>");
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
    html.push_str(r#"<div class="section"><h3 class="section-title">Missing Daily Archives</h3>"#);

    if let Some(gap) = &report.gap_report {
        if gap.is_empty {
            html.push_str("<p>No dated folders found for gap analysis.</p>");
        } else {
            for missing in &gap.missing_weekdays {
                let day_name = missing.format("%A").to_string();
                html.push_str(&format!(
                    r#"<p class="red"><strong>⚠️</strong> {missing} ({day_name}) - Archive not found</p>"#
                ));
            }

            html.push_str(&format!(
                r"<p>Range checked: {} to {}</p>",
                gap.start_date, gap.end_date
            ));

            if gap.missing_weekdays.is_empty() {
                html.push_str(&format!(
                    r#"<p class="green">No weekday gaps found.</p> <p>({} weekends skipped)</p>"#,
                    gap.skipped_weekends
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
        // Calculate progress percentage based on completed weekdays
        let progress_pct = if est.total_weekdays > 0 {
            ((est.weekdays_completed as f64 / est.total_weekdays as f64 * 100.0).min(100.0)) as u8
        } else {
            0
        };

        // Render HTML5 progress bar
        html.push_str(&format!(
            r#"<div style="margin: 15px 0;"><progress value="{}" max="100" style="width: 100%; height: 22px;"></progress>
<p style="margin: 5px 0 15px 0; font-size: 0.9em; color: #d1d1d1;">{}% complete • {} daily archives remaining</p></div>"#,
            progress_pct,
            progress_pct,
            est.weekdays_remaining
        ));

        html.push_str(&format!(
            r#"<p><strong>Current Progress:</strong> Copying <span class="green">{}</span></p>"#,
            est.current_copy_date.format("%Y-%m-%d")
        ));

        // Format full project date range
        let date_range = format!(
            "{} - {}",
            est.start_date.format("%b %Y"),
            est.end_date.format("%b %Y")
        );

        html.push_str(&format!(
            r"<p><strong>Data to Copy:</strong> {} daily archives ({})</p>",
            est.weekdays_remaining, date_range
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
                r#"<p><strong>Time to Complete:</strong> <span class="yellow">~{days} days</span> ({hours} hours) at current speed</p>"#
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
            r"<p><strong>Median Size:</strong> {}</p>",
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

fn render_bad_files_section(report: &AuditReport) -> String {
    let mut html = String::new();

    if let Some(bad_report) = &report.bad_files_report {
        html.push_str(r#"<div class="section"><h3 class="section-title">Bad ZIP Files</h3>"#);

        let archive_count = bad_report.files_by_folder.len();
        html.push_str(&format!(
            r#"<p>Found {} bad ZIP files across {} archives:</p>"#,
            bad_report.total_count, archive_count
        ));

        // Filter to show only archives with more than 3 bad files
        for (folder, files, total_in_dir) in bad_report.files_by_folder.iter().filter(|(_, _, count)| *count > 3) {
            // Display count based on whether truncation occurred
            let folder_header = if *total_in_dir > files.len() {
                format!(
                    "{} ({} bad files, showing first {})",
                    folder,
                    total_in_dir,
                    files.len()
                )
            } else {
                format!("{} ({} bad files)", folder, total_in_dir)
            };

            html.push_str(&format!(
                r#"<h4 style="color: #ffd700; margin: 15px 0 10px 0;">{}</h4>"#,
                escape_html(&folder_header)
            ));

            for file in files {
                html.push_str(&format!(
                    r#"<div style="margin: 10px 0 10px 20px;">
<p style="margin: 3px 0;"><strong>⚠️</strong> {}</p>
<p style="margin: 3px 0 3px 30px; font-size: 0.9em;">Size: {}</p>
<p style="margin: 3px 0 3px 30px; font-size: 0.9em; color: #ff6b6b;">Reason: {}</p>
</div>"#,
                    escape_html(&file.relative_path),
                    human_bytes::human_bytes(file.size as f64),
                    escape_html(&file.reason)
                ));
            }

            // Show truncation message if needed
            if *total_in_dir > files.len() {
                let remaining = total_in_dir - files.len();
                html.push_str(&format!(
                    r#"<p style="margin: 10px 0 10px 20px; font-style: italic; color: #888;">... {} more bad files in this archive</p>"#,
                    remaining
                ));
            }
        }

        html.push_str("</div>\n");
    }

    html
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
