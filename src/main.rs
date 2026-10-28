#![windows_subsystem = "windows"]

mod autostart;
mod bluetooth;
mod focus;
mod lyrics;
mod media;
mod network;
mod notifications;
mod notch;
mod panel;
mod settings;
mod settings_window;
mod shelf;
mod state;
mod sysmon;
mod todo;
mod tray;
mod ui;
mod water;
mod weather;
mod worker;

fn main() {
    state::migrate_from_nusa();
    state::ensure_dirs();
    notch::init_focus();

    // Load theme from settings.
    let settings = settings::Settings::load();
    if let Some(t) = settings.get("theme") {
        notch::set_theme(t == "dark");
    }

    // Create and show the notch window FIRST so the user sees something immediately.
    let _notch = match notch::Notch::create() {
        Ok(n) => {
            n.show();
            Some(n)
        }
        Err(e) => {
            eprintln!("notch create failed: {e}");
            return;
        }
    };

    // Create panel (hidden), settings window (hidden) and tray icon.
    let _ = panel::create_panel();
    let _ = settings_window::create_settings();
    let _tray = tray::TrayIcon::create();

    // Slow lookups (media, lyrics, weather, Bluetooth, notifications) run off the UI thread.
    // The notch's 1-second timer builds the displayed text from whatever has arrived so far.
    worker::spawn();

    // Run message loop (blocks until window closes).
    notch::Notch::run_message_loop();
}
