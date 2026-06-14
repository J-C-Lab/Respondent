#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicIsize};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

#[cfg(all(desktop, not(windows)))]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};
#[cfg(windows)]
use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::VK_RETURN,
        WindowsAndMessaging::{
            CallNextHookEx, SetWindowsHookExW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_EXTENDED,
            LLKHF_UP, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
        },
    },
};

const SHOW_DEBOUNCE_MS: u64 = 300;
const HIDE_DEBOUNCE_MS: u64 = 300;

static LAST_HIDE_MS: AtomicU64 = AtomicU64::new(0);
static LAST_SHOW_MS: AtomicU64 = AtomicU64::new(0);
#[cfg(windows)]
static WAKE_SHORTCUT_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static WAKE_HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
#[cfg(windows)]
static WAKE_HOOK_APP: OnceLock<AppHandle> = OnceLock::new();

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn mark_windows_hidden() {
    LAST_HIDE_MS.store(now_ms(), Ordering::SeqCst);
}

fn mark_windows_shown() {
    LAST_SHOW_MS.store(now_ms(), Ordering::SeqCst);
}

fn should_ignore_show_request() -> bool {
    now_ms().saturating_sub(LAST_HIDE_MS.load(Ordering::SeqCst)) < SHOW_DEBOUNCE_MS
}

fn should_ignore_hide_request() -> bool {
    now_ms().saturating_sub(LAST_SHOW_MS.load(Ordering::SeqCst)) < HIDE_DEBOUNCE_MS
}

#[cfg(all(desktop, not(windows)))]
fn numpad_enter_shortcut() -> Shortcut {
    Shortcut::new(None, Code::NumpadEnter)
}

#[cfg(windows)]
fn should_handle_windows_numpad_enter(vk_code: u32, flags: u32) -> bool {
    vk_code == VK_RETURN.0 as u32 && flags & LLKHF_EXTENDED.0 != 0 && flags & LLKHF_UP.0 == 0
}

#[cfg(windows)]
unsafe extern "system" fn windows_numpad_enter_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32
        && WAKE_SHORTCUT_ENABLED.load(Ordering::SeqCst)
        && (wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize)
    {
        let event = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        if should_handle_windows_numpad_enter(event.vkCode, event.flags.0) {
            if let Some(app) = WAKE_HOOK_APP.get() {
                handle_global_numpad_enter(app);
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(windows)]
fn install_windows_numpad_enter_hook() -> Result<(), String> {
    if WAKE_HOOK_HANDLE.load(Ordering::SeqCst) != 0 {
        return Ok(());
    }

    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(windows_numpad_enter_hook),
            HINSTANCE::default(),
            0,
        )
    }
    .map_err(|err| err.to_string())?;

    WAKE_HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst);
    Ok(())
}

#[cfg(windows)]
fn enable_wake_shortcut(_app: &AppHandle) -> Result<(), String> {
    WAKE_SHORTCUT_ENABLED.store(true, Ordering::SeqCst);
    Ok(())
}

#[cfg(all(desktop, not(windows)))]
fn enable_wake_shortcut(app: &AppHandle) -> Result<(), String> {
    let shortcut = numpad_enter_shortcut();
    if app.global_shortcut().is_registered(shortcut.clone()) {
        return Ok(());
    }
    app.global_shortcut()
        .register(shortcut)
        .map_err(|err| err.to_string())
}

#[cfg(windows)]
fn disable_wake_shortcut(_app: &AppHandle) -> Result<(), String> {
    WAKE_SHORTCUT_ENABLED.store(false, Ordering::SeqCst);
    Ok(())
}

#[cfg(all(desktop, not(windows)))]
fn disable_wake_shortcut(app: &AppHandle) -> Result<(), String> {
    let shortcut = numpad_enter_shortcut();
    if !app.global_shortcut().is_registered(shortcut.clone()) {
        return Ok(());
    }
    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|err| err.to_string())
}

#[cfg(desktop)]
fn schedule_enable_wake_shortcut(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let app = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            let _ = enable_wake_shortcut(&app);
        });
    });
}

#[cfg(desktop)]
fn schedule_disable_wake_shortcut(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let app = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            let _ = disable_wake_shortcut(&app);
        });
    });
}

fn hide_all_windows(app: &AppHandle) -> Result<(), String> {
    for (_, window) in app.webview_windows() {
        window.hide().map_err(|err| err.to_string())?;
    }
    mark_windows_hidden();
    Ok(())
}

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if should_ignore_show_request() {
        return Ok(());
    }

    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;

    if main.is_visible().map_err(|err| err.to_string())? {
        return Ok(());
    }

    main.show().map_err(|err| err.to_string())?;
    main.set_focus().map_err(|err| err.to_string())?;
    mark_windows_shown();

    #[cfg(desktop)]
    schedule_disable_wake_shortcut(app);

    Ok(())
}

pub fn handle_global_numpad_enter(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = show_main_window(&handle);
    });
}

#[cfg(windows)]
pub fn init_global_shortcut_plugin(app: &tauri::AppHandle) -> Result<(), String> {
    let _ = WAKE_HOOK_APP.set(app.clone());
    install_windows_numpad_enter_hook()
}

#[cfg(all(desktop, not(windows)))]
pub fn init_global_shortcut_plugin(app: &tauri::AppHandle) -> Result<(), String> {
    let shortcut = numpad_enter_shortcut();
    let shortcut_for_handler = shortcut.clone();
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, pressed, event| {
                if pressed != &shortcut_for_handler || event.state() != ShortcutState::Pressed {
                    return;
                }
                handle_global_numpad_enter(app);
            })
            .build(),
    )
    .map_err(|err| err.to_string())
}

#[cfg(not(desktop))]
pub fn init_global_shortcut_plugin(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

/// Toggle main-window visibility. Returns the new visibility (`true` = shown).
/// Shared by the IPC command and the tray icon.
pub fn toggle_visibility(app: &AppHandle) -> Result<bool, String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;
    let visible = main.is_visible().map_err(|err| err.to_string())?;

    if visible {
        if should_ignore_hide_request() {
            return Ok(true);
        }
        hide_all_windows(app)?;
        #[cfg(desktop)]
        schedule_enable_wake_shortcut(app);
        Ok(false)
    } else {
        show_main_window(app)?;
        Ok(true)
    }
}

#[tauri::command]
pub fn toggle_main_window_visibility(app: tauri::AppHandle) -> Result<bool, String> {
    toggle_visibility(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_request_is_ignored_immediately_after_show() {
        mark_windows_shown();
        assert!(should_ignore_hide_request());
    }

    #[cfg(windows)]
    const VK_RETURN_CODE: u32 = 0x0d;
    #[cfg(windows)]
    const LLKHF_EXTENDED_FLAG: u32 = 0x01;
    #[cfg(windows)]
    const LLKHF_UP_FLAG: u32 = 0x80;

    #[cfg(windows)]
    #[test]
    fn windows_wake_shortcut_accepts_only_numpad_enter_key_down() {
        assert!(should_handle_windows_numpad_enter(
            VK_RETURN_CODE,
            LLKHF_EXTENDED_FLAG
        ));
        assert!(!should_handle_windows_numpad_enter(VK_RETURN_CODE, 0));
        assert!(!should_handle_windows_numpad_enter(
            VK_RETURN_CODE,
            LLKHF_EXTENDED_FLAG | LLKHF_UP_FLAG
        ));
    }
}
