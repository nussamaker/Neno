use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase { Focus, ShortBreak, LongBreak }

pub struct FocusTimer {
    pub phase: Phase,
    pub remaining: Duration,
    pub cycles: u32,
    started: Option<Instant>,
    paused: bool,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            phase: Phase::Focus,
            remaining: Duration::from_secs(25 * 60),
            cycles: 0,
            started: None,
            paused: false,
        }
    }
}

impl FocusTimer {
    pub fn new() -> Self { Self::default() }

    pub fn phase_duration(phase: Phase) -> Duration {
        match phase {
            Phase::Focus => Duration::from_secs(25 * 60),
            Phase::ShortBreak => Duration::from_secs(5 * 60),
            Phase::LongBreak => Duration::from_secs(15 * 60),
        }
    }

    pub fn start(&mut self) {
        self.started = Some(Instant::now());
        self.paused = false;
    }

    pub fn pause(&mut self) {
        if let Some(s) = self.started.take() {
            self.remaining = self.remaining.saturating_sub(s.elapsed());
        }
        self.paused = true;
    }

    pub fn tick(&mut self) -> bool {
        // returns true if phase just completed
        if self.paused || self.started.is_none() { return false; }
        let s = self.started.unwrap();
        let elapsed = s.elapsed();
        if elapsed >= self.remaining {
            self.started = None;
            self.paused = true;
            // advance phase
            match self.phase {
                Phase::Focus => {
                    self.cycles += 1;
                    self.phase = if self.cycles % 4 == 0 { Phase::LongBreak } else { Phase::ShortBreak };
                }
                _ => self.phase = Phase::Focus,
            }
            self.remaining = Self::phase_duration(self.phase);
            true
        } else {
            self.remaining = Self::phase_duration(self.phase) - elapsed;
            false
        }
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Focus;
        self.remaining = Self::phase_duration(Phase::Focus);
        self.started = None;
        self.paused = false;
    }
}
