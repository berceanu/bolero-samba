# Beam Audit Tool

Complete Rust replacement for `audit_transfer.sh` and `update_web.sh`.

## Installation

```bash
cd beam_audit
cargo build --release
# Binary is at: target/release/beam_audit
```

## Usage

### 1. Terminal Audit (replaces audit_transfer.sh)

```bash
# Audit Line B (default)
./beam_audit/target/release/beam_audit

# Audit Line A
./beam_audit/target/release/beam_audit A

# Audit Line B explicitly
./beam_audit/target/release/beam_audit B
```

### 2. HTML Output (single line)

```bash
# Generate HTML for Line B
./beam_audit/target/release/beam_audit B --html > line_b.html

# Generate HTML for Line A
./beam_audit/target/release/beam_audit A --html > line_a.html
```

### 3. Full Dashboard (replaces update_web.sh)

```bash
# Generate complete dashboard with both lines
./beam_audit/target/release/beam_audit --dashboard /var/www/html/index.html
```

This automatically:
- Audits both Line A and Line B
- Generates the full HTML dashboard
- Writes directly to the output file
- Handles locking internally (prevents concurrent runs)
- Sends email alerts on state changes

## Automation

### Cron (recommended)

```bash
# Edit crontab
crontab -e

# Add: Update dashboard every 5 minutes
*/5 * * * * cd /mnt/storage/samba_share_cluster && ./beam_audit/target/release/beam_audit --dashboard /var/www/html/index.html >> /var/log/beam_audit.log 2>&1
```

### Systemd Timer

```ini
# /etc/systemd/system/beam-dashboard.service
[Unit]
Description=Beam Transfer Dashboard Generator

[Service]
Type=oneshot
WorkingDirectory=/mnt/storage/samba_share_cluster
ExecStart=/mnt/storage/samba_share_cluster/beam_audit/target/release/beam_audit --dashboard /var/www/html/index.html
User=www-data
```

```ini
# /etc/systemd/system/beam-dashboard.timer
[Unit]
Description=Update Beam Dashboard every 5 minutes

[Timer]
OnBootSec=1min
OnUnitActiveSec=5min

[Install]
WantedBy=timers.target
```

```bash
sudo systemctl enable --now beam-dashboard.timer
```

## Email Alerts

Configure email settings in `.email_config`:

```bash
SMTP_USER="your-email@gmail.com"
SMTP_PASS="your-app-password"
RECIPIENT_EMAIL="alerts@example.com"
```

The tool automatically sends alerts when transfers start/stop.

## Features

All bash script functionality is now in Rust:

✅ Active transfer detection (10s differential analysis)  
✅ File integrity verification (ZIP validation)  
✅ State tracking with timestamps  
✅ Email alerts on state changes  
✅ Gap analysis (missing weekdays)  
✅ Transfer estimates & ETA  
✅ Directory size anomaly detection  
✅ Redundancy checks (system-wide I/O monitoring)  
✅ HTML dashboard generation  
✅ Terminal colored output  

## Performance

- **Faster**: Native compiled binary vs interpreted bash
- **Parallel**: File scanning uses rayon for multi-threading
- **Efficient**: No external process spawning (awk, python, find)
- **Safe**: Rust memory safety prevents crashes

## Migration from Bash

**Old way:**
```bash
./audit_transfer.sh B          # Terminal output
./update_web.sh                # Dashboard
```

**New way:**
```bash
./beam_audit/target/release/beam_audit B                    # Terminal output
./beam_audit/target/release/beam_audit --dashboard index.html   # Dashboard
```

## No Dependencies Required

The bash scripts required:
- `python3` + `zipfile` library
- `aha` (ANSI to HTML converter)
- `dstat` (optional, for redundancy checks)
- Various GNU utilities (`find`, `awk`, `sed`, `du`, `df`)

The Rust binary requires:
- Nothing! It's a single statically-linked executable
