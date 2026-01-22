use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use log::{debug, error, info};
use std::collections::HashMap;
use std::fs;

pub struct EmailConfig {
    pub smtp_user: String,
    pub smtp_pass: String,
    pub recipient: String,
}

impl EmailConfig {
    #[must_use]
    pub fn load(base_dir: &str) -> Option<Self> {
        let config_path = format!("{}/.email_config", base_dir);
        debug!("Loading email config from: {}", config_path);
        let content = fs::read_to_string(&config_path).ok()?;
        let mut map = HashMap::new();

        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"');
                map.insert(key, val);
            }
        }

        Some(EmailConfig {
            smtp_user: (*map.get("SMTP_USER")?).to_string(),
            smtp_pass: (*map.get("SMTP_PASS")?).to_string(),
            recipient: (*map.get("RECIPIENT_EMAIL")?).to_string(),
        })
        .inspect(|cfg| {
            debug!(
                "Email config loaded successfully (user: {}, recipient: {})",
                cfg.smtp_user, cfg.recipient
            );
        })
    }
}

pub fn send_alert(subject: &str, body: &str, config: &EmailConfig) {
    // Build email with proper error handling
    let from_addr = match config.smtp_user.parse() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Invalid sender email address: {}", e);
            return;
        }
    };

    let to_addr = match config.recipient.parse() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Invalid recipient email address: {}", e);
            return;
        }
    };

    let email = match Message::builder()
        .from(from_addr)
        .to(to_addr)
        .subject(subject)
        .body(body.to_string())
    {
        Ok(msg) => msg,
        Err(e) => {
            error!("Failed to build email message: {}", e);
            return;
        }
    };

    let creds = Credentials::new(config.smtp_user.clone(), config.smtp_pass.clone());

    // Open connection to Gmail (smtps port 465)
    let mailer = match SmtpTransport::relay("smtp.gmail.com") {
        Ok(transport) => transport.credentials(creds).build(),
        Err(e) => {
            error!("Failed to connect to SMTP server: {}", e);
            return;
        }
    };

    debug!("Sending email: {}", subject);
    match mailer.send(&email) {
        Ok(_) => info!("Email sent successfully: {}", subject),
        Err(e) => error!("Failed to send email: {}", e),
    }
}
