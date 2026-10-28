use std::time::{Duration, Instant};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

pub struct SystemMonitor {
    pub cpu_usage: f32,   // 0.0 - 100.0
    pub ram_usage: f32,   // 0.0 - 100.0
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    last_idle: u64,
    last_kernel: u64,
    last_user: u64,
    last_sample: Instant,
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            ram_usage: 0.0,
            ram_total_mb: 0,
            ram_used_mb: 0,
            last_idle: 0,
            last_kernel: 0,
            last_user: 0,
            last_sample: Instant::now(),
        }
    }
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut m = Self::default();
        m.sample();
        m
    }

    pub fn sample(&mut self) {
        // CPU via GetSystemTimes
        unsafe {
            let mut idle = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                let idle = filetime_to_u64(&idle);
                let kernel = filetime_to_u64(&kernel);
                let user = filetime_to_u64(&user);
                let idle_diff = idle.saturating_sub(self.last_idle);
                let kernel_diff = kernel.saturating_sub(self.last_kernel);
                let user_diff = user.saturating_sub(self.last_user);
                let total = kernel_diff + user_diff;
                if total > 0 {
                    self.cpu_usage = (1.0 - (idle_diff as f64 / total as f64)) as f32 * 100.0;
                }
                self.last_idle = idle;
                self.last_kernel = kernel;
                self.last_user = user;
            }
        }
        // RAM via GlobalMemoryStatusEx
        unsafe {
            let mut ms = MEMORYSTATUSEX::default();
            ms.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut ms).is_ok() {
                self.ram_total_mb = ms.ullTotalPhys / (1024 * 1024);
                self.ram_used_mb = (ms.ullTotalPhys - ms.ullAvailPhys) / (1024 * 1024);
                self.ram_usage = ms.dwMemoryLoad as f32;
            }
        }
        self.last_sample = Instant::now();
    }

    pub fn sample_if_stale(&mut self, max_age: Duration) {
        if self.last_sample.elapsed() >= max_age {
            self.sample();
        }
    }
}

fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}
