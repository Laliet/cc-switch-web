use crate::capabilities::RuntimeCapabilities;

#[tauri::command]
pub fn get_capabilities() -> RuntimeCapabilities {
    RuntimeCapabilities::desktop()
}
