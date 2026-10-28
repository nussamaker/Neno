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
mod tray;
mod state;
mod todo;
mod water;

fn main() {
    state::migrate_from_nusa();
    state::ensure_dirs();
    notch::init_focus();

    let settings = settings::Settings::load();
    let todo = todo::TodoList::load();
    let water = water::WaterReminder::load();
    let mut focus = focus::FocusTimer::new();
    let shelf = shelf::Shelf::new();

    println!("Neno skeleton");
    println!("appdata: {:?}", state::appdata_dir());
    println!("settings keys: {}", settings.values.len());
    println!("todo items: {}", todo.items.len());
    println!("water goal: {} L, consumed: {} ml", water.goal_liters, water.consumed_ml);
    println!("focus phase: {:?}, remaining: {}s", focus.phase, focus.remaining.as_secs());
    println!("shelf files: {}", shelf.files.len());
    println!("network connected: {}", network::is_connected());
    let bt = bluetooth::list_paired_devices();
    println!("bluetooth devices: {:?}", bt);
    notch::set_bt_count(bt.len());
    println!("notifications access: {}", notifications::request_access());
    let nc = notifications::notification_count();
    println!("notification count: {}", nc);
    notch::set_notif_count(nc);

    let mut notch_text = format!("Focus: {}s", focus.remaining.as_secs());
    if let Some(m) = media::MediaSession::current() {
        println!("now playing: {} — {}", m.title, m.artist);
        notch::set_media_title(&format!("{} — {}", m.title, m.artist));
        notch_text = format!("{} — {} | Focus {}s", m.title, m.artist, focus.remaining.as_secs());
        if let Some(l) = lyrics::fetch_lyrics(&m.title, &m.artist) {
            println!("lyrics (first 80 chars): {}", &l[..l.len().min(80)]);
            notch::set_lyrics(&l);
        }
    } else {
        println!("no media session");
    }
    notch::set_text(&notch_text);

    // autostart example: ensure enabled pointing at current exe
    if let Ok(exe) = std::env::current_exe() {
        autostart::set_autostart(true, &exe);
    }

    match notch::Notch::create() {
        Ok(n) => {
            n.show();
            // Enable legacy WM_DROPFILES drag-and-drop on the notch window.
            println!("drag-and-drop (WM_DROPFILES) enabled");
            let _ = panel::create_panel();
            let _tray = tray::TrayIcon::create();
            println!("notch window created; running message loop (close window to exit)");
            notch::Notch::run_message_loop();
            unsafe {
                use windows::Win32::System::Ole::OleUninitialize;
                OleUninitialize();
            }
        }
        Err(e) => println!("notch create failed: {e}"),
    }

    let _ = (settings, todo, water, focus, shelf);
}
