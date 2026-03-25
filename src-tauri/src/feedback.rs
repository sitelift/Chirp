use crate::state::SharedState;
use std::sync::atomic::{AtomicU64, Ordering};

/// Discord webhook URL for user feedback.
/// Write-only — can only post messages, not read the channel.
/// Replace with your actual webhook URL before release.
const DISCORD_WEBHOOK_URL: &str = "https://discord.com/api/webhooks/1486152939553685594/mXEvBkSEcphb1t3c_ewYxn2IuEmEYBdBss8uqUq_oFfmkAPomUfMcMgFHABPdGJAxWW_";

/// Unix timestamp (seconds) of the last feedback submission.
/// Zero means no submission has been made yet.
static LAST_FEEDBACK_TS: AtomicU64 = AtomicU64::new(0);

/// Cooldown between feedback submissions: 5 minutes.
const RATE_LIMIT_SECS: u64 = 300;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub struct FeedbackContext {
    pub text: String,
    pub app_version: &'static str,
    pub os_info: String,
    pub dictation_count: usize,
    pub days_since_install: Option<i64>,
}

pub async fn send(ctx: FeedbackContext) -> Result<(), String> {
    if DISCORD_WEBHOOK_URL.is_empty() {
        return Err("Feedback not configured yet.".into());
    }

    let last = LAST_FEEDBACK_TS.load(Ordering::Relaxed);
    let now = now_secs();
    if last > 0 && now.saturating_sub(last) < RATE_LIMIT_SECS {
        let remaining = RATE_LIMIT_SECS - now.saturating_sub(last);
        return Err(format!(
            "Please wait {remaining} more second(s) before sending feedback again."
        ));
    }

    let days_field = match ctx.days_since_install {
        Some(d) => format!("{d} days"),
        None => "unknown".to_string(),
    };

    let payload = serde_json::json!({
        "embeds": [{
            "title": "Chirp Feedback",
            "color": 0xF0B723_u32,
            "description": ctx.text,
            "fields": [
                { "name": "Version",          "value": ctx.app_version,        "inline": true },
                { "name": "OS",               "value": ctx.os_info,            "inline": true },
                { "name": "Dictations",       "value": ctx.dictation_count.to_string(), "inline": true },
                { "name": "Days since install","value": days_field,             "inline": true },
            ]
        }]
    });

    let client = reqwest::Client::new();
    let response = client
        .post(DISCORD_WEBHOOK_URL)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to send feedback: {e}"))?;

    response
        .error_for_status()
        .map_err(|e| format!("Discord returned an error: {e}"))?;

    LAST_FEEDBACK_TS.store(now, Ordering::Relaxed);
    Ok(())
}

/// Collect context from shared state and send feedback to Discord.
pub async fn send_feedback_command(text: String, state: &SharedState) -> Result<(), String> {
    // Validate input
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Feedback text cannot be empty.".to_string());
    }
    if text.len() > 2000 {
        return Err("Feedback text cannot exceed 2000 characters.".to_string());
    }

    // Collect context from state in a SINGLE lock
    let (dictation_count, days_since_install) = {
        let s = state.lock().await;
        let count = s.history.len();
        let oldest = s.history.first().map(|e| e.timestamp.clone());
        drop(s);

        let days = oldest.and_then(|ts| {
            use chrono::{DateTime, Utc};
            let parsed: Result<DateTime<Utc>, _> = ts.parse();
            parsed.ok().map(|dt| {
                let now: DateTime<Utc> = Utc::now();
                (now - dt).num_days()
            })
        });

        (count, days)
    };

    let os_info = format!(
        "{} {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
    );

    let ctx = FeedbackContext {
        text,
        app_version: env!("CARGO_PKG_VERSION"),
        os_info,
        dictation_count,
        days_since_install,
    };

    send(ctx).await
}
