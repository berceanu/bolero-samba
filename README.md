# Beam Audit

Rust-based file transfer monitoring and audit system for network storage. Generates real-time dashboards with transfer speed detection, integrity validation, gap analysis, and email alerts.

## Features

- **Transfer Detection**: Accurate speed measurement using disk block allocation (matches `du` behavior)
- **Integrity Validation**: Fast ZIP file verification without full extraction
- **Gap Analysis**: Detects missing weekday archives
- **Email Alerts**: Automatic notifications on transfer state changes
- **Responsive Dashboard**: Mobile-friendly HTML with auto-refresh
- **Parallel Processing**: Concurrent audits for multiple production lines
- **Cross-Platform**: Static musl binary works on any Linux (Ubuntu 18.04+)

## Quick Start

### Build

```bash
# Standard build
cargo build --release

# Cross-platform static binary (recommended)
cargo build --release --target x86_64-unknown-linux-musl
```

### Usage

```bash
# Terminal audit (single line)
./target/release/beam_audit A     # Audit Line A
./target/release/beam_audit B     # Audit Line B

# HTML fragment output
./target/release/beam_audit B --html > output.html

# Full dashboard (both lines)
./target/release/beam_audit --dashboard /var/www/html/index.html

# Custom base directory
./target/release/beam_audit --base-dir /custom/path --dashboard dashboard.html

# Test email configuration
./target/release/beam_audit --test-email

# Custom alert threshold (default: 20 minutes)
./target/release/beam_audit --alert-threshold 15  # Alert after 15 min of stable state
```

## Deployment

### Cron Setup

```bash
# Run every 5 minutes (adjust paths as needed)
*/5 * * * * /data/storage/samba_share_cluster/beam_audit/target/x86_64-unknown-linux-musl/release/beam_audit --base-dir /data/storage/samba_share_cluster --dashboard /var/www/html/index.html >> /tmp/beam_audit.log 2>&1
```

**Note**: The `--base-dir` argument allows running from any directory, so no `cd` needed.

### Email Configuration

Create `.email_config` in the base directory (default: `/data/storage/samba_share_cluster/`):

```bash
SMTP_USER=alerts@example.com
SMTP_PASS=your-app-password
RECIPIENT_EMAIL=team@example.com
```

**Gmail Setup**:
1. Enable 2-factor authentication
2. Generate App Password at: https://myaccount.google.com/apppasswords
3. Use the 16-character app password (NOT your regular password)

**Test Configuration**:
```bash
./beam_audit --base-dir /path/to/data --test-email
```

**Email Alert Behavior**:
- Alerts sent only when state persists for `--alert-threshold` minutes (default: 20)
- Prevents spam from brief network hiccups
- All state changes logged to `.transfer_interruptions_{A|B}` for diagnostics

**Interruption Log Format** (CSV):
```
2026-01-21 14:32:15,ACTIVE,IDLE,0.0
2026-01-21 14:35:42,IDLE,ACTIVE,45.3
```
This log helps analyze network stability patterns and calculate uptime statistics.

## Architecture

```
beam_audit/
├── src/
│   ├── main.rs          # CLI & parallel execution
│   ├── scanner.rs       # File scanning & size measurement
│   ├── stats.rs         # Integrity statistics
│   ├── gap_analysis.rs  # Missing archive detection
│   ├── estimates.rs     # Transfer ETA calculations
│   ├── html_renderer.rs # Dashboard generation
│   ├── email.rs         # SMTP alerts
│   ├── system_io.rs     # I/O monitoring
│   └── types.rs         # Data structures
├── Cargo.toml
└── README.md
```

## Performance

- **Native Speed**: Compiled Rust binary (no interpreters)
- **Parallel Scanning**: Multi-threaded file operations
- **Efficient Measurement**: Uses `MetadataExt::blocks()` for accurate disk usage
- **Low Memory**: Streaming file processing

## Requirements

**Runtime**: None (static binary)

**Build**:
- Rust 1.80+ (Edition 2024)
- musl-tools (for static builds): `apt install musl-tools`

## License

MIT
