use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn list_output_devices() -> Vec<OutputDevice> {
    vec![OutputDevice {
        id: "default-output".into(),
        name: "Default output device".into(),
        is_default: true,
    }]
}
