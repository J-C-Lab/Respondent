use crate::audio::devices::{list_output_devices, OutputDevice};

#[tauri::command]
pub fn list_audio_output_devices() -> Vec<OutputDevice> {
    list_output_devices()
}

#[tauri::command]
pub fn start_session(title: String, output_device_id: String) -> Result<String, String> {
    if title.trim().is_empty() {
        return Err("Session title cannot be empty".into());
    }
    if output_device_id.trim().is_empty() {
        return Err("Output device id cannot be empty".into());
    }
    Ok(format!("session-{}", chrono::Utc::now().timestamp_millis()))
}

#[tauri::command]
pub fn end_session(session_id: String) -> Result<(), String> {
    if session_id.trim().is_empty() {
        return Err("Session id cannot be empty".into());
    }
    Ok(())
}
