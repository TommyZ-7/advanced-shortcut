use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use sysinfo::{ProcessesToUpdate, System};

#[cfg(windows)]
use base64::Engine;
#[cfg(windows)]
use image::{Rgba, RgbaImage};

// ========================================
// Data Types
// ========================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowConfig {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    Launch {
        path: String,
        args: Option<Vec<String>>,
        window_config: Option<WindowConfig>,
    },
    Kill {
        #[serde(rename = "processName")]
        process_name: String,
    },
    OpenFolder {
        path: String,
        window_config: Option<WindowConfig>,
    },
    OpenUrl {
        url: String,
        window_config: Option<WindowConfig>,
    },
    Delay {
        ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub group_id: String,
    pub actions: Vec<Action>,
    pub order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub order: i32,
    pub is_expanded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppData {
    pub shortcuts: Vec<Shortcut>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_usage: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub hwnd: i64,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

// ========================================
// CLI Shortcut Execution
// ========================================

#[derive(Debug, Default, Clone, Serialize)]
struct CliShortcutRequest {
    shortcut_id: String,
}

fn parse_cli_shortcut_request() -> Option<CliShortcutRequest> {
    let mut request = CliShortcutRequest::default();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--execute-shortcut" => {
                if let Some(id) = args.next() {
                    request.shortcut_id = id;
                }
            }
            _ => {}
        }
    }

    if request.shortcut_id.is_empty() {
        None
    } else {
        Some(request)
    }
}

/// CLI引数からショートカットを直接実行する（フロントエンドを介さない）
fn execute_shortcut_from_cli(request: &CliShortcutRequest) -> Result<(), String> {
    // データをロードしてショートカットを検索
    let app_data = load_app_data();
    let shortcut = app_data
        .shortcuts
        .iter()
        .find(|s| s.id == request.shortcut_id)
        .ok_or_else(|| format!("Shortcut not found: {}", request.shortcut_id))?
        .clone();

    // 同期的にショートカットのアクションを実行
    for action in shortcut.actions {
        if let Err(e) = execute_action_sync(&action) {
            eprintln!("Action error: {}", e);
        }
    }

    Ok(())
}

/// アクションを同期的に実行する（CLI実行用）
fn execute_action_sync(action: &Action) -> Result<String, String> {
    match action {
        Action::Launch {
            path,
            args,
            window_config,
        } => {
            let exe_name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            #[cfg(windows)]
            {
                let existing_windows = window_control::find_windows_by_process_name(&exe_name);

                if !existing_windows.is_empty() {
                    if let Some(config) = window_config {
                        for hwnd in existing_windows {
                            if let Some((cur_x, cur_y, cur_w, cur_h)) =
                                window_control::get_window_rect(hwnd)
                            {
                                let x = config.x.unwrap_or(cur_x);
                                let y = config.y.unwrap_or(cur_y);
                                let w = config.width.unwrap_or(cur_w);
                                let h = config.height.unwrap_or(cur_h);
                                let _ = window_control::set_window_position(hwnd, x, y, w, h);
                            }
                        }
                    }
                    return Ok(format!("Adjusted window for already running: {}", exe_name));
                }
            }

            let mut cmd = Command::new(&path);
            if let Some(arguments) = args {
                cmd.args(arguments);
            }

            let child = cmd
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {}", path, e))?;

            #[cfg(windows)]
            if let Some(config) = window_config {
                let pid = child.id();
                std::thread::sleep(std::time::Duration::from_millis(1000));

                for _ in 0..10 {
                    if let Some(hwnd) = window_control::find_window_by_pid(pid) {
                        let x = config.x.unwrap_or(0);
                        let y = config.y.unwrap_or(0);
                        let w = config.width.unwrap_or(800);
                        let h = config.height.unwrap_or(600);
                        let _ = window_control::set_window_position(hwnd, x, y, w, h);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }

            Ok(format!("Launched: {}", path))
        }

        Action::Kill { process_name } => {
            let mut sys = System::new_all();
            sys.refresh_processes(ProcessesToUpdate::All, true);

            let mut killed_count = 0;
            for (_pid, process) in sys.processes() {
                if process.name().to_string_lossy().to_lowercase() == process_name.to_lowercase() {
                    process.kill();
                    killed_count += 1;
                }
            }

            if killed_count > 0 {
                Ok(format!(
                    "Killed {} instance(s) of {}",
                    killed_count, process_name
                ))
            } else {
                Err(format!("Process not found: {}", process_name))
            }
        }

        Action::OpenFolder { path, window_config } => {
            #[cfg(windows)]
            let before_windows: std::collections::HashSet<isize> = if window_config.is_some() {
                window_control::get_explorer_windows()
                    .iter()
                    .map(|(hwnd, _)| hwnd.0 as isize)
                    .collect()
            } else {
                std::collections::HashSet::new()
            };

            open::that(&path).map_err(|e| format!("Failed to open folder {}: {}", path, e))?;

            #[cfg(windows)]
            if let Some(config) = window_config {
                std::thread::sleep(std::time::Duration::from_millis(800));

                let after_windows = window_control::get_explorer_windows();
                
                let new_window = after_windows.iter().find(|(hwnd, _)| {
                    !before_windows.contains(&(hwnd.0 as isize))
                });

                let target_hwnd = if let Some((hwnd, _)) = new_window {
                    Some(*hwnd)
                } else {
                    let folder_name = std::path::Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    
                    after_windows.iter()
                        .find(|(_, title)| title.contains(&folder_name))
                        .map(|(hwnd, _)| *hwnd)
                };

                if let Some(hwnd) = target_hwnd {
                    if let Some((cur_x, cur_y, cur_w, cur_h)) = window_control::get_window_rect(hwnd) {
                        let x = config.x.unwrap_or(cur_x);
                        let y = config.y.unwrap_or(cur_y);
                        let w = config.width.unwrap_or(cur_w);
                        let h = config.height.unwrap_or(cur_h);
                        let _ = window_control::set_window_position(hwnd, x, y, w, h);
                    }
                }
            }

            Ok(format!("Opened folder: {}", path))
        }

        Action::OpenUrl { url, window_config } => {
            #[cfg(windows)]
            let before_windows: std::collections::HashSet<isize> = if window_config.is_some() {
                let mut windows = std::collections::HashSet::new();
                for browser in ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"] {
                    for (hwnd, _) in window_control::get_windows_by_process_name(browser) {
                        windows.insert(hwnd.0 as isize);
                    }
                }
                windows
            } else {
                std::collections::HashSet::new()
            };

            #[cfg(windows)]
            {
                use winreg::enums::*;
                use winreg::RegKey;
                
                let mut opened = false;
                
                fn get_browser_path(prog_id: &str) -> Option<String> {
                    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
                    let command_key = format!(r"{}\shell\open\command", prog_id);
                    if let Ok(key) = hkcr.open_subkey(&command_key) {
                        if let Ok(cmd) = key.get_value::<String, _>("") {
                            let cmd = cmd.trim();
                            if cmd.starts_with('"') {
                                if let Some(end) = cmd[1..].find('"') {
                                    return Some(cmd[1..end+1].to_string());
                                }
                            } else if let Some(end) = cmd.find(' ') {
                                return Some(cmd[..end].to_string());
                            } else {
                                return Some(cmd.to_string());
                            }
                        }
                    }
                    None
                }
                
                if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER)
                    .open_subkey(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice")
                {
                    if let Ok(prog_id) = hkcu.get_value::<String, _>("ProgId") {
                        if let Some(browser_path) = get_browser_path(&prog_id) {
                            let prog_id_lower = prog_id.to_lowercase();
                            
                            let new_window_arg = if prog_id_lower.contains("firefox") {
                                "-new-window"
                            } else {
                                "--new-window"
                            };
                            
                            if Command::new(&browser_path)
                                .args([new_window_arg, &url])
                                .spawn()
                                .is_ok()
                            {
                                opened = true;
                            }
                        }
                    }
                }
                
                if !opened {
                    let browser_paths = [
                        (r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe", "--new-window"),
                        (r"C:\Program Files\Microsoft\Edge\Application\msedge.exe", "--new-window"),
                        (r"C:\Program Files\Google\Chrome\Application\chrome.exe", "--new-window"),
                        (r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe", "--new-window"),
                        (r"C:\Program Files\Mozilla Firefox\firefox.exe", "-new-window"),
                        (r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe", "--new-window"),
                    ];
                    
                    for (path, arg) in browser_paths {
                        if std::path::Path::new(path).exists() {
                            if Command::new(path)
                                .args([arg, &url])
                                .spawn()
                                .is_ok()
                            {
                                opened = true;
                                break;
                            }
                        }
                    }
                }
                
                if !opened {
                    let _ = Command::new("cmd")
                        .args(["/c", "start", "", &url])
                        .spawn()
                        .map_err(|e| format!("Failed to open URL {}: {}", url, e))?;
                }
            }

            #[cfg(not(windows))]
            {
                open::that(&url).map_err(|e| format!("Failed to open URL {}: {}", url, e))?;
            }

            #[cfg(windows)]
            if let Some(config) = window_config {
                std::thread::sleep(std::time::Duration::from_millis(1500));

                let mut target_hwnd: Option<windows::Win32::Foundation::HWND> = None;
                
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
                    let fg_hwnd = GetForegroundWindow();
                    if !fg_hwnd.is_invalid() {
                        for browser in ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"] {
                            let browser_windows = window_control::get_windows_by_process_name(browser);
                            for (hwnd, _) in &browser_windows {
                                if hwnd.0 == fg_hwnd.0 {
                                    target_hwnd = Some(*hwnd);
                                    break;
                                }
                            }
                            if target_hwnd.is_some() {
                                break;
                            }
                        }
                    }
                }
                
                if target_hwnd.is_none() {
                    for browser in ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"] {
                        let after_windows = window_control::get_windows_by_process_name(browser);
                        for (hwnd, _) in after_windows {
                            if !before_windows.contains(&(hwnd.0 as isize)) {
                                target_hwnd = Some(hwnd);
                                break;
                            }
                        }
                        if target_hwnd.is_some() {
                            break;
                        }
                    }
                }

                if let Some(hwnd) = target_hwnd {
                    if let Some((cur_x, cur_y, cur_w, cur_h)) = window_control::get_window_rect(hwnd) {
                        let x = config.x.unwrap_or(cur_x);
                        let y = config.y.unwrap_or(cur_y);
                        let w = config.width.unwrap_or(cur_w);
                        let h = config.height.unwrap_or(cur_h);
                        let _ = window_control::set_window_position(hwnd, x, y, w, h);
                    }
                }
            }

            Ok(format!("Opened URL: {}", url))
        }

        Action::Delay { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(*ms));
            Ok(format!("Delayed for {}ms", ms))
        }
    }
}

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle, code: i32) {
    app_handle.exit(code);
}

// ========================================
// Utility Functions
// ========================================

fn get_data_path() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("advanced-shortcut");

    // Create directory if it doesn't exist
    if !data_dir.exists() {
        fs::create_dir_all(&data_dir).ok();
    }

    data_dir.join("data.json")
}

fn load_app_data() -> AppData {
    let path = get_data_path();

    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_else(|_| default_app_data())
    } else {
        default_app_data()
    }
}

fn save_app_data(data: &AppData) -> Result<(), String> {
    let path = get_data_path();
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize data: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write data: {}", e))?;
    Ok(())
}

fn default_app_data() -> AppData {
    AppData {
        shortcuts: vec![],
        groups: vec![Group {
            id: "default".to_string(),
            name: "デフォルト".to_string(),
            color: "#22d3ee".to_string(),
            icon: "folder".to_string(),
            order: 0,
            is_expanded: true,
        }],
    }
}

// ========================================
// Tauri Commands
// ========================================

#[cfg(windows)]
mod window_control {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, MoveWindow,
        SetForegroundWindow,
    };

    pub fn find_window_by_pid(target_pid: u32) -> Option<HWND> {
        static FOUND_HWND: AtomicIsize = AtomicIsize::new(0);
        static TARGET: AtomicIsize = AtomicIsize::new(0);

        FOUND_HWND.store(0, Ordering::SeqCst);
        TARGET.store(target_pid as isize, Ordering::SeqCst);

        unsafe extern "system" fn enum_callback(hwnd: HWND, _: LPARAM) -> BOOL {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            let target = TARGET.load(Ordering::SeqCst) as u32;
            if pid == target {
                // Check if window is visible
                if IsWindowVisible(hwnd).as_bool() {
                    FOUND_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
                    return BOOL(0); // Stop enumeration
                }
            }
            BOOL(1) // Continue
        }

        unsafe {
            let _ = EnumWindows(Some(enum_callback), LPARAM(0));
        }

        let found = FOUND_HWND.load(Ordering::SeqCst);
        if found != 0 {
            Some(HWND(found as *mut std::ffi::c_void))
        } else {
            None
        }
    }

    pub fn find_windows_by_process_name(process_name: &str) -> Vec<HWND> {
        use sysinfo::{ProcessesToUpdate, System};
        use windows::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW;

        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let mut windows = Vec::new();
        for (pid, process) in sys.processes() {
            if process.name().to_string_lossy().to_lowercase() == process_name.to_lowercase() {
                // Get all visible windows for this PID
                let pid_windows = find_all_windows_by_pid(pid.as_u32());
                for hwnd in pid_windows {
                    // Check if window has a title (skip hidden/utility windows)
                    unsafe {
                        let title_len = GetWindowTextLengthW(hwnd);
                        if title_len > 0 {
                            windows.push(hwnd);
                        }
                    }
                }
            }
        }
        windows
    }

    pub fn find_all_windows_by_pid(target_pid: u32) -> Vec<HWND> {
        use std::sync::Mutex;
        
        static RESULTS: std::sync::OnceLock<Mutex<Vec<isize>>> = std::sync::OnceLock::new();
        static TARGET_PID: AtomicIsize = AtomicIsize::new(0);
        
        let results_lock = RESULTS.get_or_init(|| Mutex::new(Vec::new()));
        {
            let mut results = results_lock.lock().unwrap();
            results.clear();
        }
        TARGET_PID.store(target_pid as isize, Ordering::SeqCst);

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            let target = TARGET_PID.load(Ordering::SeqCst) as u32;
            if pid == target && IsWindowVisible(hwnd).as_bool() {
                let results_ptr = lparam.0 as *const Mutex<Vec<isize>>;
                if let Some(results_lock) = unsafe { results_ptr.as_ref() } {
                    if let Ok(mut results) = results_lock.lock() {
                        results.push(hwnd.0 as isize);
                    }
                }
            }
            BOOL(1) // Continue
        }

        unsafe {
            let _ = EnumWindows(Some(enum_callback), LPARAM(results_lock as *const _ as isize));
        }

        let results = results_lock.lock().unwrap();
        results.iter().map(|&h| HWND(h as *mut std::ffi::c_void)).collect()
    }

    // Get all visible explorer windows with their hwnds
    pub fn get_explorer_windows() -> Vec<(HWND, String)> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowTextLengthW, GetWindowTextW,
        };
        
        let windows = find_windows_by_process_name("explorer.exe");
        let mut result = Vec::new();
        
        for hwnd in windows {
            unsafe {
                let title_len = GetWindowTextLengthW(hwnd);
                if title_len > 0 {
                    let mut title_buf = vec![0u16; (title_len + 1) as usize];
                    GetWindowTextW(hwnd, &mut title_buf);
                    let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);
                    result.push((hwnd, title));
                }
            }
        }
        
        result
    }

    // Get all visible windows by process name (for browser windows etc.)
    pub fn get_windows_by_process_name(process_name: &str) -> Vec<(HWND, String)> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowTextLengthW, GetWindowTextW,
        };
        
        let windows = find_windows_by_process_name(process_name);
        let mut result = Vec::new();
        
        for hwnd in windows {
            unsafe {
                let title_len = GetWindowTextLengthW(hwnd);
                if title_len > 0 {
                    let mut title_buf = vec![0u16; (title_len + 1) as usize];
                    GetWindowTextW(hwnd, &mut title_buf);
                    let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);
                    result.push((hwnd, title));
                }
            }
        }
        
        result
    }

    pub fn set_window_position(
        hwnd: HWND,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        unsafe {
            MoveWindow(hwnd, x, y, width, height, true)
                .map_err(|e| format!("Failed to move window: {}", e))?;
            let _ = SetForegroundWindow(hwnd);
        }
        Ok(())
    }

    pub fn get_window_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                return Some((
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                ));
            }
        }
        None
    }
}

#[tauri::command]
async fn execute_action(action: Action) -> Result<String, String> {
    match action {
        Action::Launch {
            path,
            args,
            window_config,
        } => {
            // Check if process is already running
            let exe_name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            #[cfg(windows)]
            {
                let existing_windows = window_control::find_windows_by_process_name(&exe_name);

                if !existing_windows.is_empty() {
                    // App already running, just adjust window
                    if let Some(config) = window_config {
                        for hwnd in existing_windows {
                            if let Some((cur_x, cur_y, cur_w, cur_h)) =
                                window_control::get_window_rect(hwnd)
                            {
                                let x = config.x.unwrap_or(cur_x);
                                let y = config.y.unwrap_or(cur_y);
                                let w = config.width.unwrap_or(cur_w);
                                let h = config.height.unwrap_or(cur_h);
                                let _ = window_control::set_window_position(hwnd, x, y, w, h);
                            }
                        }
                    }
                    return Ok(format!("Adjusted window for already running: {}", exe_name));
                }
            }

            // Launch new process
            let mut cmd = Command::new(&path);
            if let Some(arguments) = args {
                cmd.args(arguments);
            }

            let child = cmd
                .spawn()
                .map_err(|e| format!("Failed to launch {}: {}", path, e))?;

            // If window config is set, wait a bit and then set window position
            #[cfg(windows)]
            if let Some(config) = window_config {
                let pid = child.id();
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                // Try to find and resize the window
                for _ in 0..10 {
                    if let Some(hwnd) = window_control::find_window_by_pid(pid) {
                        let x = config.x.unwrap_or(0);
                        let y = config.y.unwrap_or(0);
                        let w = config.width.unwrap_or(800);
                        let h = config.height.unwrap_or(600);
                        let _ = window_control::set_window_position(hwnd, x, y, w, h);
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            }

            Ok(format!("Launched: {}", path))
        }

        Action::Kill { process_name } => {
            let mut sys = System::new_all();
            sys.refresh_processes(ProcessesToUpdate::All, true);

            let mut killed_count = 0;
            for (_pid, process) in sys.processes() {
                if process.name().to_string_lossy().to_lowercase() == process_name.to_lowercase() {
                    process.kill();
                    killed_count += 1;
                }
            }

            if killed_count > 0 {
                Ok(format!(
                    "Killed {} instance(s) of {}",
                    killed_count, process_name
                ))
            } else {
                Err(format!("Process not found: {}", process_name))
            }
        }

        Action::OpenFolder { path, window_config } => {
            #[cfg(windows)]
            let before_windows: std::collections::HashSet<isize> = if window_config.is_some() {
                window_control::get_explorer_windows()
                    .iter()
                    .map(|(hwnd, _)| hwnd.0 as isize)
                    .collect()
            } else {
                std::collections::HashSet::new()
            };

            open::that(&path).map_err(|e| format!("Failed to open folder {}: {}", path, e))?;

            // If window config is set, wait and then set window position
            #[cfg(windows)]
            if let Some(config) = window_config {
                // Wait for the window to open
                tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

                // Find the new explorer window by comparing before and after
                let after_windows = window_control::get_explorer_windows();
                
                // Find windows that weren't there before
                let new_window = after_windows.iter().find(|(hwnd, _)| {
                    !before_windows.contains(&(hwnd.0 as isize))
                });

                // If we found a new window, use it; otherwise try to find by folder name
                let target_hwnd = if let Some((hwnd, _)) = new_window {
                    Some(*hwnd)
                } else {
                    // Fallback: find window with matching folder name in title
                    let folder_name = std::path::Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    
                    after_windows.iter()
                        .find(|(_, title)| title.contains(&folder_name))
                        .map(|(hwnd, _)| *hwnd)
                };

                if let Some(hwnd) = target_hwnd {
                    if let Some((cur_x, cur_y, cur_w, cur_h)) = window_control::get_window_rect(hwnd) {
                        let x = config.x.unwrap_or(cur_x);
                        let y = config.y.unwrap_or(cur_y);
                        let w = config.width.unwrap_or(cur_w);
                        let h = config.height.unwrap_or(cur_h);
                        let _ = window_control::set_window_position(hwnd, x, y, w, h);
                    }
                }
            }

            Ok(format!("Opened folder: {}", path))
        }

        Action::OpenUrl { url, window_config } => {
            #[cfg(windows)]
            let before_windows: std::collections::HashSet<isize> = if window_config.is_some() {
                // Get browser windows before opening
                let mut windows = std::collections::HashSet::new();
                for browser in ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"] {
                    for (hwnd, _) in window_control::get_windows_by_process_name(browser) {
                        windows.insert(hwnd.0 as isize);
                    }
                }
                windows
            } else {
                std::collections::HashSet::new()
            };

            // Open URL in new browser window
            #[cfg(windows)]
            {
                use winreg::enums::*;
                use winreg::RegKey;
                
                let mut opened = false;
                
                // Helper function to get browser path from registry
                fn get_browser_path(prog_id: &str) -> Option<String> {
                    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
                    let command_key = format!(r"{}\shell\open\command", prog_id);
                    if let Ok(key) = hkcr.open_subkey(&command_key) {
                        if let Ok(cmd) = key.get_value::<String, _>("") {
                            // Extract path from command like: "C:\...\chrome.exe" --args
                            let cmd = cmd.trim();
                            if cmd.starts_with('"') {
                                if let Some(end) = cmd[1..].find('"') {
                                    return Some(cmd[1..end+1].to_string());
                                }
                            } else if let Some(end) = cmd.find(' ') {
                                return Some(cmd[..end].to_string());
                            } else {
                                return Some(cmd.to_string());
                            }
                        }
                    }
                    None
                }
                
                // Try to get default browser from registry
                if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER)
                    .open_subkey(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice")
                {
                    if let Ok(prog_id) = hkcu.get_value::<String, _>("ProgId") {
                        if let Some(browser_path) = get_browser_path(&prog_id) {
                            let prog_id_lower = prog_id.to_lowercase();
                            
                            // Determine new-window flag based on browser
                            let new_window_arg = if prog_id_lower.contains("firefox") {
                                "-new-window"
                            } else {
                                "--new-window"
                            };
                            
                            if Command::new(&browser_path)
                                .args([new_window_arg, &url])
                                .spawn()
                                .is_ok()
                            {
                                opened = true;
                            }
                        }
                    }
                }
                
                // Fallback: try common browser paths
                if !opened {
                    let browser_paths = [
                        (r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe", "--new-window"),
                        (r"C:\Program Files\Microsoft\Edge\Application\msedge.exe", "--new-window"),
                        (r"C:\Program Files\Google\Chrome\Application\chrome.exe", "--new-window"),
                        (r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe", "--new-window"),
                        (r"C:\Program Files\Mozilla Firefox\firefox.exe", "-new-window"),
                        (r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe", "--new-window"),
                    ];
                    
                    for (path, arg) in browser_paths {
                        if std::path::Path::new(path).exists() {
                            if Command::new(path)
                                .args([arg, &url])
                                .spawn()
                                .is_ok()
                            {
                                opened = true;
                                break;
                            }
                        }
                    }
                }
                
                // Last resort: use start command (won't guarantee new window)
                if !opened {
                    let _ = Command::new("cmd")
                        .args(["/c", "start", "", &url])
                        .spawn()
                        .map_err(|e| format!("Failed to open URL {}: {}", url, e))?;
                }
            }

            #[cfg(not(windows))]
            {
                open::that(&url).map_err(|e| format!("Failed to open URL {}: {}", url, e))?;
            }

            // If window config is set, wait and then set window position
            #[cfg(windows)]
            if let Some(config) = window_config {
                tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

                // Find the new browser window by checking foreground window
                let mut target_hwnd: Option<windows::Win32::Foundation::HWND> = None;
                
                // First try: get the foreground window if it's a browser
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
                    let fg_hwnd = GetForegroundWindow();
                    if !fg_hwnd.is_invalid() {
                        // Check if foreground window belongs to a browser
                        for browser in ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"] {
                            let browser_windows = window_control::get_windows_by_process_name(browser);
                            for (hwnd, _) in &browser_windows {
                                if hwnd.0 == fg_hwnd.0 {
                                    target_hwnd = Some(*hwnd);
                                    break;
                                }
                            }
                            if target_hwnd.is_some() {
                                break;
                            }
                        }
                    }
                }
                
                // Second try: find new window by comparing before/after
                if target_hwnd.is_none() {
                    for browser in ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"] {
                        let after_windows = window_control::get_windows_by_process_name(browser);
                        for (hwnd, _) in after_windows {
                            if !before_windows.contains(&(hwnd.0 as isize)) {
                                target_hwnd = Some(hwnd);
                                break;
                            }
                        }
                        if target_hwnd.is_some() {
                            break;
                        }
                    }
                }

                if let Some(hwnd) = target_hwnd {
                    if let Some((cur_x, cur_y, cur_w, cur_h)) = window_control::get_window_rect(hwnd) {
                        let x = config.x.unwrap_or(cur_x);
                        let y = config.y.unwrap_or(cur_y);
                        let w = config.width.unwrap_or(cur_w);
                        let h = config.height.unwrap_or(cur_h);
                        let _ = window_control::set_window_position(hwnd, x, y, w, h);
                    }
                }
            }

            Ok(format!("Opened URL: {}", url))
        }

        Action::Delay { ms } => {
            tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
            Ok(format!("Delayed for {}ms", ms))
        }
    }
}

#[tauri::command]
async fn execute_shortcut(shortcut: Shortcut) -> Result<Vec<String>, String> {
    let mut results = Vec::new();

    for action in shortcut.actions {
        match execute_action(action).await {
            Ok(msg) => results.push(msg),
            Err(e) => results.push(format!("Error: {}", e)),
        }
    }

    Ok(results)
}

#[tauri::command]
fn get_shortcuts() -> Result<Vec<Shortcut>, String> {
    let data = load_app_data();
    Ok(data.shortcuts)
}

#[tauri::command]
fn save_shortcuts(shortcuts: Vec<Shortcut>) -> Result<(), String> {
    let mut data = load_app_data();
    data.shortcuts = shortcuts;
    save_app_data(&data)
}

#[tauri::command]
fn get_groups() -> Result<Vec<Group>, String> {
    let data = load_app_data();
    Ok(data.groups)
}

#[tauri::command]
fn save_groups(groups: Vec<Group>) -> Result<(), String> {
    let mut data = load_app_data();
    data.groups = groups;
    save_app_data(&data)
}

#[tauri::command]
fn get_process_list() -> Vec<ProcessInfo> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, process)| ProcessInfo {
            pid: pid.as_u32(),
            name: process.name().to_string_lossy().to_string(),
            memory_usage: process.memory(),
        })
        .collect();

    // Sort by name and deduplicate
    processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    processes.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

    processes
}

#[tauri::command]
fn get_app_data() -> Result<AppData, String> {
    Ok(load_app_data())
}

#[tauri::command]
fn save_app_data_cmd(data: AppData) -> Result<(), String> {
    save_app_data(&data)
}

// ========================================
// Installed Apps
// ========================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub name: String,
    pub path: String,
    pub icon: Option<String>,
}

#[tauri::command]
fn get_installed_apps() -> Vec<InstalledApp> {
    use std::collections::HashSet;
    use winreg::enums::*;
    use winreg::RegKey;

    let mut apps: Vec<InstalledApp> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    // Registry paths to check
    let registry_paths = [
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];

    for (hkey, path) in registry_paths {
        if let Ok(key) = RegKey::predef(hkey).open_subkey(path) {
            for subkey_name in key.enum_keys().filter_map(|k| k.ok()) {
                if let Ok(subkey) = key.open_subkey(&subkey_name) {
                    let display_name: Result<String, _> = subkey.get_value("DisplayName");
                    let install_location: Result<String, _> = subkey.get_value("InstallLocation");
                    let display_icon: Result<String, _> = subkey.get_value("DisplayIcon");

                    if let Ok(name) = display_name {
                        // Skip system components and duplicates
                        let name_lower = name.to_lowercase();
                        if seen_names.contains(&name_lower) {
                            continue;
                        }
                        if name.starts_with("KB")
                            || name.contains("Update")
                            || name.contains("Hotfix")
                        {
                            continue;
                        }

                        // Try to find executable path
                        let path = if let Ok(icon) = &display_icon {
                            // DisplayIcon often contains the exe path
                            let cleaned = icon
                                .trim_matches('"')
                                .split(',')
                                .next()
                                .unwrap_or("")
                                .to_string();
                            if cleaned.ends_with(".exe") && std::path::Path::new(&cleaned).exists()
                            {
                                cleaned
                            } else if let Ok(loc) = &install_location {
                                // Try to find exe in install location
                                find_exe_in_dir(loc)
                            } else {
                                String::new()
                            }
                        } else if let Ok(loc) = &install_location {
                            find_exe_in_dir(loc)
                        } else {
                            String::new()
                        };

                        if !path.is_empty() {
                            seen_names.insert(name_lower);
                            apps.push(InstalledApp {
                                name: name.clone(),
                                path,
                                icon: display_icon.ok(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Also add apps from Start Menu
    if let Some(start_menu) = dirs::data_dir() {
        let start_menu_path = start_menu.join(r"Microsoft\Windows\Start Menu\Programs");
        add_shortcuts_from_dir(&start_menu_path, &mut apps, &mut seen_names);
    }

    // Common Start Menu
    let common_start = PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs");
    if common_start.exists() {
        add_shortcuts_from_dir(&common_start, &mut apps, &mut seen_names);
    }

    // Sort by name
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    apps
}

fn find_exe_in_dir(dir: &str) -> String {
    use glob::glob;

    let pattern = format!("{}/*.exe", dir.trim_end_matches('\\'));
    if let Ok(entries) = glob(&pattern) {
        for entry in entries.filter_map(|e| e.ok()) {
            let file_name = entry
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase());
            // Skip uninstall, setup, update executables
            if let Some(name) = file_name {
                if !name.contains("unins") && !name.contains("setup") && !name.contains("update") {
                    return entry.to_string_lossy().to_string();
                }
            }
        }
    }
    String::new()
}

fn add_shortcuts_from_dir(
    dir: &PathBuf,
    apps: &mut Vec<InstalledApp>,
    seen: &mut std::collections::HashSet<String>,
) {
    use glob::glob;

    let pattern = format!("{}/**/*.lnk", dir.to_string_lossy());
    if let Ok(entries) = glob(&pattern) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Some(file_stem) = entry.file_stem() {
                let name = file_stem.to_string_lossy().to_string();
                let name_lower = name.to_lowercase();

                // Skip duplicates and uninstall shortcuts
                if seen.contains(&name_lower) || name_lower.contains("uninstall") {
                    continue;
                }

                seen.insert(name_lower);
                apps.push(InstalledApp {
                    name,
                    path: entry.to_string_lossy().to_string(),
                    icon: None,
                });
            }
        }
    }
}

#[tauri::command]
fn get_window_position(process_name: String) -> Result<WindowConfig, String> {
    #[cfg(windows)]
    {
        let windows = window_control::find_windows_by_process_name(&process_name);
        if let Some(hwnd) = windows.first() {
            if let Some((x, y, width, height)) = window_control::get_window_rect(*hwnd) {
                return Ok(WindowConfig {
                    x: Some(x),
                    y: Some(y),
                    width: Some(width),
                    height: Some(height),
                });
            }
        }
        Err(format!("No window found for process: {}", process_name))
    }

    #[cfg(not(windows))]
    {
        Err("Window position not supported on this platform".to_string())
    }
}

#[tauri::command]
fn get_window_list() -> Vec<WindowInfo> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsWindowVisible, GetWindow, GW_OWNER,
        };
        use sysinfo::{ProcessesToUpdate, System};
        use std::collections::HashMap;

        // プロセス情報を取得
        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        
        let process_map: HashMap<u32, String> = sys
            .processes()
            .iter()
            .map(|(pid, p)| (pid.as_u32(), p.name().to_string_lossy().to_string()))
            .collect();

        let mut results: Vec<WindowInfo> = Vec::new();

        unsafe {
            // EnumWindowsで全ウィンドウを列挙
            let _ = EnumWindows(
                Some(enum_windows_callback),
                LPARAM(&mut results as *mut Vec<WindowInfo> as isize),
            );
        }

        // プロセス名を追加
        for info in &mut results {
            if let Some(name) = process_map.get(&info.pid) {
                info.process_name = name.clone();
            }
        }

        return results;

        unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // 可視ウィンドウのみ
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }

            // 所有者ウィンドウがあるウィンドウはスキップ（ポップアップ等）
            if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
                if owner.0 as usize != 0 {
                    return BOOL(1);
                }
            }

            // タイトルを取得
            let title_len = GetWindowTextLengthW(hwnd);
            if title_len == 0 {
                return BOOL(1);
            }

            let mut title_buf = vec![0u16; (title_len + 1) as usize];
            GetWindowTextW(hwnd, &mut title_buf);
            let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

            // PIDを取得
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            // ウィンドウサイズを取得
            let mut rect = RECT::default();
            let (x, y, width, height) = if GetWindowRect(hwnd, &mut rect).is_ok() {
                (
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                )
            } else {
                return BOOL(1);
            };

            // 小さすぎるウィンドウはスキップ
            if width < 100 || height < 50 {
                return BOOL(1);
            }

            let info = WindowInfo {
                hwnd: hwnd.0 as i64,
                title,
                process_name: String::new(), // 後で設定
                pid,
                x,
                y,
                width,
                height,
            };

            let results = &mut *(lparam.0 as *mut Vec<WindowInfo>);
            results.push(info);

            BOOL(1)
        }
    }

    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[derive(Serialize)]
pub struct ShortcutResolveResponse {
    path: String,
    args: String,
}

// ========================================
// Desktop Shortcut Creation
// ========================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopShortcutOptions {
    pub name: String,
    pub icon_path: Option<String>,
    pub icon_index: Option<i32>,
    pub custom_icon_data: Option<String>, // Base64 encoded image data
    pub border_radius: u32, // 0-50 for percentage
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDesktopShortcutRequest {
    pub shortcut_id: String,
    pub target_path: String,
    pub options: DesktopShortcutOptions,
}

#[tauri::command]
fn create_desktop_shortcut(
    app_handle: tauri::AppHandle,
    request: CreateDesktopShortcutRequest,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        create_windows_shortcut(&app_handle, &request)
    }

    #[cfg(not(windows))]
    {
        Err("Desktop shortcuts are only supported on Windows".to_string())
    }
}

#[cfg(windows)]
fn create_windows_shortcut(
    _app_handle: &tauri::AppHandle,
    request: &CreateDesktopShortcutRequest,
) -> Result<String, String> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::IPersistFile;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    // Get the current executable path
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
    let exe_path_str = exe_path.to_string_lossy().to_string();

    // Create arguments for the shortcut execution
    let args = format!(
        "--execute-shortcut {}",
        request.shortcut_id
    );

    // Process icon if custom icon data is provided
    let icon_path = if let Some(icon_data) = &request.options.custom_icon_data {
        // Create icons directory in app data
        let icons_dir = get_icons_dir();
        if !icons_dir.exists() {
            fs::create_dir_all(&icons_dir)
                .map_err(|e| format!("Failed to create icons directory: {}", e))?;
        }

        // Create ICO file from the base64 image data
        let ico_path = icons_dir.join(format!("{}.ico", request.shortcut_id));
        create_ico_from_base64(icon_data, &ico_path, request.options.border_radius)?;
        Some(ico_path.to_string_lossy().to_string())
    } else {
        request.options.icon_path.clone()
    };

    unsafe {
        // Initialize COM
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        // Create ShellLink instance
        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("Failed to create ShellLink: {}", e))?;

        // Set target path (the app executable)
        let wide_target: Vec<u16> = exe_path_str.encode_utf16().chain(std::iter::once(0)).collect();
        shell_link
            .SetPath(PCWSTR(wide_target.as_ptr()))
            .map_err(|e| format!("Failed to set path: {}", e))?;

        // Set arguments
        let wide_args: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
        shell_link
            .SetArguments(PCWSTR(wide_args.as_ptr()))
            .map_err(|e| format!("Failed to set arguments: {}", e))?;

        // Set working directory
        let working_dir = exe_path.parent().unwrap_or(&exe_path);
        let wide_working_dir: Vec<u16> = working_dir
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        shell_link
            .SetWorkingDirectory(PCWSTR(wide_working_dir.as_ptr()))
            .map_err(|e| format!("Failed to set working directory: {}", e))?;

        // Set icon
        if let Some(icon) = &icon_path {
            let wide_icon: Vec<u16> = icon.encode_utf16().chain(std::iter::once(0)).collect();
            let icon_index = request.options.icon_index.unwrap_or(0);
            shell_link
                .SetIconLocation(PCWSTR(wide_icon.as_ptr()), icon_index)
                .map_err(|e| format!("Failed to set icon: {}", e))?;
        }

        // Set description
        let description = format!("Advanced Shortcut - {}", request.options.name);
        let wide_desc: Vec<u16> = description.encode_utf16().chain(std::iter::once(0)).collect();
        shell_link
            .SetDescription(PCWSTR(wide_desc.as_ptr()))
            .map_err(|e| format!("Failed to set description: {}", e))?;

        // Get IPersistFile interface and save
        let persist_file: IPersistFile = shell_link
            .cast()
            .map_err(|e| format!("Failed to get IPersistFile: {}", e))?;

        // Ensure target path ends with .lnk
        let lnk_path = if request.target_path.ends_with(".lnk") {
            request.target_path.clone()
        } else {
            format!("{}.lnk", request.target_path)
        };

        let wide_lnk_path: Vec<u16> = lnk_path.encode_utf16().chain(std::iter::once(0)).collect();
        persist_file
            .Save(PCWSTR(wide_lnk_path.as_ptr()), true)
            .map_err(|e| format!("Failed to save shortcut: {}", e))?;

        CoUninitialize();
    }

    Ok(format!("Created shortcut: {}", request.target_path))
}

#[cfg(windows)]
fn get_icons_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("advanced-shortcut")
        .join("icons")
}

#[cfg(windows)]
fn create_ico_from_base64(base64_data: &str, output_path: &PathBuf, border_radius: u32) -> Result<(), String> {
    // Decode base64 data
    let image_data = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Load image
    let img = image::load_from_memory(&image_data)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    // Resize to standard icon sizes
    let sizes = [256, 128, 64, 48, 32, 16];
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for size in sizes {
        // Resize image
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let mut rgba_img = resized.to_rgba8();

        // Apply border radius if > 0
        if border_radius > 0 {
            apply_rounded_corners(&mut rgba_img, border_radius);
        }

        // Convert to ICO image entry
        let width = rgba_img.width();
        let height = rgba_img.height();
        let raw_data = rgba_img.into_raw();

        let ico_image = ico::IconImage::from_rgba_data(width, height, raw_data);
        icon_dir.add_entry(ico::IconDirEntry::encode(&ico_image)
            .map_err(|e| format!("Failed to encode icon: {}", e))?);
    }

    // Save ICO file
    let file = fs::File::create(output_path)
        .map_err(|e| format!("Failed to create ico file: {}", e))?;
    icon_dir.write(file)
        .map_err(|e| format!("Failed to write ico file: {}", e))?;

    Ok(())
}

#[cfg(windows)]
fn apply_rounded_corners(img: &mut RgbaImage, radius_percent: u32) {
    let width = img.width();
    let height = img.height();
    let radius = (width.min(height) as f64 * radius_percent as f64 / 100.0) as u32;

    if radius == 0 {
        return;
    }

    for y in 0..height {
        for x in 0..width {
            let in_corner = is_in_rounded_corner(x, y, width, height, radius);
            if !in_corner {
                continue;
            }

            // Calculate distance from corner center
            let (cx, cy) = get_corner_center(x, y, width, height, radius);
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance > radius as f64 {
                // Outside rounded corner - make transparent
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else if distance > (radius as f64 - 1.5) {
                // Anti-aliasing at the edge
                let alpha = ((radius as f64 - distance) / 1.5 * 255.0).clamp(0.0, 255.0) as u8;
                let mut pixel = *img.get_pixel(x, y);
                pixel[3] = (pixel[3] as u32 * alpha as u32 / 255) as u8;
                img.put_pixel(x, y, pixel);
            }
        }
    }
}

#[cfg(windows)]
fn is_in_rounded_corner(x: u32, y: u32, width: u32, height: u32, radius: u32) -> bool {
    // Top-left
    if x < radius && y < radius {
        return true;
    }
    // Top-right
    if x >= width - radius && y < radius {
        return true;
    }
    // Bottom-left
    if x < radius && y >= height - radius {
        return true;
    }
    // Bottom-right
    if x >= width - radius && y >= height - radius {
        return true;
    }
    false
}

#[cfg(windows)]
fn get_corner_center(x: u32, y: u32, width: u32, height: u32, radius: u32) -> (f64, f64) {
    // Top-left
    if x < radius && y < radius {
        return (radius as f64, radius as f64);
    }
    // Top-right
    if x >= width - radius && y < radius {
        return ((width - radius) as f64, radius as f64);
    }
    // Bottom-left
    if x < radius && y >= height - radius {
        return (radius as f64, (height - radius) as f64);
    }
    // Bottom-right
    ((width - radius) as f64, (height - radius) as f64)
}

#[tauri::command]
fn get_desktop_path() -> Result<String, String> {
    dirs::desktop_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Failed to get desktop path".to_string())
}

#[tauri::command]
fn resolve_shortcut_link(lnk_path: String) -> Result<ShortcutResolveResponse, String> {
    #[cfg(windows)]
    {
        use windows::core::{Interface, PCWSTR};
        use windows::Win32::System::Com::IPersistFile;
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

        unsafe {
            // Initialize COM
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            // Create ShellLink instance
            let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("Failed to create ShellLink: {}", e))?;

            // Get IPersistFile interface
            let persist_file: IPersistFile = shell_link
                .cast()
                .map_err(|e| format!("Failed to get IPersistFile: {}", e))?;

            // Convert path to wide string
            let wide_path: Vec<u16> = lnk_path.encode_utf16().chain(std::iter::once(0)).collect();

            // Load the shortcut
            use windows::Win32::System::Com::STGM_READ;
            persist_file
                .Load(PCWSTR(wide_path.as_ptr()), STGM_READ)
                .map_err(|e| format!("Failed to load shortcut: {}", e))?;

            // Get target path
            let mut target_path = [0u16; 260];
            let mut find_data =
                std::mem::zeroed::<windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW>();
            shell_link
                .GetPath(&mut target_path, &mut find_data, 0)
                .map_err(|e| format!("Failed to get path: {}", e))?;

            // Get arguments
            let mut arguments = [0u16; 1024];
            shell_link
                .GetArguments(&mut arguments)
                .map_err(|e| format!("Failed to get arguments: {}", e))?;

            CoUninitialize();

            // Convert wide string to String
            let path_end = target_path
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(target_path.len());
            let target = String::from_utf16_lossy(&target_path[..path_end]);

            let args_end = arguments
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(arguments.len());
            let args = String::from_utf16_lossy(&arguments[..args_end]);

            if target.is_empty() {
                return Err("Could not resolve shortcut target".to_string());
            }

            Ok(ShortcutResolveResponse { path: target, args })
        }
    }

    #[cfg(not(windows))]
    {
        Err("Shortcut resolution not supported on this platform".to_string())
    }
}

// ========================================
// Plugin Setup
// ========================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // CLI引数をチェック
    if let Some(cli_request) = parse_cli_shortcut_request() {
        // CLI引数がある場合はフロントエンドを起動せずに直接ショートカットを実行
        match execute_shortcut_from_cli(&cli_request) {
            Ok(_) => {
                // 正常終了
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Error executing shortcut: {}", e);
                std::process::exit(1);
            }
        }
    }

    // CLI引数がない場合は通常のGUIアプリとして起動
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            execute_action,
            execute_shortcut,
            get_shortcuts,
            save_shortcuts,
            get_groups,
            save_groups,
            get_process_list,
            get_installed_apps,
            get_window_position,
            get_window_list,
            resolve_shortcut_link,
            get_app_data,
            save_app_data_cmd,
            create_desktop_shortcut,
            get_desktop_path,
            exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
