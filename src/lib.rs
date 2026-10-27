use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

mod vfs;
mod shell;
mod commands;
mod net;

use shell::Shell;
use std::sync::Mutex;
use once_cell::sync::Lazy;

static SHELL: Lazy<Mutex<Shell>> = Lazy::new(|| Mutex::new(Shell::new()));

#[wasm_bindgen(start)]
pub fn main() {
    console::log_1(&"mobile-linux-shell WASM core started".into());
}

#[wasm_bindgen]
pub fn init_shell() -> Result<(), JsValue> {
    let mut s = SHELL.lock().map_err(|e| JsValue::from_str(&e.to_string()))?;
    s.init()?;
    Ok(())
}

#[wasm_bindgen]
pub async fn handle_input(line: String) -> Result<String, JsValue> {
    let mut s = SHELL.lock().map_err(|e| JsValue::from_str(&e.to_string()))?;
    s.execute(&line).await
}

#[wasm_bindgen]
pub fn get_cwd() -> String {
    SHELL.lock().map(|s| s.cwd.clone()).unwrap_or_else(|_| "/root".into())
}

#[wasm_bindgen]
pub fn get_prompt() -> String {
    let cwd = get_cwd();
    format!("root@android:{} $ ", cwd)
}

#[wasm_bindgen]
pub async fn upload_file(path: String, data: Vec<u8>) -> Result<(), JsValue> {
    let mut s = SHELL.lock().map_err(|e| JsValue::from_str(&e.to_string()))?;
    s.vfs.write_file(&path, &data).await
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub async fn download_file(path: String) -> Result<Vec<u8>, JsValue> {
    let s = SHELL.lock().map_err(|e| JsValue::from_str(&e.to_string()))?;
    s.vfs.read_file(&path).await
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn list_extra_keys() -> JsValue {
    // Returned to JS so the Termux-style bar can be built
    let keys = vec![
        "ESC", "TAB", "CTRL", "ALT", "↑", "↓", "←", "→",
        "/", "-", "HOME", "END", "PGUP", "PGDN",
        "Ctrl+C", "Ctrl+D", "Ctrl+Z", "Paste"
    ];
    serde_wasm_bindgen::to_value(&keys).unwrap()
}
