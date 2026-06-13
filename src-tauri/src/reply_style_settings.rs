use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const MAX_USER_PROMPT_CHARS: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyStyleSettings {
    pub user_prompt: String,
}

impl Default for ReplyStyleSettings {
    fn default() -> Self {
        Self {
            user_prompt: String::new(),
        }
    }
}

pub struct ReplyStyleSettingsStore {
    path: PathBuf,
    settings: Mutex<ReplyStyleSettings>,
}

impl ReplyStyleSettingsStore {
    pub fn open(app: &AppHandle) -> Result<Self, String> {
        let path = app
            .path()
            .app_data_dir()
            .map_err(|err| format!("Resolve reply style settings path failed: {err}"))?
            .join("reply-style.json");
        let settings = load_settings(&path);
        Ok(Self {
            path,
            settings: Mutex::new(settings),
        })
    }

    pub fn with_settings(settings: ReplyStyleSettings) -> Self {
        let normalized = normalize_reply_style_settings(settings).unwrap_or_default();
        Self {
            path: PathBuf::from("reply-style-test.json"),
            settings: Mutex::new(normalized),
        }
    }

    pub fn get(&self) -> ReplyStyleSettings {
        self.settings
            .lock()
            .expect("reply style settings lock")
            .clone()
    }

    pub fn save(&self, settings: ReplyStyleSettings) -> Result<ReplyStyleSettings, String> {
        let normalized = normalize_reply_style_settings(settings)?;
        save_settings(&self.path, &normalized)?;
        *self.settings.lock().expect("reply style settings lock") = normalized.clone();
        Ok(normalized)
    }
}

pub fn normalize_reply_style_settings(
    settings: ReplyStyleSettings,
) -> Result<ReplyStyleSettings, String> {
    let user_prompt = settings.user_prompt.trim().to_string();
    if user_prompt.chars().count() > MAX_USER_PROMPT_CHARS {
        return Err(format!(
            "回复风格提示词不能超过 {MAX_USER_PROMPT_CHARS} 字符"
        ));
    }
    Ok(ReplyStyleSettings { user_prompt })
}

fn load_settings(path: &PathBuf) -> ReplyStyleSettings {
    let raw = fs::read_to_string(path).ok();
    raw.and_then(|content| serde_json::from_str(&content).ok())
        .and_then(|settings| normalize_reply_style_settings(settings).ok())
        .unwrap_or_default()
}

fn save_settings(path: &PathBuf, settings: &ReplyStyleSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create reply style settings directory failed: {err}"))?;
    }
    let encoded = serde_json::to_string(settings)
        .map_err(|err| format!("Encode reply style settings failed: {err}"))?;
    fs::write(path, encoded).map_err(|err| format!("Write reply style settings failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trims_and_rejects_overlong_prompt() {
        assert_eq!(
            normalize_reply_style_settings(ReplyStyleSettings {
                user_prompt: "  detailed  ".into(),
            })
            .expect("valid"),
            ReplyStyleSettings {
                user_prompt: "detailed".into(),
            }
        );

        let overlong = "x".repeat(MAX_USER_PROMPT_CHARS + 1);
        assert!(normalize_reply_style_settings(ReplyStyleSettings {
            user_prompt: overlong,
        })
        .is_err());
    }
}
