# AGENTS.md — Contributor & AI Agent Guide for OmaFLAC

Welcome to **OmaFLAC**! This document provides a concise, high-leverage architectural map, coding conventions, and operational workflows for AI agents and human contributors working on this codebase.

---

## 1. Project Overview & Philosophy

- **What it is**: `OmaFLAC` is an audiophile-grade Terminal User Interface (TUI) audio player built in pure Rust for **Omarchy Linux** and modern Linux desktop environments.
- **Core Technology Stack**:
  - **TUI Framework**: [`ratatui`](https://crates.io/crates/ratatui) + [`crossterm`](https://crates.io/crates/crossterm) (raw mode, alternate screen, mouse capture).
  - **Audio Pipeline**: [`cpal`](https://crates.io/crates/cpal) (low-latency output stream) + [`symphonia`](https://crates.io/crates/symphonia) (bit-perfect decoding for FLAC, MP3, WAV, OGG, M4A/AAC, ALAC).
  - **Metadata & Tagging**: [`lofty`](https://crates.io/crates/lofty) (tag reading, album art extraction).
  - **DSP & FFT**: [`rustfft`](https://crates.io/crates/rustfft) (1024-point FFT real-time audio visualizer).
  - **Crossbeam**: [`crossbeam-channel`](https://crates.io/crates/crossbeam-channel) (lock-free lockstep audio boundary transfer).

---

## 2. Architecture & Codebase Map

```
omaflac/
├── Cargo.toml
├── Cargo.lock
├── build.rs                        # Embeds compile-time Git commit hash (OMAFLAC_GIT_HASH)
├── install.sh                      # Cross-platform installer & updater script
├── tests/
│   ├── library_test.rs             # Integration tests for library scanning & hierarchy
│   └── search_and_features_test.rs # Integration tests for search, two-pane albums, and artwork cache
└── src/
    ├── main.rs                     # Entry point: CLI commands, terminal setup, event loop
    ├── lib.rs                      # Library crate root
    ├── app.rs                      # Core state machine (App), focus management, key/mouse routing
    ├── config.rs                   # AppConfig (~/.config/omaflac/config.json), repeat/shuffle modes
    ├── updater.rs                  # Background GitHub version check & self-update engine
    │
    ├── audio/                      # Audio subsystem (Decoupled engine & threads)
    │   ├── mod.rs                  # AudioEngine public facade
    │   ├── player.rs               # AudioPlayer background thread, CPAL output callback, gapless queue
    │   ├── visualizer.rs           # VisualizerBridge, real-time lock-free FFT buffer & smoothing
    │   └── waveform.rs             # Background audio amplitude dynamics pre-calculation & caching
    │
    ├── library/                    # Music library, metadata, and playlist persistence
    │   ├── mod.rs                  # Library struct (tracks collection, search indexing, hierarchy)
    │   ├── track.rs                # Track struct (path, title, artist, album, duration, audio format)
    │   ├── scanner.rs              # Recursive multithreaded directory scanner via walkdir & lofty
    │   └── playlist.rs             # PlaylistsManager (~/.config/omaflac/playlists.json)
    │
    └── ui/                         # Presentation layer (Ratatui widgets & views)
        ├── mod.rs                  # Root UI layout, header rendering, view dispatcher
        ├── theme.rs                # OmarchyTheme palette, color tokens, and style helpers
        ├── artwork.rs              # 24-bit RGB half-block (▀) album cover renderer & cache
        ├── spectrum.rs             # Real-time FFT frequency spectrum analyzer widget
        ├── waveform_widget.rs      # btop-style 2x4 Braille matrix (U+2800..U+28FF) waveform widget
        ├── components/
        │   ├── header.rs           # Top bar: stylized "OmaFLAC" title, search, update & status badges
        │   ├── sidebar.rs          # Left navigation: Artists, Albums, Songs, Now Playing, Playlists
        │   ├── player_bar.rs       # Persistent bottom player bar: controls, progress line, volume
        │   ├── about_modal.rs      # Stylized ASCII art "About OmaFLAC" modal dialog
        │   ├── folder_dialog.rs    # Interactive "Add Library Folder" modal dialog
        │   ├── help_modal.rs       # F1 / ? controls overlay dialog
        │   └── playlist_dialog.rs  # Add-to-playlist / New-playlist modal dialog
        └── views/
            ├── artists_view.rs     # 3-column hierarchical browser (Artists → Albums → Songs)
            ├── albums_view.rs      # 2-column Album catalog & Songs browser
            ├── songs_view.rs       # Filterable track table with metadata & favorite stars
            ├── search_results_view.rs # 3-column instant search results (Artists, Albums, Songs)
            ├── now_playing.rs      # Large album art, metadata, spectrum visualizer, compact waveform
            ├── playlists_view.rs   # Playlist manager and track listing
            ├── settings_view.rs    # Library directories management, rescan triggers, and updates
            └── file_browser.rs     # Standalone filesystem browser view
```

---

## 3. Critical Architectural Rules & Invariants

### 1. No `println!` or `eprintln!` in TUI Mode
- **Never** write debug prints to standard output or standard error while the TUI is running. Doing so bypasses the terminal alternate-screen buffer and corrupts the rendered cells.
- If debugging is necessary, write to a log file or inspect via tests.

### 2. Gapless Audio Pipeline Invariants
- Audio playback transitions between consecutive songs are **gapless**.
- The upcoming track is pre-buffered on a background thread (`SetNextTrack`).
- The Symphonia decoder thread switches formats at EOF and emits an `AudioItem::Boundary` token directly into the bounded channel.
- The CPAL stream callback consumes boundaries without stopping, pausing, or flushing the output stream, resetting `elapsed` and emitting `PlayerEvent::TrackTransitioned`.
- Whenever the playlist, current track, repeat mode, or shuffle mode changes, `self.update_next_track()` must be called to update the predictive buffer.

### 3. Visual Styling & Omarchy Aesthetic
- **Monochromatic Glyph Rule**: Never use colored emoji characters. Use monochrome Unicode glyphs styled with theme colors (`★`, `≡`, `◉`, `○`, `▪`, `▸`, `♪`, `♫`, `⟳`, `+`, `‖`, `⇄`, `↻`).
- **Rounded Borders (`btop` Styling)**: All boxes, panels, modals, and dialogs strictly use `BorderType::Rounded` (`╭`, `╮`, `╯`, `╰`).
- **Section Highlights**: Active/focused sections (`FocusArea::Sidebar`, `FocusArea::MainView`, `FocusArea::PlayerBar`) must highlight their borders with `theme.block_focus_style()` and title dots `●`.
- **Side Nav Order**: Library order is strictly **Artists (1) → Albums (2) → Songs (3) → Now Playing (4)**. Default startup view is **Artists**.

### 4. Mouse & Keyboard Interaction Parity
- Every clickable button, list item, or slider must register a click target struct during rendering (e.g., `PlayerClickTarget`, `SidebarClickTarget`, `ArtistsViewClickTarget`).
- Mouse events in `src/app.rs` route through `handle_mouse_event` to ensure 100% feature parity between keyboard shortcuts and mouse clicks/scrolls.

---

## 4. Common Developer Workflows

### Check Code & Types
```bash
cargo check
```

### Run Test Suite
```bash
cargo test
```

### Build & Install Release Binary
```bash
./install.sh
```

### Check Version & Self-Update via CLI
```bash
omaflac --version
omaflac check-update
omaflac update
```

### Run Player Locally
```bash
./target/release/omaflac
```

---

## 5. Key Structs & State References

- **`App`** ([`src/app.rs`](file:///home/jon/Documents/GitHub/omaflac/src/app.rs)):
  - `view_mode: ViewMode`: Active view (default: `ViewMode::Artists`).
  - `focus: FocusArea`: Current keyboard focus (`Sidebar`, `MainView`, `PlayerBar`).
  - `audio: AudioEngine`: Thread-safe handle to the playback engine.
  - `library: Library`: In-memory metadata database.
  - `playlists_manager: PlaylistsManager`: JSON-persisted user playlists.
- **`AudioEngine`** ([`src/audio/mod.rs`](file:///home/jon/Documents/GitHub/omaflac/src/audio/mod.rs)):
  - `play_track(track)`: Immediate synchronous track playback.
  - `set_next_track(track)`: Asynchronous pre-buffering for gapless transitions.
  - `visualizer()`: Lock-free bridge to real-time audio FFT data.
- **`OmarchyTheme`** ([`src/ui/theme.rs`](file:///home/jon/Documents/GitHub/omaflac/src/ui/theme.rs)):
  - Accent colors: `accent` (`#9ec66a` forest green), `cyan` (`#7dcfff`), `purple` (`#bb9af7`).
  - Backgrounds: `background` (`#1a1b26`), `dark_background` (`#16161e`), `lighter_background` (`#24283b`).
