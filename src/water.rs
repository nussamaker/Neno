use std::fs;
use crate::state;

#[derive(Clone)]
pub struct WaterReminder {
    pub goal_liters: f32,   // 1.5 / 2.0 / 2.5 / 3.0
    pub consumed_ml: u32,
    pub snoozed: bool,
}

impl Default for WaterReminder {
    fn default() -> Self {
        Self { goal_liters: 2.0, consumed_ml: 0, snoozed: false }
    }
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
                }
            }
        }
        w
    }

    pub fn save(&self) {
        let dir = state::appdata_dir();
        let _ = fs::create_dir_all(&dir);
        let out = format!("goal={}\nconsumed={}\nsnoozed={}\n",
            self.goal_liters, self.consumed_ml, if self.snoozed { 1 } else { 0 });
        let _ = fs::write(dir.join("water.txt"), out);
    }

    pub fn progress(&self) -> f32 {
        self.consumed_ml as f32 / (self.goal_liters * 1000.0)
    }
}
