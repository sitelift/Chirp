//! AI cleanup key storage and status.
//!
//! The Anthropic key is stored in Windows Credential Manager (generic
//! credential "Chirp / anthropic-api-key"), never in settings.json. The
//! `ANTHROPIC_API_KEY` environment variable still wins as a developer override
//! and is never written to the credential store.
use crate::llm::{LlmClient, LlmError};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

const SERVICE: &str = "Chirp";
const ACCOUNT: &str = "anthropic-api-key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
}

pub fn load_key() -> Option<String> {
    entry().ok()?.get_password().ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

fn store_key(key: &str) -> Result<(), String> {
    entry()?.set_password(key).map_err(|e| e.to_string())
}

fn delete_key() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn env_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY").ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    None,
    Checking,
    Connected,
    Invalid,
    Offline,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub state: KeyState,
    /// Masked key for display, e.g. `sk-ant-••••••••••3fA9`.
    pub key_hint: Option<String>,
    pub from_env: bool,
}

pub struct AiState(pub Mutex<AiStatus>);

impl Default for AiState {
    fn default() -> Self {
        Self(Mutex::new(AiStatus { state: KeyState::None, key_hint: None, from_env: false }))
    }
}

pub fn hint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let tail: String = chars[chars.len().saturating_sub(4)..].iter().collect();
    format!("sk-ant-••••••••••{tail}")
}

fn classify(err: &LlmError) -> KeyState {
    match err {
        LlmError::Api { status: 401 | 403, .. } => KeyState::Invalid,
        _ => KeyState::Offline,
    }
}

fn publish(app: &AppHandle, f: impl FnOnce(&mut AiStatus)) -> AiStatus {
    let state = app.state::<AiState>();
    let mut s = state.0.lock().unwrap_or_else(|e| e.into_inner());
    let before = s.clone();
    f(&mut s);
    let after = s.clone();
    drop(s);
    if before.state != after.state || before.key_hint != after.key_hint {
        let _ = app.emit("ai-status", &after);
    }
    after
}

pub fn status(app: &AppHandle) -> AiStatus {
    app.state::<AiState>().0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Load the key at startup and validate it in the background.
pub fn init(app: &AppHandle) {
    let (key, from_env) = match env_key() {
        Some(k) => (Some(k), true),
        None => (load_key(), false),
    };
    let client = app.state::<LlmClient>().inner().clone();
    client.set_key(key.clone());
    let Some(key) = key else { return };
    publish(app, |s| {
        s.state = KeyState::Checking;
        s.key_hint = Some(hint(&key));
        s.from_env = from_env;
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = client.validate(&key).await;
        publish(&app, |s| s.state = result.as_ref().map(|_| KeyState::Connected).unwrap_or_else(classify));
    });
}

/// Called after every cleanup attempt so the Home row shows live status.
pub fn report(app: &AppHandle, outcome: Result<(), &LlmError>) {
    let next = match outcome {
        Ok(()) => KeyState::Connected,
        Err(LlmError::Timeout | LlmError::Refusal | LlmError::Truncated | LlmError::Runaway | LlmError::Empty) => return,
        Err(e) => classify(e),
    };
    publish(app, |s| if s.state != KeyState::None { s.state = next });
}

#[tauri::command]
pub fn get_ai_status(app: AppHandle) -> AiStatus {
    status(&app)
}

/// Validate, then store. A key that cannot be checked because Anthropic is
/// unreachable is stored anyway (state `offline`) so it works once back online.
#[tauri::command]
pub async fn set_api_key(app: AppHandle, key: String) -> Result<AiStatus, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Empty key".into());
    }
    publish(&app, |s| {
        s.state = KeyState::Checking;
        s.key_hint = Some(hint(&key));
        s.from_env = false;
    });
    let client = app.state::<LlmClient>().inner().clone();
    let state = match client.validate(&key).await {
        Ok(()) => KeyState::Connected,
        Err(e) => classify(&e),
    };
    if state != KeyState::Invalid {
        store_key(&key)?;
        client.set_key(Some(key));
    }
    Ok(publish(&app, |s| s.state = state))
}

#[tauri::command]
pub fn remove_api_key(app: AppHandle) -> Result<AiStatus, String> {
    delete_key()?;
    app.state::<LlmClient>().set_key(env_key());
    let from_env = env_key();
    Ok(publish(&app, |s| {
        s.state = if from_env.is_some() { KeyState::Connected } else { KeyState::None };
        s.key_hint = from_env.as_deref().map(hint);
        s.from_env = from_env.is_some();
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_masks_all_but_the_last_four() {
        assert_eq!(hint("sk-ant-api03-abcdef3fA9"), "sk-ant-••••••••••3fA9");
        assert_eq!(hint("ab"), "sk-ant-••••••••••ab");
    }

    #[test]
    fn only_auth_failures_mark_the_key_invalid() {
        assert_eq!(classify(&LlmError::Api { status: 401, body: String::new() }), KeyState::Invalid);
        assert_eq!(classify(&LlmError::Http("dns".into())), KeyState::Offline);
        assert_eq!(LlmError::Http("dns".into()).reason(), "offline");
        assert_eq!(LlmError::Api { status: 401, body: String::new() }.reason(), "key invalid");
    }
}
