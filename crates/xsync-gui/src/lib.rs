use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;
use tokio::runtime::Runtime;
use xsync_core::auth::{execute_oauth_login, load_stored_token, save_stored_token, token_path, DEFAULT_CLIENT_ID, DEFAULT_CLIENT_SECRET};
use xsync_core::config::{add_or_update_directory, load_config, save_config};
use xsync_core::db::Database;
use xsync_core::drive::DriveClient;
use xsync_core::sync::{SyncCoordinator, SyncOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedDirDto {
    pub local_path: String,
    pub drive_folder_id: String,
    pub drive_folder_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFileDto {
    pub rel_path: String,
    pub size_str: String,
    pub time_str: String,
    pub is_syncing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedFileDto {
    pub name: String,
    pub rel_path: String,
    pub full_path: String,
    pub local_root: String,
    pub folder_name: String,
    pub size_bytes: u64,
    pub size_str: String,
    pub modified_secs: i64,
    pub time_str: String,
    pub synced_time_str: String,
    pub extension: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CategoryStatsDto {
    pub documents_bytes: u64,
    pub documents_count: usize,
    pub images_bytes: u64,
    pub images_count: usize,
    pub videos_bytes: u64,
    pub videos_count: usize,
    pub audio_bytes: u64,
    pub audio_count: usize,
    pub code_bytes: u64,
    pub code_count: usize,
    pub archives_bytes: u64,
    pub archives_count: usize,
    pub others_bytes: u64,
    pub others_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffItemDto {
    pub path: String,
    pub side: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateDto {
    pub is_connected: bool,
    pub is_syncing: bool,
    pub is_paused: bool,
    pub user_name: String,
    pub user_email: String,
    pub status_message: String,
    pub storage_used_str: String,
    pub storage_total_str: String,
    pub storage_ratio: f32,
    pub total_directories: usize,
    pub total_files_tracked: usize,
    pub total_bytes_str: String,
    pub directories: Vec<WatchedDirDto>,
}

pub struct GuiContext {
    pub is_paused: Arc<AtomicBool>,
    pub is_syncing: Arc<AtomicBool>,
    pub status_message: Arc<std::sync::Mutex<String>>,
    pub rt: Arc<Runtime>,
}

struct SyncGuard(Arc<AtomicBool>);
impl Drop for SyncGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_timestamp(ts: i64) -> String {
    if let Some(utc) = DateTime::<Utc>::from_timestamp(ts, 0) {
        let local: DateTime<Local> = DateTime::from(utc);
        local.format("%b %d, %H:%M").to_string()
    } else {
        "Just now".to_string()
    }
}

fn format_relative_timestamp(ts: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let diff = now.saturating_sub(ts);
    if diff < 15 {
        "Just now".to_string()
    } else if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if let Some(utc) = DateTime::<Utc>::from_timestamp(ts, 0) {
        let local: DateTime<Local> = DateTime::from(utc);
        local.format("%b %d, %H:%M").to_string()
    } else {
        "Recently".to_string()
    }
}

static AUTH_CACHE: std::sync::Mutex<Option<(std::time::Instant, bool, String, String, String, String, f32)>> =
    std::sync::Mutex::new(None);
static STATS_CACHE: std::sync::Mutex<Option<(std::time::Instant, CategoryStatsDto)>> =
    std::sync::Mutex::new(None);

#[tauri::command]
async fn get_app_state(state: State<'_, Arc<GuiContext>>) -> Result<AppStateDto, String> {
    let mut is_connected = false;
    let mut user_name = "Not Connected".to_string();
    let mut user_email = "Sign in via Google OAuth".to_string();
    let mut storage_used_str = "0 GB".to_string();
    let mut storage_total_str = "15 GB".to_string();
    let mut storage_ratio = 0.0f32;

    let should_check_network = {
        let cache = AUTH_CACHE.lock().unwrap();
        if let Some((instant, conn, ref name, ref email, ref used, ref total, ratio)) = *cache {
            if instant.elapsed() < std::time::Duration::from_secs(if conn { 60 } else { 15 }) {
                is_connected = conn;
                user_name = name.clone();
                user_email = email.clone();
                storage_used_str = used.clone();
                storage_total_str = total.clone();
                storage_ratio = ratio;
                false
            } else {
                true
            }
        } else {
            true
        }
    };

    if should_check_network {
        match load_stored_token() {
            Ok(Some(_)) => match DriveClient::from_auth().await {
                Ok(drive) => match drive.get_about().await {
                    Ok(about) => {
                        is_connected = true;
                        if let Some(u) = about.user {
                            user_name = u.display_name;
                            user_email = u.email_address;
                        }
                        if let Some(quota) = about.storage_quota {
                            let used = quota.usage_bytes();
                            let limit = quota.limit_bytes();
                            storage_used_str = format_bytes(used);
                            storage_total_str = format_bytes(limit);
                            if limit > 0 {
                                storage_ratio = (used as f32 / limit as f32).clamp(0.0, 1.0);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[xsync-gui] drive.get_about failed: {:#}", e);
                    }
                },
                Err(e) => {
                    eprintln!("[xsync-gui] DriveClient::from_auth failed: {:#}", e);
                }
            },
            Ok(None) => {}
            Err(e) => {
                eprintln!("[xsync-gui] load_stored_token error: {:#}", e);
            }
        }

        let mut cache = AUTH_CACHE.lock().unwrap();
        *cache = Some((
            std::time::Instant::now(),
            is_connected,
            user_name.clone(),
            user_email.clone(),
            storage_used_str.clone(),
            storage_total_str.clone(),
            storage_ratio,
        ));
    }

    let mut dir_items: Vec<WatchedDirDto> = Vec::new();
    if let Ok(cfg) = load_config() {
        for d in cfg.directories {
            dir_items.push(WatchedDirDto {
                local_path: d.local_path.to_string_lossy().to_string(),
                drive_folder_id: d.drive_folder_id,
                drive_folder_name: d.drive_folder_name.unwrap_or_else(|| "Google Drive".into()),
            });
        }
    }

    let mut total_files_tracked = 0usize;
    let mut total_bytes_str = "0 MB".to_string();

    if let Ok(db) = Database::open_default() {
        if let Ok(stats) = db.get_db_stats() {
            total_files_tracked = stats.total_files as usize;
            total_bytes_str = format_bytes(stats.total_size_bytes);
        }
    }

    let is_paused = state.is_paused.load(Ordering::SeqCst);
    let is_syncing = state.is_syncing.load(Ordering::SeqCst);

    let status_message = if is_paused {
        "Sync is paused".to_string()
    } else if is_syncing {
        let msg = state.status_message.lock().unwrap().clone();
        if msg.is_empty() || msg == "All files up to date" {
            "Synchronizing files...".to_string()
        } else {
            msg
        }
    } else if is_connected {
        let msg = state.status_message.lock().unwrap().clone();
        if msg.is_empty() {
            "All files up to date".to_string()
        } else {
            msg
        }
    } else {
        "Google Drive not connected".to_string()
    };

    Ok(AppStateDto {
        is_connected,
        is_syncing,
        is_paused,
        user_name,
        user_email,
        status_message,
        storage_used_str,
        storage_total_str,
        storage_ratio,
        total_directories: dir_items.len(),
        total_files_tracked,
        total_bytes_str,
        directories: dir_items,
    })
}

#[tauri::command]
async fn get_recent_activity(limit: Option<usize>) -> Result<Vec<RecentFileDto>, String> {
    let mut recent_items = Vec::new();
    let l = limit.unwrap_or(50);

    if let Ok(db) = Database::open_default() {
        if let Ok(recent) = db.get_recent_files(l) {
            for f in recent {
                recent_items.push(RecentFileDto {
                    rel_path: f.rel_path.to_string_lossy().to_string(),
                    size_str: format_bytes(f.size_bytes),
                    time_str: format_relative_timestamp(f.last_sync_timestamp),
                    is_syncing: false,
                });
            }
        }
    }

    Ok(recent_items)
}

fn categorize_ext(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp" | "ico" | "tiff" | "heic" => "image",
        "pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "odt" | "xls" | "xlsx" | "csv" | "ppt" | "pptx" | "epub" => "document",
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv" | "wmv" | "m4v" => "video",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" => "audio",
        "rs" | "ts" | "js" | "svelte" | "py" | "json" | "toml" | "yaml" | "yml" | "html" | "css" | "sh" | "c" | "cpp" | "h" | "go" | "java" | "sql" | "xml" => "code",
        "zip" | "tar" | "gz" | "7z" | "bz2" | "xz" | "rar" | "deb" | "rpm" => "archive",
        _ => "other",
    }
}

#[tauri::command]
async fn get_all_files(limit: Option<usize>) -> Result<Vec<DetailedFileDto>, String> {
    let mut items = Vec::new();
    let l = limit.unwrap_or(300);

    if let Ok(db) = Database::open_default() {
        if let Ok(files) = db.get_recent_files(l) {
            for f in files {
                let name = f
                    .rel_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| f.rel_path.to_string_lossy().to_string());
                let ext = f
                    .rel_path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let category = categorize_ext(&ext).to_string();
                let folder_name = f
                    .local_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Folder".to_string());
                let full_path = f.local_root.join(&f.rel_path).to_string_lossy().to_string();

                items.push(DetailedFileDto {
                    name,
                    rel_path: f.rel_path.to_string_lossy().to_string(),
                    full_path,
                    local_root: f.local_root.to_string_lossy().to_string(),
                    folder_name,
                    size_bytes: f.size_bytes,
                    size_str: format_bytes(f.size_bytes),
                    modified_secs: f.modified_secs,
                    time_str: format_timestamp(f.modified_secs),
                    synced_time_str: format_relative_timestamp(f.last_sync_timestamp),
                    extension: ext,
                    category,
                });
            }
        }
    }

    Ok(items)
}

#[tauri::command]
async fn get_category_stats() -> Result<CategoryStatsDto, String> {
    {
        let cache = STATS_CACHE.lock().unwrap();
        if let Some((instant, ref stats)) = *cache {
            if instant.elapsed() < std::time::Duration::from_secs(30) {
                return Ok(stats.clone());
            }
        }
    }

    let mut stats = CategoryStatsDto::default();

    if let Ok(db) = Database::open_default() {
        if let Ok(files) = db.get_recent_files(10000) {
            for f in files {
                let ext = f
                    .rel_path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let category = categorize_ext(&ext);
                match category {
                    "document" => {
                        stats.documents_bytes += f.size_bytes;
                        stats.documents_count += 1;
                    }
                    "image" => {
                        stats.images_bytes += f.size_bytes;
                        stats.images_count += 1;
                    }
                    "video" => {
                        stats.videos_bytes += f.size_bytes;
                        stats.videos_count += 1;
                    }
                    "audio" => {
                        stats.audio_bytes += f.size_bytes;
                        stats.audio_count += 1;
                    }
                    "code" => {
                        stats.code_bytes += f.size_bytes;
                        stats.code_count += 1;
                    }
                    "archive" => {
                        stats.archives_bytes += f.size_bytes;
                        stats.archives_count += 1;
                    }
                    _ => {
                        stats.others_bytes += f.size_bytes;
                        stats.others_count += 1;
                    }
                }
            }
        }
    }

    let mut cache = STATS_CACHE.lock().unwrap();
    *cache = Some((std::time::Instant::now(), stats.clone()));

    Ok(stats)
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    let target = if p.is_file() {
        p.parent().map(|d| d.to_path_buf()).unwrap_or(p)
    } else {
        p
    };
    std::process::Command::new("xdg-open")
        .arg(&target)
        .spawn()
        .map_err(|e| format!("Failed to open file manager: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn sync_all(state: State<'_, Arc<GuiContext>>) -> Result<String, String> {
    if state.is_paused.load(Ordering::SeqCst) {
        return Err("Sync is currently paused. Please resume first.".into());
    }

    if state.is_syncing.load(Ordering::SeqCst) {
        return Ok("Sync already in progress.".into());
    }

    state.is_syncing.store(true, Ordering::SeqCst);
    let is_syncing_clone = state.is_syncing.clone();
    let status_msg_clone = state.status_message.clone();

    tokio::spawn(async move {
        let _guard = SyncGuard(is_syncing_clone);

        {
            let mut msg = status_msg_clone.lock().unwrap();
            *msg = "Connecting to Google Drive...".to_string();
        }

        let config = load_config().unwrap_or_default();
        match DriveClient::from_auth().await {
            Ok(drive) => match Database::open_default() {
                Ok(db) => {
                    let opts = SyncOptions {
                        dry_run: false,
                        permanent_delete: false,
                        concurrency: 4,
                        show_progress: false,
                        force: false,
                    };

                    for dir in config.directories {
                        if !dir.local_path.exists() {
                            continue;
                        }
                        let folder_name = dir
                            .local_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Folder".to_string());

                        {
                            let mut msg = status_msg_clone.lock().unwrap();
                            *msg = format!("Syncing {}...", folder_name);
                        }

                        if let Ok(coord) = SyncCoordinator::new(
                            &dir.local_path,
                            dir.drive_folder_id,
                            drive.clone(),
                            db.clone(),
                        ) {
                            let _ = coord.reconcile_with_options(&opts).await;
                        }
                    }

                    *STATS_CACHE.lock().unwrap() = None;
                    {
                        let mut msg = status_msg_clone.lock().unwrap();
                        *msg = "All files up to date".to_string();
                    }
                }
                Err(e) => {
                    eprintln!("[xsync-gui] Database open error during sync: {:#}", e);
                    let mut msg = status_msg_clone.lock().unwrap();
                    *msg = "Database error during sync".to_string();
                }
            },
            Err(e) => {
                eprintln!("[xsync-gui] DriveClient auth error during sync: {:#}", e);
                let mut msg = status_msg_clone.lock().unwrap();
                *msg = "Authentication error during sync".to_string();
            }
        }
    });

    Ok("Sync triggered successfully.".into())
}

#[tauri::command]
fn toggle_pause(state: State<'_, Arc<GuiContext>>) -> Result<bool, String> {
    let current = state.is_paused.load(Ordering::SeqCst);
    let next = !current;
    state.is_paused.store(next, Ordering::SeqCst);
    Ok(next)
}

#[tauri::command]
async fn browse_folder() -> Result<Option<String>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select Local Folder to Sync")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

#[tauri::command]
async fn add_directory(local_path: String, remote_name: String) -> Result<String, String> {
    let local = PathBuf::from(&local_path);
    if !local.exists() {
        return Err(format!("Directory {:?} does not exist", local));
    }

    let drive = DriveClient::from_auth()
        .await
        .map_err(|e| format!("Google Drive auth error: {}", e))?;

    let folder_name = if remote_name.trim().is_empty() {
        local
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "xsync_folder".to_string())
    } else {
        remote_name.trim().to_string()
    };

    let folder_id = match drive.find_child_by_name("root", &folder_name).await {
        Ok(Some(existing)) if existing.is_folder() => existing.id,
        _ => {
            let created = drive
                .create_folder("root", &folder_name)
                .await
                .map_err(|e| format!("Failed to create folder on Drive: {}", e))?;
            created.id
        }
    };

    add_or_update_directory(&local, &folder_id, Some(folder_name))
        .map_err(|e| format!("Failed to save config: {}", e))?;

    *STATS_CACHE.lock().unwrap() = None;
    Ok("Folder added successfully".into())
}

#[tauri::command]
fn remove_directory(local_path: String) -> Result<String, String> {
    let mut config = load_config().map_err(|e| format!("Failed to load config: {}", e))?;
    config
        .directories
        .retain(|d| d.local_path != PathBuf::from(&local_path));
    save_config(&config).map_err(|e| format!("Failed to save config: {}", e))?;
    *STATS_CACHE.lock().unwrap() = None;
    Ok("Folder removed successfully".into())
}

#[tauri::command]
async fn run_diff(folder1: String, folder2: String) -> Result<Vec<DiffItemDto>, String> {
    let drive = DriveClient::from_auth()
        .await
        .map_err(|e| format!("Auth error: {}", e))?;

    let id1 = if folder1.trim().is_empty() || folder1 == "root" {
        "root".to_string()
    } else {
        match drive.find_child_by_name("root", folder1.trim()).await {
            Ok(Some(f)) => f.id,
            _ => folder1.trim().to_string(),
        }
    };

    let id2 = match drive.find_child_by_name("root", folder2.trim()).await {
        Ok(Some(f)) => f.id,
        _ => folder2.trim().to_string(),
    };

    let f1_files = drive
        .list_files_recursive(&id1)
        .await
        .map_err(|e| format!("Failed listing folder 1: {}", e))?;
    let f2_files = drive
        .list_files_recursive(&id2)
        .await
        .map_err(|e| format!("Failed listing folder 2: {}", e))?;

    use std::collections::HashMap;
    let mut map1 = HashMap::new();
    for (p, f) in f1_files {
        map1.insert(p, f.md5_checksum);
    }

    let mut map2 = HashMap::new();
    for (p, f) in f2_files {
        map2.insert(p, f.md5_checksum);
    }

    let mut diffs = Vec::new();
    for (p, md5_1) in &map1 {
        match map2.get(p) {
            Some(md5_2) => {
                if md5_1 != md5_2 {
                    diffs.push(DiffItemDto {
                        path: p.to_string_lossy().to_string(),
                        side: "modified".into(),
                    });
                }
            }
            None => {
                diffs.push(DiffItemDto {
                    path: p.to_string_lossy().to_string(),
                    side: "only_in_source".into(),
                });
            }
        }
    }

    for p in map2.keys() {
        if !map1.contains_key(p) {
            diffs.push(DiffItemDto {
                path: p.to_string_lossy().to_string(),
                side: "only_in_dest".into(),
            });
        }
    }

    Ok(diffs)
}

#[tauri::command]
async fn run_merge(source: String, destination: String) -> Result<String, String> {
    let drive = DriveClient::from_auth()
        .await
        .map_err(|e| format!("Auth error: {}", e))?;

    let src_id = if source.trim().is_empty() || source == "root" {
        "root".to_string()
    } else {
        match drive.find_child_by_name("root", source.trim()).await {
            Ok(Some(f)) => f.id,
            _ => source.trim().to_string(),
        }
    };

    let dest_id = match drive.find_child_by_name("root", destination.trim()).await {
        Ok(Some(f)) => f.id,
        _ => destination.trim().to_string(),
    };

    let src_files = drive
        .list_files_recursive(&src_id)
        .await
        .map_err(|e| format!("Failed listing source: {}", e))?;
    let dest_files = drive
        .list_files_recursive(&dest_id)
        .await
        .map_err(|e| format!("Failed listing destination: {}", e))?;

    use std::collections::HashSet;
    let mut dest_set = HashSet::new();
    for (p, _) in dest_files {
        dest_set.insert(p);
    }

    let mut copied_count = 0;
    for (p, f) in src_files {
        if !dest_set.contains(&p) {
            let _ = drive.copy_file(&f.id, &dest_id, &f.name).await;
            copied_count += 1;
        }
    }

    Ok(format!("Successfully merged {} missing files.", copied_count))
}

#[tauri::command]
fn systemd_action(action: String) -> Result<String, String> {
    match action.as_str() {
        "install" => {
            let service_content = r#"[Unit]
Description=xsync - Linux-Native Git-Aware Realtime Google Drive Sync Daemon
After=network.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/xsync watch
Restart=on-failure
RestartSec=10

[Install]
WantedBy=default.target
"#;
            if let Some(cfg) = dirs::config_dir() {
                let user_dir = cfg.join("systemd/user");
                let _ = std::fs::create_dir_all(&user_dir);
                let _ = std::fs::write(user_dir.join("xsync.service"), service_content);
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .status();
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "enable", "--now", "xsync.service"])
                    .status();
            }
            Ok("Service installed and enabled.".into())
        }
        "start" => {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "start", "xsync.service"])
                .status();
            Ok("Service started.".into())
        }
        "stop" => {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "stop", "xsync.service"])
                .status();
            Ok("Service stopped.".into())
        }
        "restart" => {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "restart", "xsync.service"])
                .status();
            Ok("Service restarted.".into())
        }
        _ => Err("Invalid action".into()),
    }
}

#[tauri::command]
async fn login() -> Result<String, String> {
    let cfg = load_config().unwrap_or_default();
    let (cid, csec) = if let Some(ref id) = cfg.client_id {
        (id.clone(), cfg.client_secret.clone())
    } else {
        (
            DEFAULT_CLIENT_ID.to_string(),
            DEFAULT_CLIENT_SECRET.map(|s| s.to_string()),
        )
    };

    let token = execute_oauth_login(&cid, csec)
        .await
        .map_err(|e| format!("OAuth login error: {}", e))?;

    save_stored_token(&token).map_err(|e| format!("Failed to save token: {}", e))?;
    *AUTH_CACHE.lock().unwrap() = None;
    Ok("Authentication successful!".into())
}

#[tauri::command]
fn logout() -> Result<String, String> {
    if let Ok(p) = token_path() {
        let _ = std::fs::remove_file(p);
    }
    *AUTH_CACHE.lock().unwrap() = None;
    Ok("Logged out successfully.".into())
}

pub fn run() {
    let is_paused = Arc::new(AtomicBool::new(false));
    let is_syncing = Arc::new(AtomicBool::new(false));
    let status_message = Arc::new(std::sync::Mutex::new("All files up to date".to_string()));
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio Runtime"));

    let context = Arc::new(GuiContext {
        is_paused: is_paused.clone(),
        is_syncing: is_syncing.clone(),
        status_message: status_message.clone(),
        rt: rt.clone(),
    });

    // Start background inotify watcher daemon for continuous real-time sync
    {
        let is_paused_watcher = is_paused.clone();
        let is_syncing_watcher = is_syncing.clone();
        let status_msg_watcher = status_message.clone();

        rt.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let Ok(drive) = DriveClient::from_auth().await else {
                return;
            };
            let Ok(db) = Database::open_default() else {
                return;
            };
            let config = load_config().unwrap_or_default();

            let mut debouncers = Vec::new();

            for dir in config.directories {
                if !dir.local_path.exists() {
                    continue;
                }

                let debounce_ms = dir.debounce_ms.max(500);
                let Ok(watcher) = xsync_core::watcher::FsWatcher::new(
                    &dir.local_path,
                    std::time::Duration::from_millis(debounce_ms),
                ) else {
                    continue;
                };

                let Ok(coord) = SyncCoordinator::new(
                    &dir.local_path,
                    dir.drive_folder_id.clone(),
                    drive.clone(),
                    db.clone(),
                ) else {
                    continue;
                };

                let (tx, mut rx) = tokio::sync::mpsc::channel(100);
                let Ok(debouncer) = watcher.start_watching(tx) else {
                    continue;
                };
                debouncers.push(debouncer);

                let is_paused_inner = is_paused_watcher.clone();
                let is_syncing_inner = is_syncing_watcher.clone();
                let status_msg_inner = status_msg_watcher.clone();
                let coord = Arc::new(coord);

                tokio::spawn(async move {
                    while let Some(change) = rx.recv().await {
                        if is_paused_inner.load(Ordering::SeqCst) {
                            continue;
                        }

                        // Directory changes or folder creations are handled silently in the background
                        // Only actual file uploads/deletions trigger the visible syncing state
                        let is_file_event = match &change {
                            xsync_core::watcher::FsChange::CreateOrModify(p) => p.is_file(),
                            xsync_core::watcher::FsChange::Delete(_) => true,
                        };

                        if is_file_event {
                            let _guard = SyncGuard(is_syncing_inner.clone());
                            is_syncing_inner.store(true, Ordering::SeqCst);
                            {
                                let mut msg = status_msg_inner.lock().unwrap();
                                *msg = "Syncing local change...".to_string();
                            }

                            if let Err(e) = coord.handle_fs_change(change).await {
                                eprintln!("[xsync-gui watcher] handle_fs_change error: {:#}", e);
                            }

                            *STATS_CACHE.lock().unwrap() = None;

                            {
                                let mut msg = status_msg_inner.lock().unwrap();
                                *msg = "All files up to date".to_string();
                            }
                        } else {
                            let _ = coord.handle_fs_change(change).await;
                        }
                    }
                });
            }

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                let _ = &debouncers;
            }
        });
    }

    tauri::Builder::default()
        .manage(context)
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            get_recent_activity,
            get_all_files,
            get_category_stats,
            open_folder,
            sync_all,
            toggle_pause,
            browse_folder,
            add_directory,
            remove_directory,
            run_diff,
            run_merge,
            systemd_action,
            login,
            logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
