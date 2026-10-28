use std::fs;
use crate::state;

#[derive(Clone)]
pub struct WaterReminder {
    pub goal_liters: f32,
    pub consumed_ml: u32,
    pub snoozed: bool,
    pub last_date: String,
}

impl Default for WaterReminder {
    fn default() -> Self {
        Self {
            goal_liters: 2.0,
            consumed_ml: 0,
            snoozed: false,
            last_date: today_string(),
        }
    }
}

fn today_string() -> String {
    // Simple date string using system time (no chrono dep).
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // days since epoch
    let days = secs / 86400;
    format!("{}", days)
}

impl WaterReminder {
    pub fn load() -> Self {
        let path = state::appdata_dir().join("water.txt");
        let mut w = Self::default();
        if let Ok(txt) = fs::read_to_string(&path) {
            for line in txt.lines() {
                if let Some(v) = line.strip_prefix("goal=") {
                    if let Ok(f) = v.parse::<f32>() { w.goal_liters = f; }
                } else if let Some(v) = line.strip_prefix("consumed=") {
                    if let Ok(n) = v.parse::<u32>() { w.consumed_ml = n; }
                } else if let Some(v) = line.strip_prefix("snoozed=") {
                    w.snoozed = v == "1";
                } else if let Some(v) = line.strip_prefix("date=") {
                    w.last_date = v.to_string();
                }
            }
        }
        // Settings override
        let settings = crate::settings::Settings::load();
        if let Some(v) = settings.get("water_goal") {
            if let Ok(f) = v.parse::<f32>() { w.goal_liters = f; }
        }
        // Daily reset: if date changed, reset consumed.
        let today = today_string();
        if w.last_date != today {
            w.consumed_ml = 0;
            w.snoozed = false;
            w.last_date = today;
        }
        w
    }

    pub fn save(&self) {
        let dir = state::appdata_dir();
        let _ = fs::create_dir_all(&dir);
        let out = format!(
            "goal={}\nconsumed={}\nsnoozed={}\ndate={}\n",
            self.goal_liters, self.consumed_ml,
            if self.snoozed { 1 } else { 0 },
            self.last_date
        );
        let _ = fs::write(dir.join("water.txt"), out);
    }

    pub fn progress(&self) -> f32 {
        if self.goal_liters <= 0.0 {
            return 0.0;
        }
        (self.consumed_ml as f32 / (self.goal_liters * 1000.0)).clamp(0.0, 1.0)
    }

    pub fn refill(&mut self, amount_ml: u32) {
        self.consumed_ml = self.consumed_ml.saturating_add(amount_ml);
        self.snoozed = false;
        self.save();
    }

    pub fn snooze(&mut self) {
        self.snoozed = true;
        self.save();
    }
}
