# Neno

A Windows "notch" overlay app — music, lyrics, focus timer, water reminder, to-do list, and file shelf in a compact always-on-top bar.

Reconstructed from reverse engineering of the original "Nusa" app, rebranded to **Neno**.

## Download

The latest installer is on the [Releases page](https://github.com/nussamaker/Neno/releases/latest). Run `NenoSetup.exe`; it installs to `%LOCALAPPDATA%\Programs\Neno` and runs without a console window.

## Features

- **Notch overlay** — Nusa-style compact clock pill at the top of the screen that expands into a rounded five-page card (media, focus, water, to-do, shelf), custom GDI drawing, always-on-top
- **Themes** — Dark (default), Light, and Aurora, an animated teal/cyan/violet glass field with drifting ribbons; cycle it from Settings and it is remembered across restarts
- **Mascot** — the original Nusa character lives beside the clock in the collapsed pill: small capsule eyes and a pixel-art mitten that rests, waves on launch and when you return, and dozes off (closed slits + “Z”) after 9 seconds without input
- **Media** — now-playing via WinRT, album art placeholder, progress bar, play/pause and skip controls, LRCLIB lyrics fetch + local cache
- **Navigation** — click the pill to expand, click the page dots or side chevrons, scroll the mouse wheel, right-click or move away to collapse
- **Focus timer** — Pomodoro (25 min focus / 5 min short break / 15 min long break)
- **Water reminder** — bottle goals (1.5 / 2.0 / 2.5 / 3.0 L), refill/snooze, progress
- **To-do list** — dark task panel with checkboxes: add (type + Enter), toggle (click), remove (right-click), scroll with the wheel, persisted to `%APPDATA%\Neno\todo.txt`
- **Shelf** — styled drop zone in the notch that lists the three newest files (newest first); file copy + `.url` Internet Shortcuts, drag-and-drop (WM_DROPFILES)
- **Autostart** — toggle `HKCU\...\Run\Neno`
- **System tray icon** — the cat-character icon (small eyes on an aurora badge); left-click toggles the to-do panel; right-click opens a dark menu with Toggle Panel / Toggle Notch / Settings / Quit
- **Bluetooth** — paired device listing
- **Notifications** — count via UserNotificationListener
- **Network** — connectivity check, shared HTTP client with timeouts
- **Weather** — Open-Meteo current conditions, cached 30 min
- **Settings window** — dark Nusa-styled card for water goal, autostart, theme, and weather; Esc to close
- **System monitor** — CPU / RAM in the notch text
- **State migration** — auto-migrates legacy `%APPDATA%\Nusa` → `%APPDATA%\Neno`

## Prerequisites

- **Windows 10/11** (x64)
- **Rust** (stable, MSVC toolchain): https://rustup.rs
- **Windows SDK** (installed with Visual Studio Build Tools or Visual Studio)

## Build

```powershell
git clone https://github.com/nussamaker/Neno.git
cd Neno
cargo build --release
```

The binary will be at `target\release\neno.exe`.

## Run

```powershell
.\target\release\neno.exe
```

A small notch window appears at the top-center of your screen. Click it to expand the card; the to-do page opens the task panel.

## Project structure

```
neno/
├── Cargo.toml
├── build.rs             # embeds assets/neno.ico as the executable icon
├── assets/
│   └── neno.ico         # original cat character, 16-256 px
├── src/
│   ├── main.rs          # entry point, wires all modules
│   ├── notch.rs         # Win32 window + custom GDI rendering + timer
│   ├── panel.rs         # To-Do + water panel window
│   ├── settings_window.rs # Settings card window
│   ├── ui.rs            # shared GDI helpers + dark DWM frame
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
│   ├── weather.rs       # Open-Meteo fetch + cache
│   ├── worker.rs        # background polling worker
│   ├── sysmon.rs        # CPU / RAM sampling
│   └── network.rs       # connectivity check
├── docs/
│   └── REA_REPORT.md    # reverse-engineering analysis report
└── README.md
```

## Settings

Optional keys in `%APPDATA%\Neno\settings.txt` (one `key=value` per line):

| Key | Default | Meaning |
|---|---|---|
| `theme` | `dark` | `dark`, `light`, or `aurora` |
| `water_goal` | `2.0` | daily water goal in litres (set by the Settings window) |
| `weather_enabled` | `true` | set to `false` to disable weather (no network call) |
| `weather_lat` | `51.5074` | latitude, -90 to 90 (default is London) |
| `weather_lon` | `-0.1278` | longitude, -180 to 180 |

Weather is re-read from settings about every 10 minutes, so edits apply without a restart.

## Threading

The UI thread only creates windows and pumps messages. Anything slow or blocking
(WinRT media/Bluetooth/notification queries, LRCLIB, Open-Meteo) runs on a single
background worker (`worker.rs`) with HTTP timeouts, and publishes results through
the mutex-protected setters in `notch.rs`. The notch's 1-second timer assembles the
displayed text from whatever has arrived.

## Data locations

- `%APPDATA%\Neno\` — `settings.txt`, `todo.txt`, `water.txt`, `shelf.txt`, `lyrics\`
- `%LOCALAPPDATA%\Neno\Shelf\` — dropped shelf files

## Provenance

Neno was reconstructed from static analysis of the **Nusa** application by Rama
(https://rama2555.github.io/nusa/) and rebranded. It is not a clean-room
implementation. Before distributing it, confirm Nusa's license terms or get the
original author's permission, and credit them accordingly. The analysis report is
in `docs/REA_REPORT.md`, and original identifiers (`Nusa`, `nusa.exe`, AppId
`{A03D4881-…}`, hashes) are preserved there as evidence.

## License

TODO: no license has been chosen yet. Add a `LICENSE` file once the provenance
question above is settled.
