//! AI cleanup key storage and status, one key per provider.
//!
//! Keys are stored in Windows Credential Manager (generic credentials
//! "Chirp / anthropic-api-key" and "Chirp / openai-api-key"), never in
//! settings.json or logs. `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` still win as
//! developer overrides and are never written to the credential store.
use crate::llm::{LlmClient, LlmError, Provider};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

const SERVICE: &str = "Chirp";

/// The providers that hold a key.
const KEYED: [Provider; 2] = [Provider::Anthropic, Provider::Openai];

fn account(provider: Provider) -> Result<&'static str, String> {
    match provider {
        Provider::Anthropic => Ok("anthropic-api-key"),
        Provider::Openai => Ok("openai-api-key"),
        Provider::Off => Err("No key for provider off".into()),
    }
}

fn env_var(provider: Provider) -> Option<&'static str> {
    match provider {
        Provider::Anthropic => Some("ANTHROPIC_API_KEY"),
        Provider::Openai => Some("OPENAI_API_KEY"),
        Provider::Off => None,
    }
}

fn entry(provider: Provider) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account(provider)?).map_err(|e| e.to_string())
}

pub fn load_key(provider: Provider) -> Option<String> {
    entry(provider).ok()?.get_password().ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

fn store_key(provider: Provider, key: &str) -> Result<(), String> {
    entry(provider)?.set_password(key).map_err(|e| e.to_string())
}

fn delete_key(provider: Provider) -> Result<(), String> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn env_key(provider: Provider) -> Option<String> {
    std::env::var(env_var(provider)?).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub state: KeyState,
    /// Masked key for display, e.g. `sk-ant-••••••••••3fA9`.
    pub key_hint: Option<String>,
    pub from_env: bool,
}

impl Default for AiStatus {
    fn default() -> Self {
        Self { state: KeyState::None, key_hint: None, from_env: false }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatuses {
    pub anthropic: AiStatus,
    pub openai: AiStatus,
}

impl AiStatuses {
    fn get_mut(&mut self, provider: Provider) -> Option<&mut AiStatus> {
        match provider {
            Provider::Anthropic => Some(&mut self.anthropic),
            Provider::Openai => Some(&mut self.openai),
            Provider::Off => None,
        }
    }
}

#[derive(Default)]
pub struct AiState(pub Mutex<AiStatuses>);

pub fn hint(provider: Provider, key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let tail: String = chars[chars.len().saturating_sub(4)..].iter().collect();
    let prefix = if provider == Provider::Openai { "sk-" } else { "sk-ant-" };
    format!("{prefix}••••••••••{tail}")
}

fn classify(err: &LlmError) -> KeyState {
    match err {
        LlmError::Api { status: 401 | 403, .. } => KeyState::Invalid,
        _ => KeyState::Offline,
    }
}

/// Update one provider's status; emits `ai-status` with every provider's status
/// when anything visible changed.
fn publish(app: &AppHandle, provider: Provider, f: impl FnOnce(&mut AiStatus)) -> AiStatuses {
    let state = app.state::<AiState>();
    let mut all = state.0.lock().unwrap_or_else(|e| e.into_inner());
    let before = all.clone();
    if let Some(s) = all.get_mut(provider) {
        f(s);
    }
    let after = all.clone();
    drop(all);
    if before != after {
        let _ = app.emit("ai-status", &after);
    }
    after
}

pub fn status(app: &AppHandle) -> AiStatuses {
    app.state::<AiState>().0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Set the active provider from settings, load every key and validate each in
/// the background.
pub fn init(app: &AppHandle) {
    let client = app.state::<LlmClient>().inner().clone();
    let provider = app.state::<crate::state::SharedState>().blocking_lock().settings.provider();
    client.set_provider(provider);
    for provider in KEYED {
        let (key, from_env) = match env_key(provider) {
            Some(k) => (Some(k), true),
            None => (load_key(provider), false),
        };
        client.set_key_for(provider, key.clone());
        let Some(key) = key else { continue };
        publish(app, provider, |s| {
            s.state = KeyState::Checking;
            s.key_hint = Some(hint(provider, &key));
            s.from_env = from_env;
        });
        let app = app.clone();
        let client = client.clone();
        tauri::async_runtime::spawn(async move {
            let result = client.validate_for(provider, &key).await;
            publish(&app, provider, |s| s.state = result.as_ref().map(|_| KeyState::Connected).unwrap_or_else(classify));
        });
    }
}

/// Called after every cleanup attempt so the Home row shows live status for the
/// active provider.
pub fn report(app: &AppHandle, outcome: Result<(), &LlmError>) {
    let next = match outcome {
        Ok(()) => KeyState::Connected,
        Err(LlmError::Timeout | LlmError::Refusal | LlmError::Truncated | LlmError::Runaway | LlmError::Empty) => return,
        Err(e) => classify(e),
    };
    let provider = app.state::<LlmClient>().provider();
    publish(app, provider, |s| if s.state != KeyState::None { s.state = next });
}

#[tauri::command]
pub fn get_ai_status(app: AppHandle) -> AiStatuses {
    status(&app)
}

/// Validate, then store. A key that cannot be checked because the provider is
/// unreachable is stored anyway (state `offline`) so it works once back online.
#[tauri::command]
pub async fn set_api_key(app: AppHandle, provider: Provider, key: String) -> Result<AiStatuses, String> {
    account(provider)?;
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Empty key".into());
    }
    publish(&app, provider, |s| {
        s.state = KeyState::Checking;
        s.key_hint = Some(hint(provider, &key));
        s.from_env = false;
    });
    let client = app.state::<LlmClient>().inner().clone();
    let state = match client.validate_for(provider, &key).await {
        Ok(()) => KeyState::Connected,
        Err(e) => classify(&e),
    };
    if state != KeyState::Invalid {
        store_key(provider, &key)?;
        client.set_key_for(provider, Some(key));
    }
    Ok(publish(&app, provider, |s| s.state = state))
}

#[tauri::command]
pub fn remove_api_key(app: AppHandle, provider: Provider) -> Result<AiStatuses, String> {
    delete_key(provider)?;
    let from_env = env_key(provider);
    app.state::<LlmClient>().set_key_for(provider, from_env.clone());
    Ok(publish(&app, provider, |s| {
        s.state = if from_env.is_some() { KeyState::Connected } else { KeyState::None };
        s.key_hint = from_env.as_deref().map(|k| hint(provider, k));
        s.from_env = from_env.is_some();
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_masks_all_but_the_last_four() {
        assert_eq!(hint(Provider::Anthropic, "sk-ant-api03-abcdef3fA9"), "sk-ant-••••••••••3fA9");
        assert_eq!(hint(Provider::Anthropic, "ab"), "sk-ant-••••••••••ab");
        assert_eq!(hint(Provider::Openai, "sk-proj-abcdefabcd"), "sk-••••••••••abcd");
    }

    #[test]
    fn only_auth_failures_mark_the_key_invalid() {
        assert_eq!(classify(&LlmError::Api { status: 401, body: String::new() }), KeyState::Invalid);
        assert_eq!(classify(&LlmError::Http("dns".into())), KeyState::Offline);
        assert_eq!(LlmError::Http("dns".into()).reason(), "offline");
        assert_eq!(LlmError::Api { status: 401, body: String::new() }.reason(), "key invalid");
    }

    #[test]
    fn accounts_and_status_json_follow_the_spec() {
        assert_eq!(account(Provider::Anthropic).unwrap(), "anthropic-api-key");
        assert_eq!(account(Provider::Openai).unwrap(), "openai-api-key");
        assert!(account(Provider::Off).is_err());
        let all = AiStatuses {
            anthropic: AiStatus::default(),
            openai: AiStatus { state: KeyState::Connected, key_hint: Some(hint(Provider::Openai, "sk-1234")), from_env: true },
        };
        assert_eq!(
            serde_json::to_value(&all).unwrap(),
            serde_json::json!({
                "anthropic": { "state": "none", "keyHint": null, "fromEnv": false },
                "openai": { "state": "connected", "keyHint": "sk-••••••••••1234", "fromEnv": true }
            })
        );
        let p: Provider = serde_json::from_str("\"openai\"").unwrap();
        assert_eq!(p, Provider::Openai);
    }
}
