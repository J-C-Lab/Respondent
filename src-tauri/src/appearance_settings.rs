use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

pub const APPEARANCE_SETTINGS_EVENT: &str = "appearance-settings-changed";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub window_opacity: u8,
    pub window_blur: u8,
    pub appearance_theme: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            window_opacity: 72,
            window_blur: 24,
            appearance_theme: "dark".into(),
        }
    }
}

pub struct AppearanceSettingsStore {
    path: PathBuf,
    settings: Mutex<AppearanceSettings>,
}

impl AppearanceSettingsStore {
    pub fn open(app: &AppHandle) -> Result<Self, String> {
        let path = app
            .path()
            .app_data_dir()
            .map_err(|err| format!("Resolve appearance settings path failed: {err}"))?
            .join("appearance.json");
        let settings = load_settings(&path);
        Ok(Self {
            path,
            settings: Mutex::new(settings),
        })
    }

    pub fn get(&self) -> AppearanceSettings {
        self.settings.lock().expect("appearance settings lock").clone()
    }

    pub fn publish(
        &self,
        app: &AppHandle,
        settings: AppearanceSettings,
    ) -> Result<AppearanceSettings, String> {
        let normalized = normalize_appearance_settings(settings);
        save_settings(&self.path, &normalized)?;
        *self.settings.lock().expect("appearance settings lock") = normalized.clone();

        let webviews = app.webview_windows();
        if webviews.is_empty() {
            app.emit(APPEARANCE_SETTINGS_EVENT, &normalized)
                .map_err(|err| format!("Broadcast appearance settings failed: {err}"))?;
        } else {
            for (label, webview) in webviews {
                webview.emit(APPEARANCE_SETTINGS_EVENT, &normalized).map_err(|err| {
                    format!("Broadcast appearance settings to {label} failed: {err}")
                })?;
            }
        }

        Ok(normalized)
    }
}

pub fn normalize_appearance_settings(settings: AppearanceSettings) -> AppearanceSettings {
    AppearanceSettings {
        window_opacity: settings.window_opacity.clamp(55, 92),
        window_blur: settings.window_blur.clamp(8, 32),
        appearance_theme: if settings.appearance_theme == "light" {
            "light".into()
        } else {
            "dark".into()
        },
    }
}

fn load_settings(path: &PathBuf) -> AppearanceSettings {
    let raw = fs::read_to_string(path).ok();
    raw.and_then(|content| serde_json::from_str(&content).ok())
        .map(normalize_appearance_settings)
        .unwrap_or_default()
}

fn save_settings(path: &PathBuf, settings: &AppearanceSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create appearance settings directory failed: {err}"))?;
    }
    let encoded = serde_json::to_string(settings)
        .map_err(|err| format!("Encode appearance settings failed: {err}"))?;
    fs::write(path, encoded).map_err(|err| format!("Write appearance settings failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_clamps_and_validates_theme() {
        assert_eq!(
            normalize_appearance_settings(AppearanceSettings {
                window_opacity: 10,
                window_blur: 99,
                appearance_theme: "neon".into(),
            }),
            AppearanceSettings {
                window_opacity: 55,
                window_blur: 32,
                appearance_theme: "dark".into(),
            }
        );
    }
}
