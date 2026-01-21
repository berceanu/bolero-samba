# Beam Audit

File transfer monitoring for network storage with real-time dashboards, integrity checks, and email alerts.

## Features

- Transfer speed detection (matches `du` disk usage)
- ZIP integrity validation
- Gap analysis for missing archives
- Email alerts on state changes
- Auto-refresh dashboard
- Static musl binary

## Quick Start

```bash
# Build static binary
cargo build --release --target x86_64-unknown-linux-musl

# Audit single line
./target/release/beam_audit A

# Generate dashboard
./target/release/beam_audit --dashboard /var/www/html/index.html

# Test email
./target/release/beam_audit --test-email
```

## Deployment

```bash
# Cron (every 5 minutes)
*/5 * * * * /path/to/beam_audit --base-dir /data/storage --dashboard /var/www/html/index.html
```

## Email Setup

Create `.email_config` in base directory:

```bash
SMTP_USER=alerts@example.com
SMTP_PASS=your-app-password
RECIPIENT_EMAIL=team@example.com
```

Alerts sent after state persists for 20 minutes (configurable with `--alert-threshold`).

## Requirements

**Build**: Rust 1.80+, musl-tools

## License

MIT
