use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase { Focus, ShortBreak, LongBreak }

pub struct FocusTimer {
    pub phase: Phase,
    pub cycles: u32,
    phase_start: Option<Instant>,
    paused_elapsed: Duration,
    paused: bool,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            phase: Phase::Focus,
            cycles: 0,
            phase_start: None,
            paused_elapsed: Duration::ZERO,
            paused: true,
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

    pub fn remaining(&self) -> Duration {
        let dur = Self::phase_duration(self.phase);
        if self.paused {
            dur.saturating_sub(self.paused_elapsed)
        } else if let Some(start) = self.phase_start {
            let elapsed = self.paused_elapsed + start.elapsed();
            dur.saturating_sub(elapsed)
        } else {
            dur.saturating_sub(self.paused_elapsed)
        }
    }

    pub fn is_running(&self) -> bool {
        self.phase_start.is_some() && !self.paused
    }

    pub fn start(&mut self) {
        if self.phase_start.is_none() {
            self.phase_start = Some(Instant::now());
        }
        self.paused = false;
    }

    pub fn pause(&mut self) {
        if let Some(start) = self.phase_start.take() {
            self.paused_elapsed += start.elapsed();
        }
        self.paused = true;
    }

    pub fn tick(&mut self) -> bool {
        // returns true if phase just completed
        if self.paused || self.phase_start.is_none() {
            return false;
        }
        let dur = Self::phase_duration(self.phase);
        let elapsed = self.paused_elapsed + self.phase_start.unwrap().elapsed();
        if elapsed >= dur {
            // advance phase
            match self.phase {
                Phase::Focus => {
                    self.cycles += 1;
                    self.phase = if self.cycles % 4 == 0 { Phase::LongBreak } else { Phase::ShortBreak };
                }
                _ => self.phase = Phase::Focus,
            }
            self.phase_start = None;
            self.paused_elapsed = Duration::ZERO;
            self.paused = true;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Focus;
        self.cycles = 0;
        self.phase_start = None;
        self.paused_elapsed = Duration::ZERO;
        self.paused = true;
    }
}
