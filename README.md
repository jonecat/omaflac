# ♬ omaflac

> **High-Fidelity TUI Audio Player built for Omarchy Linux**  
> Written in pure Rust with Ratatui, Symphonia, CPAL, and Lofty.

`omaflac` is a terminal audio player engineered specifically for **Omarchy Linux** and modern Linux desktop environments. Designed with the aesthetics and power of Omarchy in mind, it combines audiophile-grade playback with modern TUI capabilities: 24-bit truecolor album artwork, live real-time frequency spectrum analysis, interactive track waveforms, full keyboard & mouse/touch navigation, and an iTunes-style library hierarchy.

---

## ✨ Features

- **Multi-Format Bit-Perfect Decoding**:
  - Full native support for **FLAC**, **MP3**, **OGG** (Vorbis & Opus), **WAV**, **AIFF**, **M4A/AAC**, and **ALAC**.
  - Direct low-latency streaming through ALSA, PipeWire, and PulseAudio via CPAL.
- **Modern TUI with Keyboard, Mouse & Touch**:
  - Interactive mouse click & drag seekbar, volume slider, list scrolling, tab switching, and button controls.
  - Fast keyboard shortcuts (Vim keys `j`/`k`, `h`/`l`, `Tab`, `Space`, `/`, etc.).
- **24-bit Truecolor Album Artwork Visualization**:
  - Automatically extracts embedded artwork tags (ID3 APIC, FLAC picture blocks, MP4 covr) and detects folder art (`cover.jpg`, `folder.png`).
  - Renders album covers using an integrated 24-bit RGB half-block engine (`▀`), achieving double vertical resolution without flickering or protocol incompatibilities across all modern terminals (Ghostty, Foot, Kitty, Alacritty, WezTerm, Tmux).
- **Real-Time Audio Frequency Spectrum Analyzer**:
  - Live audio tap connected to a 1024-point Fast Fourier Transform (FFT) powered by `rustfft`.
  - Logarithmically spaced frequency bands (sub-bass, bass, mids, highs) with equal-loudness weighting.
  - Hardware-style smooth attack/decay physics with peak hold markers rendered using multi-level Unicode block glyphs (` `, `▂`, `▃`, `▄`, `▅`, `▆`, `▇`, `█`).
- **Interactive Full-Track Waveform**:
  - Analyzes audio dynamics to generate an amplitude waveform of the currently playing track.
  - Color-codes played vs. unplayed sections and displays the playback head.
  - Click or drag anywhere on the waveform to seek instantly.
- **iTunes-Style Hierarchical Library & Sidebar**:
  - **Library Views**:
    - **Songs**: Filterable, sortable library table with track metadata, duration, format badges, and favorite stars.
    - **Artists**: 3-column hierarchical browser (**Artists → Albums → Songs**).
    - **Albums**: Album catalog with release years, track counts, and cover indicators.
    - **Now Playing**: Immersive screen featuring large cover art, track info, real-time frequency visualizer, and waveform seekbar.
    - **File Browser**: Built-in filesystem explorer to browse and play folders directly.
  - **Playlists**:
    - ⭐ **Favorites** (quick toggling with `f` or star click)
    - 🕒 **Recently Played**
    - Custom user playlists (create, add songs with `a`, remove songs with `d`).
  - **Settings**:
    - Add and remove library scan folders.
    - Instant library re-scan.
- **Global Instant Search**:
  - Press `/` or click the search box to filter instantly across Title, Artist, Album, and Genre across the entire library.
- **Omarchy Linux Aesthetic**:
  - Omarchy signature forest green (`#9ec66a`), deep slate background (`#1a1b26`), crisp borders, and clean status indicators.

---

## 🚀 Installation & Running

### Requirements
- Omarchy Linux (or any Arch-based distribution).
- ALSA / PipeWire (`alsa-lib` is pre-installed on Omarchy).
- Rust toolchain (managed via `mise` or `rustup`).

### Build & Run
```bash
# Clone the repository
git clone https://github.com/jonecat/omaflac.git
cd omaflac

# Build optimized release binary
cargo build --release

# Run omaflac
./target/release/omaflac
```

### Install Globally
```bash
cargo install --path .
# Now run anywhere:
omaflac
```

---

## ⌨️ Controls & Shortcuts

| Action | Shortcut / Mouse |
| :--- | :--- |
| **Play / Pause** | `Space` or click `▶`/`⏸` |
| **Stop** | `s` or click `⏹` |
| **Next Track** | `n` or click `⏭` |
| **Previous Track** | `p` or click `⏮` |
| **Seek Backward / Forward 5s** | `←` / `→` or click / drag on waveform seekbar |
| **Seek Backward / Forward 30s** | `[` / `]` or `Shift + ←` / `Shift + →` |
| **Volume Up / Down (5%)** | `+` / `-` or mouse wheel on volume bar |
| **Mute / Unmute** | `m` or click `🔊`/`🔇` icon |
| **Cycle Repeat Mode (Off / All / 1)** | `r` or click `🔁` |
| **Toggle Shuffle** | `z` or click `🔀` |
| **Global Search** | `/` or click search bar |
| **Add Track to Playlist** | `a` |
| **Toggle Favorite Track** | `f` or click `★` |
| **Switch Sidebar / Main Focus** | `Tab` / `Shift+Tab` |
| **Direct View Shortcuts** | `1` Artists, `2` Albums, `3` Songs, `4` Now Playing, `5` Playlists, `6` Settings |
| **Navigate Items** | `j` / `k` or `↑` / `↓` or mouse scroll |
| **Switch Columns (Artists View)** | `h` / `l` or `←` / `→` or click column |
| **Play Selected Item** | `Enter` or double-click row |
| **Remove Track from Playlist** | `d` or `Delete` |
| **Open Help Overlay** | `?` or click `[? Help]` |
| **Quit** | `q` or `Ctrl+C` |

---

## 📁 Configuration & Data Paths

`omaflac` stores user configuration and library data in `~/.config/omaflac/`:
- `~/.config/omaflac/config.json`: Library folders, default volume, repeat mode, shuffle.
- `~/.config/omaflac/library.json`: Scanned track database and metadata index.
- `~/.config/omaflac/playlists.json`: User-created playlists, favorites, and playback history.

---

## 🛠️ Architecture

```
src/
├── main.rs                  # Terminal setup, raw mode, mouse capture, 30 FPS tick loop
├── app.rs                   # Core App state, input handling, and event coordinator
├── audio/
│   ├── mod.rs               # AudioEngine facade connecting player, visualizer & waveform
│   ├── player.rs            # Background CPAL streaming thread with Symphonia decoding
│   ├── visualizer.rs        # Real-time FFT audio visualizer & logarithmic band calculator
│   └── waveform.rs          # Audio peak dynamic extraction and downsampling engine
├── library/
│   ├── mod.rs               # Library catalog with artist/album indexing and searching
│   ├── track.rs             # Track data model and audio format metadata
│   ├── scanner.rs           # Multi-format tag reader and album cover finder
│   └── playlist.rs          # Playlist management, favorites, and persistence
├── ui/
│   ├── mod.rs               # Layout manager and frame renderer
│   ├── theme.rs             # Omarchy Linux colors, styles, and glyphs
│   ├── artwork.rs           # 24-bit Truecolor half-block album artwork widget
│   ├── spectrum.rs          # Real-time FFT frequency spectrum analyzer widget
│   ├── waveform_widget.rs   # Symmetrical track waveform & seekbar widget
│   ├── mouse.rs             # Mouse hit-testing and coordinate utilities
│   ├── components/          # Header, Sidebar, Player bar, Playlist dialog, Help modal
│   └── views/               # Songs, Artists, Albums, Now Playing, Playlists, Files, Settings
└── config.rs                # Application configuration loader and serializer
```

---

## 📜 License

MIT License. Designed with care for Omarchy Linux.
