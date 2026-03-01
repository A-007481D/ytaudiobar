# Aider Configuration for YTAudioBar

## Project Info

- **Name:** YTAudioBar
- **Type:** Cross-platform desktop app (Tauri)
- **Frontend:** React + TypeScript + TailwindCSS
- **Backend:** Rust + Tauri v2
- **Repository:** https://github.com/ilyassan/ytaudiobar

## Key Directories

```
src/                    React frontend
src-tauri/src/          Rust backend
.github/workflows/      CI/CD pipelines
```

## Important Files to Watch

- `package.json` — frontend version
- `src-tauri/Cargo.toml` — Rust version
- `src-tauri/tauri.conf.json` — Tauri config + version
- `src-tauri/src/main.rs` — window setup, commands
- `src-tauri/src/database.rs` — data persistence
- `src/lib/tauri.ts` — frontend-backend bindings

## Development Rules

### Code Changes

- Read before modifying (understand existing patterns)
- Preserve working workarounds if documented
- Test on multiple platforms if possible

### Commits

- **Style:** Imperative lowercase
- **Example:** "Add window persistence feature"
- **Frequency:** One feature per commit
- **Co-author:** Use git hooks (automatic via husky)

### Versions

- Update all three version files together
- Use semantic versioning (MAJOR.MINOR.PATCH)
- Pre-release: append `-beta`, `-rc1`, etc.

### Releases

- Tags with `-` are pre-releases (no auto-update for users)
- Tags without `-` are stable (users auto-update)
- Workflow auto-detects and marks accordingly

## Platform-Specific Behavior

### Linux

⚠️ **Important gotchas:**

1. Window position set while hidden is ignored by WM
2. Must call `show()` before `set_position()`
3. `hide()` + `show()` cycle triggers startup notification
4. Saved position lost after hide — save before hiding

### Windows

- Standard window positioning
- Always-on-top works reliably

### macOS

- Uses menu bar (not system tray)
- Requires native code signing

## Database

- **Engine:** SQLite (async with sqlx)
- **Migrations:** Use `ALTER TABLE` (backwards compatible)
- **Settings:** Stored in `app_settings` table (id='default')

## Testing Recommendations

- Manual testing on Windows, Linux, macOS
- Test pre-release (beta) tags before promoting to stable
- Check window behavior on multiple monitor setups

## Common Mistakes to Avoid

1. ❌ Removing plugins without understanding impact
2. ❌ Using `block_on()` inside Tauri runtime (use `block_in_place()`)
3. ❌ Committing version bumps separately
4. ❌ Setting window position before showing on Linux
5. ❌ Over-engineering simple features

## Useful Commands

```bash
npm install              # Install dependencies
npm run dev              # Dev server
npm run tauri dev        # Tauri dev mode
npm run tauri build      # Build release
git tag v2.1.5          # Tag release
git push origin --tags  # Push tags
```
