<div align="center">
  <img src="assets/logo.svg" alt="xsync Logo" width="120"/>
  <h1>xsync</h1>
  <p><b>Linux-Native Google Drive client with native <code>.gitignore</code> integration.</b></p>
  <p>
    <a href="https://github.com/sadabx/xsync/releases"><img src="https://img.shields.io/badge/Release-v0.2.0-blue?style=flat-square" alt="Release"></a>
    <img src="https://img.shields.io/badge/Rust-1.75%2B-orange?style=flat-square&logo=rust&logoColor=white" alt="Rust">
    <img src="https://img.shields.io/badge/Platform-Linux-green?style=flat-square&logo=linux&logoColor=white" alt="Linux">
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0-blue.svg?style=flat-square" alt="License"></a>
  </p>
</div>

`xsync` provides a fast, native Google Drive synchronization experience on Linux. Built with Rust, it synchronizes local directories with Google Drive in real time using kernel `inotify` events while automatically respecting `.gitignore` and `.xsyncignore` rules to keep build targets, dependencies, and temporary files from syncing to the cloud.

---

## Features

- **Interactive Terminal Dashboard**: Run `xsync` with no arguments to launch an interactive dashboard with connection status, tracked directories, and guided actions.
- **Cross-Platform Desktop GUI**: Modern desktop app with system tray controls, live sync indicators, storage quota progress, diff visualization, and folder management.
- **Git & Ignore Integration**: Automatically honors `.gitignore` rules in current and nested directories, with support for custom `.xsyncignore` patterns.
- **Real-Time Synchronization**: Watches local filesystem events using Linux `inotify` with intelligent debounce buffering before uploading.
- **Two-Way Reconciliation & Conflict Handling**: Fast two-way sync with automatic conflict copy preservation (`.conflict-<timestamp>`) and bulk deletion safeguards.
- **Resumable Transfers**: Chunked resumable uploads and downloads with bounded concurrency, real-time progress bars, and automatic exponential backoff.
- **Google Workspace Support**: Automatically exports Google Docs, Sheets, and Slides into standard office formats (`.docx`, `.xlsx`, `.pptx`).
- **Cloud Tools**: Server-side recursive folder diffing (`xsync diff`) and zero-download folder merging (`xsync merge`).
- **Background Daemon**: Integrated systemd user service management with one-command installation.

---

## Installation

**One-line install (Linux x86_64 / aarch64)**:

```bash
curl -fsSL https://raw.githubusercontent.com/sadabx/xsync/main/install.sh | bash
```

<details>
<summary><b>Alternative: Build from source via Cargo</b></summary>
<br/>

```bash
# Direct install from Git
cargo install --git https://github.com/sadabx/xsync.git xsync

# Or clone and compile locally
git clone https://github.com/sadabx/xsync.git
cd xsync
cargo install --path crates/xsync-cli
```
</details>

---

## Quick Start

### 1. Authenticate
```bash
xsync auth
```
*Opens your default browser to complete OAuth2 PKCE login. Works out of the box with zero Google Cloud configuration required.*

### 2. Link a Folder
```bash
# Link your workspace to a remote Google Drive folder
xsync init ~/Projects -d "Projects_Backup"

# (Optional) Preview what files will be synced
xsync scan
```

### 3. Sync or Watch
```bash
# Run a one-time two-way synchronization pass
xsync sync

# Or start the real-time background watcher
xsync watch
```

> **Tip**: Run `xsync` with no arguments anytime to access the interactive menu.

---

## Desktop GUI

`xsync` includes a fast, 100% native Rust desktop GUI powered by [Slint](https://slint.dev) (no WebKit, HTML, or browser runtime required):

```bash
# Launch Native GUI
cargo run --package xsync-gui
```

Features include a real-time storage quota bar, native file picker (`rfd`), directory management, live activity feeds, visual folder diffing, and one-click cloud folder merges.

---

## Commands

| Command | Usage | Description |
| --- | --- | --- |
| *(default)* | `xsync` | Launch interactive terminal dashboard |
| `auth` | `xsync auth [--client-id <ID> --client-secret <SECRET>]` | Authenticate with Google Drive via OAuth2 PKCE |
| `init` | `xsync init <path> -d <remote_folder_or_id>` | Link a local directory to Google Drive |
| `scan` | `xsync scan [path]` | Preview files that will sync, respecting `.gitignore` |
| `sync` | `xsync sync [path] [--dry-run] [--concurrency <N>] [--force]` | Run two-way reconciliation with progress indicators |
| `watch` | `xsync watch [path] [--notify] [--force]` | Start real-time background inotify daemon |
| `status` | `xsync status` | Display tracked paths, sync queue, and database status |
| `diff` | `xsync diff <folder1> <folder2>` | Compare two remote Drive folders via MD5 checksums |
| `merge` | `xsync merge <source_folder> <target_folder>` | Server-side merge unique files without downloading |
| `service` | `xsync service <install\|status\|start\|stop\|logs>` | Manage the systemd background user service |
| `completions` | `xsync completions <bash\|zsh\|fish>` | Generate shell auto-completions |

---

## Run as a Background Service

Run `xsync` continuously in the background using the integrated systemd manager:

```bash
# Install, enable, and start user service
xsync service install

# Check status
xsync service status

# View live daemon logs
xsync service logs
```

---

## Shell Completions

Generate shell completions:

```bash
# Bash
xsync completions bash > ~/.local/share/bash-completion/completions/xsync

# Zsh
xsync completions zsh > ~/.zfunc/_xsync

# Fish
xsync completions fish > ~/.config/fish/completions/xsync.fish
```

---

## License

GPL-3.0 License. See [LICENSE](LICENSE) for details.