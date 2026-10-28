# Neno

A Windows "notch" overlay app — music, lyrics, focus timer, water reminder, to-do list, and file shelf in a compact always-on-top bar.

Reconstructed from reverse engineering of the original "Nusa" app, rebranded to **Neno**.

## Features

- **Notch overlay** — always-on-top, Direct2D + DirectWrite rendering, live text (focus countdown, media title, Bluetooth count, notification count, lyrics snippet)
- **Media** — now-playing via WinRT, keyboard controls (Space = play/pause, → = next, ← = prev), LRCLIB lyrics fetch + local cache
- **Focus timer** — Pomodoro (25 min focus / 5 min short break / 15 min long break)
- **Water reminder** — bottle goals (1.5 / 2.0 / 2.5 / 3.0 L), refill/snooze, progress
- **To-do list** — add (type + Enter), toggle (click), persisted to `%APPDATA%\Neno\todo.txt`
- **Shelf** — file copy + `.url` Internet Shortcuts, drag-and-drop (WM_DROPFILES)
- **Autostart** — toggle `HKCU\...\Run\Neno`
- **System tray icon** — click to toggle the to-do panel
- **Bluetooth** — paired device listing
- **Notifications** — count via UserNotificationListener
- **Network** — connectivity check
- **State migration** — auto-migrates legacy `%APPDATA%\Nusa` → `%APPDATA%\Neno`

## Prerequisites

- **Windows 10/11** (x64)
- **Rust** (stable, MSVC toolchain): https://rustup.rs
- **Windows SDK** (installed with Visual Studio Build Tools or Visual Studio)

## Build

```powershell
git clone https://github.com/<your-username>/neno.git
cd neno
cargo build --release
```

The binary will be at `target\release\neno.exe`.

## Run

```powershell
.\target\release\neno.exe
```

A small notch window appears at the top-center of your screen. Click it to open the To-Do panel.

## Project structure

```
neno/
├── Cargo.toml
├── src/
│   ├── main.rs          # entry point, wires all modules
│   ├── notch.rs         # Win32 window + Direct2D/DirectWrite rendering + timer
│   ├── panel.rs         # To-Do + water panel window
│   ├── media.rs         # WinRT media session + controls
│   ├── lyrics.rs        # LRCLIB fetch + file cache
│   ├── focus.rs         # Pomodoro state machine
│   ├── water.rs         # water reminder state
│   ├── todo.rs          # to-do list persistence
│   ├── settings.rs      # key=value settings
│   ├── state.rs         # paths + Nusa→Neno migration
│   ├── shelf.rs         # file shelf + .url shortcuts + drag-and-drop
│   ├── autostart.rs     # registry Run value
│   ├── tray.rs          # system tray icon
│   ├── bluetooth.rs     # paired device listing
│   ├── notifications.rs # notification count
│   └── network.rs       # connectivity check
├── docs/
│   └── REA_REPORT.md    # reverse-engineering analysis report
└── README.md
```

## Data locations

- `%APPDATA%\Neno\` — `settings.txt`, `todo.txt`, `water.txt`, `shelf.txt`, `lyrics\`
- `%LOCALAPPDATA%\Neno\Shelf\` — dropped shelf files

## Reverse engineering

This project was reconstructed from static analysis of `NusaSetup.exe` / `nusa.exe`. The full analysis report is in `docs/REA_REPORT.md`. Original identifiers (`Nusa`, `nusa.exe`, AppId `{A03D4881-…}`, hashes) are preserved there as evidence.

## License

See LICENSE file. (Add your chosen license.)
EOF
echo done