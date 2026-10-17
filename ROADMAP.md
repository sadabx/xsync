# xsync Development Roadmap & Feature Parity Plan

This roadmap tracks missing features from **Google Drive for Desktop (Windows/macOS)** to bring complete, best-in-class Google Drive functionality to Linux while preserving `xsync`'s core principles: **lightning speed, native Rust architecture, developer-first `.gitignore` awareness, and zero telemetry bloat.**

---

## Roadmap Overview

```
Phase 1: Daemon Controls & Usability
├── ⏸ Real-Time Pause & Resume (Unix Domain Socket IPC)
├── ⚡ Bandwidth Throttling / Rate Limiting (Token-Bucket)
└── 👤 Multi-Account Profiles (`--profile work|personal`)

Phase 2: Google Workspace & Cloud Scope
├── 🏢 Google Workspace Shared Drives (Team Drives) Support
├── 🤝 "Shared with Me" Folder Ingestion & Syncing
└── 📁 Selective Subdirectory Exclusion Tree

Phase 3: Cross-Platform Desktop GUI (Linux, Windows, macOS)
├── 🖥 Modern Native Desktop App via Slint (100% Rust, Zero-WebKit)
├── 🍏 macOS Menu Bar & Windows System Tray Applet
├── 🎨 Dark-Themed Visual Dashboard (Folders, Live Transfers, Quotas)
└── 🔍 Visual Folder Diff & Interactive Server-Side Merge Tool

Phase 4: OS Shell & File Manager Integration
├── 🪟 Windows File Explorer Context Menu & Sync Badges
├── 🍏 macOS Finder Sync Extension
└── 🐧 Linux File Manager Extensions (Nautilus, Nemo, Dolphin)

Phase 5: Advanced Filesystem Architecture
└── 💽 Virtual Filesystem / Streaming Mode (`xsync mount` via pure Rust `fuse3` / WinFsp)
```

---

## Phase 1: Daemon Controls & Usability

### 1. Real-Time Pause & Resume (`xsync pause` / `xsync resume`)
* **Goal**: Allow users to temporarily pause sync activity without killing the background daemon or stopping the systemd service.
* **Architecture**:
  * Implement an IPC control listener using a **Unix Domain Socket** located at `~/.config/xsync/daemon.sock`.
  * The watcher daemon runs with an `Arc<AtomicBool>` pause state.
  * When paused:
    - Kernel inotify filesystem events continue buffering in an in-memory queue.
    - Active network uploads and reconciliation passes are frozen.
  * When resumed:
    - The pending event queue is drained, and a rapid delta-check is performed.
* **CLI Surface**:
  * `xsync pause` - Pauses active syncing.
  * `xsync resume` - Resumes syncing and flushes pending changes.
  * `xsync status` - Displays `● Active (Syncing)` vs `⏸ Paused (Queue: N files)`.

---

### 2. Bandwidth Throttling / Rate Limiter
* **Goal**: Prevent background synchronization from saturating home Wi-Fi or metered connections.
* **Architecture**:
  * Implement a **Token Bucket** rate limiter in `DriveClient` for upload and download byte streams.
  * Wrap chunked upload and download byte bodies with a metered stream adapter that clamps transfer rates to configured byte-per-second thresholds.
* **CLI Surface**:
  * `xsync sync --max-upload-speed 5M --max-download-speed 10M`
  * `xsync watch --max-upload-speed 2M`
  * Configurable globally in `~/.config/xsync/config.toml` under `[network]`.

---

### 3. Multi-Account Support (Profiles)
* **Goal**: Enable simultaneous or switchable syncing for multiple Google accounts (e.g., personal `@gmail.com` and enterprise Google Workspace `@company.com`).
* **Architecture**:
  * Introduce profile namespaces:
    - Tokens: `~/.config/xsync/profiles/<profile_name>/token.json`
    - Databases: `~/.config/xsync/profiles/<profile_name>/state.db`
    - Configurations: `~/.config/xsync/profiles/<profile_name>/config.toml`
  * Default profile remains `default` for backward compatibility.
* **CLI Surface**:
  * `xsync auth --profile work`
  * `xsync init ~/Work -d "WorkDrive" --profile work`
  * `xsync watch --profile work`

---

## Phase 2: Google Workspace & Cloud Scope

### 4. Shared Drives (Team Drives) & "Shared with Me"
* **Goal**: Allow users to link and sync folders from corporate Google Workspace Shared Drives and shared documents.
* **Architecture**:
  * Enable `supportsAllDrives=true` and `includeItemsFromAllDrives=true` across all Google Drive v3 REST API endpoints.
  * Add Drive listing method `list_shared_drives()` to discover and map available Shared Drives.
* **CLI Surface**:
  * `xsync list-drives` - Lists personal Drive and all accessible Shared Drives.
  * `xsync init <local_path> -d <folder_id> --shared-drive <drive_id>`

---

### 5. Selective Subtree Sync
* **Goal**: Choose specific remote subfolders to omit from local synchronization when full directory cloning is undesirable.
* **Architecture**:
  * Add a `[selective_sync]` section in directory config mapping paths to sync or skip.
  * Filter during `pull_reconcile` before recursive traversal.
* **CLI Surface**:
  * `xsync init <path> --exclude "Archive/*" --exclude "LargeMedia/*"`

---

## Phase 3: Cross-Platform Desktop GUI (Linux, Windows, macOS)

### 6. Modern Lightweight Desktop Application (`xsync-gui`)
* **Goal**: Provide a beautiful, responsive, non-bloated desktop GUI across **Linux, Windows, and macOS** that replaces the need for the official 1.5 GB Google Drive desktop client.
* **Why Slint (100% Native Rust, Zero-WebKit)**:
  * **Memory Footprint**: Uses ~15–25 MB of RAM (versus Electron’s 600 MB – 1.5 GB and WebKit's 180+ MB).
  * **Pure Native Graphics**: Renders via FemtoVG / Skia hardware acceleration. Zero Chromium, WebKitGTK, or HTML/JS dependencies.
  * **Direct Rust Integration**: The Slint UI connects directly to `xsync-core` structs and async Tokio background tasks—zero IPC or JSON serialization overhead.
  * **Wayland & Linux Native**: Flawless native Wayland (KDE, GNOME) and X11 performance without WebKitGTK DMA-BUF protocol errors.
  * **Native Packaging**: Builds `.msi`/`.exe` for Windows, `.dmg` for macOS (Universal binary: Apple Silicon M1-M4 + Intel), and `.deb`/`.AppImage` for Linux.
* **Core GUI Features**:
  1. **System Tray / Menu Bar Applet**:
     - Dynamic icon indicating state: idle, syncing spinner, paused, or conflict alert.
     - Quick flyout menu: Current upload/download speeds, live transfer queue, 1-click pause toggle (2h / 4h / until resumed), and "Open Google Drive in Browser".
  2. **Main Dashboard**:
     - Directory Manager: Drag-and-drop local folders to link them to Google Drive folders.
     - Storage Quota Bar: Visual breakdown of Google Drive storage (Used vs Available).
     - Visual `.xsyncignore` Editor: Toggle common ignore presets (`node_modules`, build targets, virtual environments, raw videos) with one click.
     - Live Transfer Monitor: Visual progress meters, ETA calculations, and transfer logs.
  3. **Visual Folder Diff & Server-Side Merge Tool**:
     - Visual dual-pane comparison between two Google Drive folders.
     - Color-coded badges for unique, modified, and identical files.
     - 1-click "Merge Unique Files" button executing server-side copy operations.

---

## Phase 4: OS Shell & File Manager Integration

### 7. Native OS Shell Badges & Context Menus
* **Goal**: Provide native file manager overlays and right-click actions matching Windows File Explorer, macOS Finder, and Linux file managers.
* **Platform Architecture**:
  * **Windows**:
    - Windows Shell Extension (C++ / Rust `windows-rs` COM DLL) or Windows Cloud Files API.
    - Context menu: "Copy Google Drive Link", "Pause Sync for this folder", "Force Sync Now".
    - Icon overlays in File Explorer for sync status (green checkmark, blue arrows, conflict symbol).
  * **macOS**:
    - Finder Sync Extension (`FIFinderSyncController`) in Swift/Objective-C communicating with `xsync-core`.
    - Finder sidebar item and badge emblems on synced folders.
  * **Linux**:
    - Extensions for GNOME Files (`nautilus-python`), Nemo, and KDE Dolphin.

---

## Phase 5: Virtual Filesystem / Streaming Mode (`xsync mount`)

### 8. Virtual Filesystem / Files On-Demand via Pure Rust FUSE
* **Goal**: Allow users to mount their entire Google Drive library as a virtual filesystem (e.g. `/mnt/gdrive` on Linux, `G:\` on Windows, `/Volumes/GoogleDrive` on macOS), streaming files on-demand without consuming local SSD storage.
* **Architecture Specification**:
  * **Pure Rust & Zero Garbage Collection**:
    - Linux/macOS: Use the async [`fuse3`](https://crates.io/crates/fuse3) crate or `fuser` running directly on Tokio.
    - Windows: WinFsp (Windows File System Proxy) driver integration via Rust bindings.
    - Zero Go dependency, zero GC stutter during 4K media playback.
  * **Chunked Read-Ahead Streaming**:
    - When an application executes `read(offset, size)`, issue HTTP `Range: bytes=X-Y` queries directly to Google Drive.
    - Read-ahead window: dynamically buffer the next 16 MiB chunk for sequential reads.
  * **Sparse Local LRU Disk Cache**:
    - Store recently read blocks in user cache directory (`~/.cache/xsync/vfs/<file_id>/` or `%LOCALAPPDATA%/xsync/vfs/`).
    - Automatically evict least-recently-used chunks when disk cache exceeds a user-configured limit (e.g., 10 GB).
  * **Write-Back Journal**:
    - Support staging modified blocks to local disk with an asynchronous write-back worker that handles resumable uploads upon file `flush()` or `release()`.
* **CLI Surface**:
  * `xsync mount <remote_folder_or_id> <mountpoint> [options]`
    - `--vfs-cache-mode full|writes|off`
    - `--vfs-cache-max-size 10G`
    - `--vfs-read-chunk-size 16M`
    - `--allow-other`
  * `xsync unmount <mountpoint>`

---

## Principles Maintained Throughout Roadmap

1. **Pure Native Rust**: Zero polyglot runtime overhead, single self-contained binary distribution.
2. **Developer-First**: Native `.gitignore` and `.xsyncignore` rules remain first-class citizens across all features.
3. **Zero Data Loss**: Conflict preservation, deletion guards, and atomic SQLite state transactions are strictly enforced.
4. **Clean Unix Philosophy**: Modular commands, scriptable JSON output options, and headless server compatibility.
