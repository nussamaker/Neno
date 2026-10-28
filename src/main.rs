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
mod shelf;
mod state;
mod todo;
mod tray;
mod water;

fn main() {
    state::migrate_from_nusa();
    state::ensure_dirs();
    notch::init_focus();

    // Create and show the notch window FIRST so the user sees something immediately.
    let _notch = match notch::Notch::create() {
        Ok(n) => {
            n.show();
            Some(n)
        }
        Err(e) => {
            // Can't show window; bail.
            eprintln!("notch create failed: {e}");
            return;
        }
    };

    // Create panel (hidden) and tray icon.
    let _ = panel::create_panel();
    let _tray = tray::TrayIcon::create();

    // Now do the (potentially slow) data gathering.
    let settings = settings::Settings::load();
    let todo = todo::TodoList::load();
    let water = water::WaterReminder::load();
    let focus_remaining = notch::FOCUS.get().and_then(|m| m.lock().ok()).map(|f| f.remaining().as_secs()).unwrap_or(0);

    let mut notch_text = format!("Focus {}s", focus_remaining);
    let bt = bluetooth::list_paired_devices();
    notch::set_bt_count(bt.len());
    let nc = (|| {
        notifications::request_access();
        notifications::notification_count()
    })();
    notch::set_notif_count(nc);

    if let Some(m) = media::MediaSession::current() {
        notch::set_media_title(&format!("{} — {}", m.title, m.artist));
        if let Some(l) = lyrics::fetch_lyrics(&m.title, &m.artist) {
            notch::set_lyrics(&l);
        }
    }

    notch_text = format!(
        "Focus {}s | BT {} | Notif {}",
        focus_remaining, bt.len(), nc
    );
    notch::set_text(&notch_text);

    // Run message loop (blocks until window closes).
    notch::Notch::run_message_loop();

    let _ = (settings, todo, water);
}
