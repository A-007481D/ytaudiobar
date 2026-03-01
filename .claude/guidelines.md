# Claude Code Guidelines for YTAudioBar

## Project Overview

**YTAudioBar** is a cross-platform desktop app (macOS/Windows/Linux) for streaming, downloading, and organizing YouTube audio.

- **Frontend:** React + TypeScript + TailwindCSS (`/src`)
- **Backend:** Rust + Tauri v2 (`/src-tauri/src`)
- **Current Version:** 2.1.5
- **Platforms:** macOS (Swift), Windows & Linux (Rust + Tauri)

## Code Structure

### Frontend (`/src`)

- `src/app/routes/home.tsx` — main player UI
- `src/lib/tauri.ts` — Tauri command bindings
- `src/components/` — reusable UI components

### Backend (`/src-tauri/src`)

- `main.rs` — window setup, event handlers, command registration
- `database.rs` — SQLite management + window geometry persistence
- `ytdlp_manager.rs` — YouTube content fetching
- `audio_manager.rs` — playback control
- `queue_manager.rs` — playlist logic
- `download_manager.rs` — offline downloads
- `media_key_manager.rs` — system media keys

## Development Workflow

### Before Coding

1. Check `git status` for uncommitted changes
2. Read relevant code before modifying
3. Ask user if unclear about intent

### Committing

- **Style:** Imperative, lowercase, no emoji
- **Example:** "Fix Linux window positioning on startup"
- **Co-author:** Always include `Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>`

### Version Bumping

Update all three files together:

- `package.json` (version field)
- `src-tauri/Cargo.toml` (version field)
- `src-tauri/tauri.conf.json` (version field)

### Releasing

- **Pre-release (testing):** `git tag v2.1.5-beta` (with dash)
- **Stable (users get it):** `git tag v2.1.5` (no dash)
- **Workflow auto-detects:** `-` in tag = pre-release, no `latest.json`

## Platform-Specific Notes

### Linux ⚠️

- `set_position()` on hidden windows: ignored (must show first)
- Window raising: requires `hide()` → `set_always_on_top(true/false)` → `set_focus()`
- Position restoration: save before hide (WM forgets it)

### Windows

- Window positioning: works on hidden windows
- Close: hides instead of closing (tray behavior)

### macOS

- Uses menu bar (not tray)
- Native code signing required

## Key Patterns

### Add Tauri Command

```rust
#[tauri::command]
async fn my_command(param: String, state: State<'_, AppState>) -> Result<String, String> {
    // implementation
}
// Register in invoke_handler
```

### Save Data Between Sessions

```rust
// Save on close
WindowEvent::CloseRequested => {
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let _ = db.save_window_geometry(x, y, w, h).await;
    });
}
```

### Restore on Startup

Use `tokio::task::block_in_place()` (not `block_on()`) inside Tauri's runtime.

## Database

- **Schema:** `app_settings` table with window geometry columns
- **Migrations:** Use `ALTER TABLE` (backwards compatible)
- **No:** Dropping/recreating tables

## Don'ts

- ❌ Remove working workarounds without understanding why
- ❌ Change plugins (e.g., removed `tauri-plugin-window-state` to fix positioning)
- ❌ Commit version bumps separately from features
- ❌ Use `--no-verify` on git hooks
- ❌ Over-engineer simple features
