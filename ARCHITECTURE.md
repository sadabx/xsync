# Architecture Specification: xsync

## 1. Workspace Layout
A modular Cargo workspace providing both terminal and desktop interfaces:

```text
xsync/
├── Cargo.toml               # Workspace manifest
├── ARCHITECTURE.md          # System specification
└── crates/
    ├── xsync-core/          # Core sync library
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── auth/        # OAuth2 token exchange & ~/.config/xsync/token.json
    │       ├── drive/       # Google Drive v3 REST client (MD5 checksum, resumable transfers)
    │       ├── db/          # SQLite state database (local path <-> remote ID mapping)
    │       ├── filter/      # Gitignore engine (.gitignore & .xsyncignore matcher)
    │       ├── watcher/     # Inotify watcher via `notify-debouncer-full`
    │       └── sync/        # Two-way sync coordinator & conflict resolution
    ├── xsync-cli/           # Terminal interface
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs      # CLI commands (auth, init, scan, sync, watch, diff, merge)
    │       └── interactive.rs # TUI interactive dashboard
    └── xsync-gui/           # Native Cross-platform Desktop GUI (Slint)
        ├── Cargo.toml
        ├── build.rs         # Compiles ui/app.slint to native Rust code
        ├── src/
        └── ui/              # Reactive Slint UI & assets (app.slint, logo.png)
```

## 2. CLI Subcommands

* `xsync` *(no args)*: Interactive terminal dashboard.
* `xsync auth`: Run OAuth2 PKCE login in browser and save token.
* `xsync init <path>`: Link a local directory to a remote Google Drive folder.
* `xsync scan`: Traverse target folder using `ignore` and print all unignored files that would sync.
* `xsync sync`: Run a two-way push/pull reconciliation pass.
* `xsync watch`: Start the real-time background inotify daemon.
* `xsync status`: Inspect sync queue and database health.
* `xsync diff`: Recursively compare two remote folders via MD5 checksums.
* `xsync merge`: Server-side merge unique files without downloading.
* `xsync service`: Manage systemd user service (`install`, `restart`, `logs`).

## 3. Storage & Config

All configuration and cache files reside in:
`~/.config/xsync/`

* `config.toml`: Watched directories and remote root folder IDs.
* `state.db`: SQLite database for paths, hashes, and Drive IDs.
* `token.json`: Stored OAuth tokens.
