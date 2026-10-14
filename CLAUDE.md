# CLAUDE.md — Claude Code Guidelines for OmaFLAC

## Build & Test Commands
- **Check Compilation**: `cargo check`
- **Run Tests**: `cargo test`
- **Build Release**: `cargo build --release`
- **Install to Local Path**: `cargo build --release && install -m 755 target/release/omaflac ~/.local/bin/omaflac`
- **Run Player**: `./target/release/omaflac`

## Architectural Guidelines
1. **Never write `println!` or `eprintln!` during TUI execution**: Raw stdout/stderr writes corrupt Ratatui's alternate screen buffer.
2. **Audio Gapless Pipeline**: Preload next tracks via `AudioEngine::set_next_track`. Never flush CPAL ring buffers on normal track boundaries.
3. **Monochromatic Glyph Rule**: Use monochrome Unicode glyphs (`★`, `≡`, `◉`, `○`, `▪`, `▸`, `♪`, `♫`, `⟳`, `+`), never colored emojis.
4. **Rounded Borders**: All panels use `BorderType::Rounded` (`╭`, `╮`, `╯`, `╰`), replicating `btop` styling.
5. **Sidebar Order & Default**:
   - Order: `Artists (1) → Albums (2) → Songs (3) → Now Playing (4) → Playlists (5) → Settings (6)`.
   - Default initial view: `ViewMode::Artists`.
6. **Focus Highlights**: Active focus area (`FocusArea::Sidebar`, `FocusArea::MainView`, `FocusArea::PlayerBar`) must highlight border with `theme.block_focus_style()` and title with `●`.
7. **Mouse/Keyboard Parity**: Every clickable widget must register a click target struct and handle events in `src/app.rs`.
8. **Terminal-Locked Theme**: OmaFLAC always assumes and locks to the host terminal's theme (Ghostty, Kitty, Alacritty, WezTerm, Foot, Omarchy, or terminal OSC query). Never add manual theme selectors.
