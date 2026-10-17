slint::include_modules!();

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use slint::{ComponentHandle, ModelRc, VecModel};
use tokio::runtime::Runtime;
use xsync_core::auth::{load_stored_token, token_path};
use xsync_core::config::{add_or_update_directory, load_config, save_config};
use xsync_core::db::Database;
use xsync_core::drive::{DriveClient, DriveFile};
use xsync_core::sync::{SyncCoordinator, SyncOptions};

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
    use chrono::{DateTime, Local, Utc};
    if let Some(utc) = DateTime::<Utc>::from_timestamp(ts, 0) {
        let local: DateTime<Local> = DateTime::from(utc);
        local.format("%b %d, %H:%M").to_string()
    } else {
        "Just now".to_string()
    }
}

pub struct AppState {
    pub is_paused: Arc<AtomicBool>,
    pub rt: Arc<Runtime>,
}

pub fn update_ui_from_background(window_weak: slint::Weak<MainWindow>, is_paused: bool) {
    std::thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut connected = false;
            let mut user_name = "Not Connected".to_string();
            let mut user_email = "Sign in via CLI or config".to_string();
            let mut storage_used_str = "0 GB".to_string();
            let mut storage_total_str = "15 GB".to_string();
            let mut storage_ratio = 0.0f32;

            if let Ok(Some(_)) = load_stored_token() {
                if let Ok(drive) = DriveClient::from_auth().await {
                    if let Ok(about) = drive.get_about().await {
                        connected = true;
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
                }
            }

            let mut dir_items: Vec<WatchedDir> = Vec::new();
            if let Ok(cfg) = load_config() {
                for d in cfg.directories {
                    dir_items.push(WatchedDir {
                        local_path: d.local_path.to_string_lossy().to_string().into(),
                        drive_folder_id: d.drive_folder_id.into(),
                        drive_folder_name: d.drive_folder_name.unwrap_or_else(|| "Google Drive".into()).into(),
                    });
                }
            }

            let mut total_files_tracked = 0i32;
            let mut total_bytes_str = "0 MB".to_string();
            let mut recent_items: Vec<RecentFile> = Vec::new();

            if let Ok(db) = Database::open_default() {
                if let Ok(stats) = db.get_db_stats() {
                    total_files_tracked = stats.total_files as i32;
                    total_bytes_str = format_bytes(stats.total_size_bytes);
                }
                if let Ok(recent) = db.get_recent_files(20) {
                    for f in recent {
                        recent_items.push(RecentFile {
                            rel_path: f.rel_path.to_string_lossy().to_string().into(),
                            size_str: format_bytes(f.size_bytes).into(),
                            time_str: format_timestamp(f.last_sync_timestamp).into(),
                        });
                    }
                }
            }

            let total_dirs = dir_items.len() as i32;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = window_weak.upgrade() {
                    w.set_is_connected(connected);
                    w.set_user_name(user_name.into());
                    w.set_user_email(user_email.into());
                    w.set_storage_used_str(storage_used_str.into());
                    w.set_storage_total_str(storage_total_str.into());
                    w.set_storage_ratio(storage_ratio);
                    w.set_total_directories(total_dirs);
                    w.set_total_files_tracked(total_files_tracked);
                    w.set_total_bytes_str(total_bytes_str.into());
                    w.set_is_paused(is_paused);
                    w.set_directories(ModelRc::new(VecModel::from(dir_items)));
                    w.set_recent_files(ModelRc::new(VecModel::from(recent_items)));
                }
            });
        });
    });
}

pub fn run() {
    #[cfg(target_os = "linux")]
    let _ = i_slint_core::api::set_xdg_app_id("xsync-gui");

    let main_window = MainWindow::new().expect("Failed to initialize Slint MainWindow");
    let is_paused = Arc::new(AtomicBool::new(false));
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio Runtime"));

    // Initial state populate
    update_ui_from_background(main_window.as_weak(), is_paused.load(Ordering::SeqCst));

    // Callback: Sync All Now
    {
        let window_weak = main_window.as_weak();
        let is_paused_clone = is_paused.clone();
        let rt_clone = rt.clone();
        main_window.on_sync_now(move || {
            if is_paused_clone.load(Ordering::SeqCst) {
                if let Some(w) = window_weak.upgrade() {
                    w.set_status_message("Sync is currently paused.".into());
                }
                return;
            }

            let window_weak_inner = window_weak.clone();
            let is_paused_val = is_paused_clone.load(Ordering::SeqCst);
            if let Some(w) = window_weak.upgrade() {
                w.set_is_syncing(true);
                w.set_status_message("Synchronizing all Google Drive directories...".into());
            }

            rt_clone.spawn(async move {
                let config = load_config().unwrap_or_default();
                if let Ok(drive) = DriveClient::from_auth().await {
                    if let Ok(db) = Database::open_default() {
                        let opts = SyncOptions {
                            dry_run: false,
                            permanent_delete: false,
                            concurrency: 4,
                            show_progress: false,
                            force: false,
                        };

                        for dir in config.directories {
                            if !dir.local_path.exists() { continue; }
                            if let Ok(coord) = SyncCoordinator::new(&dir.local_path, dir.drive_folder_id, drive.clone(), db.clone()) {
                                let _ = coord.reconcile_with_options(&opts).await;
                            }
                        }
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = window_weak_inner.upgrade() {
                        w.set_is_syncing(false);
                        w.set_status_message("All Google Drive directories are in sync.".into());
                    }
                    update_ui_from_background(window_weak_inner, is_paused_val);
                });
            });
        });
    }

    // Callback: Toggle Pause
    {
        let window_weak = main_window.as_weak();
        let is_paused_clone = is_paused.clone();
        main_window.on_toggle_pause(move || {
            let current = is_paused_clone.load(Ordering::SeqCst);
            let next = !current;
            is_paused_clone.store(next, Ordering::SeqCst);

            if let Some(w) = window_weak.upgrade() {
                w.set_is_paused(next);
                if next {
                    w.set_status_message("Synchronization paused.".into());
                } else {
                    w.set_status_message("Synchronization resumed.".into());
                }
            }
        });
    }

    // Callback: Browse Local Folder using rfd (native folder picker)
    {
        let window_weak = main_window.as_weak();
        main_window.on_browse_local_folder(move || {
            let window_weak_clone = window_weak.clone();
            std::thread::spawn(move || {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let folder_str = folder.to_string_lossy().to_string();
                    let folder_name = folder
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "DriveSync".to_string());

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = window_weak_clone.upgrade() {
                            w.set_new_folder_local(folder_str.into());
                            if w.get_new_folder_remote().is_empty() {
                                w.set_new_folder_remote(folder_name.into());
                            }
                        }
                    });
                }
            });
        });
    }

    // Callback: Add Directory
    {
        let window_weak = main_window.as_weak();
        let is_paused_clone = is_paused.clone();
        let rt_clone = rt.clone();
        main_window.on_add_directory(move |local, remote| {
            let local_str = local.to_string();
            let remote_str = if remote.is_empty() { "root".to_string() } else { remote.to_string() };
            let window_weak_inner = window_weak.clone();
            let is_paused_val = is_paused_clone.load(Ordering::SeqCst);

            rt_clone.spawn(async move {
                let p = PathBuf::from(&local_str);
                let _ = std::fs::create_dir_all(&p);
                if let Ok(canonical) = std::fs::canonicalize(&p) {
                    if let Ok(drive) = DriveClient::from_auth().await {
                        let drive_folder_id = if remote_str == "root" {
                            "root".to_string()
                        } else if let Ok(meta) = drive.get_file_metadata(&remote_str).await {
                            if meta.is_folder() { meta.id } else { "root".to_string() }
                        } else {
                            match drive.find_child_by_name("root", &remote_str).await {
                                Ok(Some(f)) if f.is_folder() => f.id,
                                _ => {
                                    drive.create_folder("root", &remote_str)
                                        .await
                                        .map(|f| f.id)
                                        .unwrap_or_else(|_| "root".to_string())
                                }
                            }
                        };

                        let _ = add_or_update_directory(&canonical, &drive_folder_id, Some(remote_str.clone()));
                        if let Ok(db) = Database::open_default() {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i64;
                            let _ = db.upsert_root(&xsync_core::db::SyncRootRecord {
                                local_root: canonical,
                                drive_root_id: drive_folder_id,
                                drive_root_name: Some(remote_str),
                                created_at: now,
                                last_sync_at: now,
                            });
                        }
                    }
                }

                update_ui_from_background(window_weak_inner, is_paused_val);
            });
        });
    }

    // Callback: Remove Directory
    {
        let window_weak = main_window.as_weak();
        let is_paused_clone = is_paused.clone();
        main_window.on_remove_directory(move |path| {
            let p = PathBuf::from(path.as_str());
            let canonical = std::fs::canonicalize(&p).unwrap_or(p);
            if let Ok(mut config) = load_config() {
                config.directories.retain(|d| d.local_path != canonical);
                let _ = save_config(&config);
            }
            update_ui_from_background(window_weak.clone(), is_paused_clone.load(Ordering::SeqCst));
        });
    }

    // Callback: Run Diff
    {
        let window_weak = main_window.as_weak();
        let rt_clone = rt.clone();
        main_window.on_run_diff(move |folder1, folder2| {
            let f1 = folder1.to_string();
            let f2 = folder2.to_string();
            let window_weak_inner = window_weak.clone();

            if let Some(w) = window_weak.upgrade() {
                w.set_diff_summary("Comparing folders on Google Drive...".into());
            }

            rt_clone.spawn(async move {
                if let Ok(drive) = DriveClient::from_auth().await {
                    let files1 = drive.list_files_recursive(&f1).await.unwrap_or_default();
                    let files2 = drive.list_files_recursive(&f2).await.unwrap_or_default();

                    let map1: HashMap<PathBuf, &DriveFile> = files1.iter().map(|(p, f)| (p.clone(), f)).collect();
                    let map2: HashMap<PathBuf, &DriveFile> = files2.iter().map(|(p, f)| (p.clone(), f)).collect();

                    let mut items: Vec<DiffItem> = Vec::new();
                    let mut identical = 0;
                    let mut modified = 0;

                    for (path, f1_meta) in &map1 {
                        if let Some(f2_meta) = map2.get(path) {
                            let md5_1 = f1_meta.md5_checksum.as_deref().unwrap_or("");
                            let md5_2 = f2_meta.md5_checksum.as_deref().unwrap_or("");
                            if !md5_1.is_empty() && md5_1 == md5_2 {
                                identical += 1;
                            } else {
                                modified += 1;
                                items.push(DiffItem {
                                    path: path.to_string_lossy().to_string().into(),
                                    side: "modified".into(),
                                });
                            }
                        } else {
                            items.push(DiffItem {
                                path: path.to_string_lossy().to_string().into(),
                                side: "unique_to_source".into(),
                            });
                        }
                    }

                    for (path, _) in &map2 {
                        if !map1.contains_key(path) {
                            items.push(DiffItem {
                                path: path.to_string_lossy().to_string().into(),
                                side: "unique_to_destination".into(),
                            });
                        }
                    }

                    let summary = format!(
                        "Diff completed: {} identical, {} modified, {} source-only, {} dest-only",
                        identical, modified,
                        items.iter().filter(|i| i.side == "unique_to_source").count(),
                        items.iter().filter(|i| i.side == "unique_to_destination").count()
                    );

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = window_weak_inner.upgrade() {
                            w.set_diff_summary(summary.into());
                            w.set_diff_items(ModelRc::new(VecModel::from(items)));
                        }
                    });
                }
            });
        });
    }

    // Callback: Run Merge
    {
        let window_weak = main_window.as_weak();
        let rt_clone = rt.clone();
        main_window.on_run_merge(move |src, dest| {
            let s = src.to_string();
            let d = dest.to_string();
            let window_weak_inner = window_weak.clone();

            if let Some(w) = window_weak.upgrade() {
                w.set_diff_summary("Merging missing files into destination...".into());
            }

            rt_clone.spawn(async move {
                if let Ok(drive) = DriveClient::from_auth().await {
                    let files1 = drive.list_files_recursive(&s).await.unwrap_or_default();
                    let files2 = drive.list_files_recursive(&d).await.unwrap_or_default();
                    let map2: HashMap<PathBuf, &DriveFile> = files2.iter().map(|(p, f)| (p.clone(), f)).collect();

                    let mut copied = 0;
                    for (rel_path, file) in &files1 {
                        if !map2.contains_key(rel_path) {
                            let parent_rel = rel_path.parent().unwrap_or_else(|| Path::new(""));
                            if let Ok(dest_parent_id) = drive.ensure_remote_dir_path(&d, parent_rel).await {
                                if drive.copy_file(&file.id, &dest_parent_id, &file.name).await.is_ok() {
                                    copied += 1;
                                }
                            }
                        }
                    }

                    let msg = format!("Merge complete: {} missing files copied.", copied);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = window_weak_inner.upgrade() {
                            w.set_diff_summary(msg.into());
                        }
                    });
                }
            });
        });
    }

    // Callback: Systemd Action
    {
        main_window.on_systemd_action(move |action| {
            let act = action.to_string();
            std::thread::spawn(move || {
                match act.as_str() {
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
                            let _ = std::process::Command::new("systemctl").args(["--user", "daemon-reload"]).status();
                            let _ = std::process::Command::new("systemctl").args(["--user", "enable", "--now", "xsync.service"]).status();
                        }
                    }
                    "restart" => {
                        let _ = std::process::Command::new("systemctl").args(["--user", "restart", "xsync.service"]).status();
                    }
                    "start" => {
                        let _ = std::process::Command::new("systemctl").args(["--user", "start", "xsync.service"]).status();
                    }
                    "stop" => {
                        let _ = std::process::Command::new("systemctl").args(["--user", "stop", "xsync.service"]).status();
                    }
                    _ => {}
                }
            });
        });
    }

    // Callback: Refresh State
    {
        let window_weak = main_window.as_weak();
        let is_paused_clone = is_paused.clone();
        main_window.on_refresh_state(move || {
            update_ui_from_background(window_weak.clone(), is_paused_clone.load(Ordering::SeqCst));
        });
    }

    // Callback: Logout
    {
        let window_weak = main_window.as_weak();
        let is_paused_clone = is_paused.clone();
        main_window.on_logout(move || {
            if let Ok(p) = token_path() {
                let _ = std::fs::remove_file(p);
            }
            update_ui_from_background(window_weak.clone(), is_paused_clone.load(Ordering::SeqCst));
        });
    }

    // Callback: Login
    {
        let window_weak = main_window.as_weak();
        let is_paused_clone = is_paused.clone();
        let rt_clone = rt.clone();
        main_window.on_login(move || {
            let window_weak_inner = window_weak.clone();
            let is_paused_val = is_paused_clone.load(Ordering::SeqCst);
            if let Some(w) = window_weak.upgrade() {
                w.set_status_message("Opening Google authentication in browser...".into());
            }

            rt_clone.spawn(async move {
                use xsync_core::auth::{execute_oauth_login, save_stored_token, DEFAULT_CLIENT_ID, DEFAULT_CLIENT_SECRET};
                let cfg = load_config().unwrap_or_default();
                let (cid, csec) = if let Some(ref id) = cfg.client_id {
                    (id.clone(), cfg.client_secret.clone())
                } else {
                    (DEFAULT_CLIENT_ID.to_string(), DEFAULT_CLIENT_SECRET.map(|s| s.to_string()))
                };

                if let Ok(token) = execute_oauth_login(&cid, csec).await {
                    let _ = save_stored_token(&token);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = window_weak_inner.upgrade() {
                            w.set_status_message("Authentication successful!".into());
                        }
                        update_ui_from_background(window_weak_inner, is_paused_val);
                    });
                }
            });
        });
    }

    // Run the native event loop
    main_window.run().expect("Error running Slint native event loop");
}
