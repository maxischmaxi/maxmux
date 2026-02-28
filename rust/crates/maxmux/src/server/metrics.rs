// System metrics collector – reads Linux pseudo-filesystems (/proc, /sys) and
// spawns git commands to populate the status bar's SystemMetrics struct.

use maxmux_statusbar::modules::{BatteryInfo, CpuInfo, GitInfo, MemoryInfo};

/// Collects system-level metrics for the status bar.
///
/// CPU usage is computed as a delta between successive reads of `/proc/stat`,
/// so the first call to [`collect_cpu`] always returns `None`.
pub struct SystemMetricsCollector {
    prev_cpu_total: u64,
    prev_cpu_idle: u64,
    cpu_initialized: bool,
}

impl SystemMetricsCollector {
    pub fn new() -> Self {
        Self {
            prev_cpu_total: 0,
            prev_cpu_idle: 0,
            cpu_initialized: false,
        }
    }

    /// Read CPU usage from `/proc/stat`.
    ///
    /// Returns `None` on the first call (no previous sample to compute a
    /// delta against) or if the file cannot be read.
    pub fn collect_cpu(&mut self) -> Option<CpuInfo> {
        let stat = std::fs::read_to_string("/proc/stat").ok()?;
        let line = stat.lines().next()?;
        let fields: Vec<u64> = line
            .split_whitespace()
            .skip(1) // skip "cpu"
            .filter_map(|s| s.parse().ok())
            .collect();

        let total: u64 = fields.iter().sum();
        let idle = fields.get(3).copied().unwrap_or(0);

        if !self.cpu_initialized {
            self.prev_cpu_total = total;
            self.prev_cpu_idle = idle;
            self.cpu_initialized = true;
            return None;
        }

        let dtotal = total.saturating_sub(self.prev_cpu_total);
        let didle = idle.saturating_sub(self.prev_cpu_idle);

        self.prev_cpu_total = total;
        self.prev_cpu_idle = idle;

        if dtotal == 0 {
            return None;
        }
        let usage = ((dtotal - didle) as f64 / dtotal as f64) * 100.0;
        Some(CpuInfo { usage })
    }

    /// Read memory statistics from `/proc/meminfo`.
    pub fn collect_memory(&self) -> Option<MemoryInfo> {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        let mut total_kb = 0u64;
        let mut available_kb = 0u64;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line.split_whitespace().nth(1)?.parse().ok()?;
            } else if line.starts_with("MemAvailable:") {
                available_kb = line.split_whitespace().nth(1)?.parse().ok()?;
            }
        }

        let total_mb = total_kb as f64 / 1024.0;
        let used_mb = (total_kb - available_kb) as f64 / 1024.0;
        let percentage = if total_mb > 0.0 {
            (used_mb / total_mb) * 100.0
        } else {
            0.0
        };

        Some(MemoryInfo {
            used_mb,
            total_mb,
            percentage,
        })
    }

    /// Read battery info from `/sys/class/power_supply/BAT0/`.
    ///
    /// Returns `None` if no battery is present (e.g. on a desktop).
    pub fn collect_battery(&self) -> Option<BatteryInfo> {
        let capacity = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity").ok()?;
        let status = std::fs::read_to_string("/sys/class/power_supply/BAT0/status").ok()?;
        Some(BatteryInfo {
            level: capacity.trim().parse().ok()?,
            charging: status.trim() == "Charging",
            present: true,
        })
    }

    /// Read the system hostname from `/proc/sys/kernel/hostname`.
    #[allow(dead_code)]
    pub fn collect_hostname(&self) -> Option<String> {
        std::fs::read_to_string("/proc/sys/kernel/hostname")
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Read the current username from the `$USER` environment variable.
    #[allow(dead_code)]
    pub fn collect_username(&self) -> Option<String> {
        std::env::var("USER").ok()
    }

    /// Collect git information for a working directory.
    ///
    /// Spawns `git rev-parse`, `git status`, and `git rev-list` to obtain
    /// branch name, dirty state, and ahead/behind counts.
    #[allow(dead_code)]
    pub fn collect_git(&self, cwd: &str) -> Option<GitInfo> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(cwd)
            .output()
            .ok()?;
        let dirty = !String::from_utf8_lossy(&status.stdout).trim().is_empty();

        // Ahead/behind relative to upstream (may fail if no upstream is set).
        let ab = Command::new("git")
            .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
            .current_dir(cwd)
            .output()
            .ok();
        let (ahead, behind) = ab
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let parts: Vec<&str> = s.trim().split('\t').collect();
                    Some((
                        parts.first()?.parse::<u32>().ok()?,
                        parts.get(1)?.parse::<u32>().ok()?,
                    ))
                } else {
                    None
                }
            })
            .unwrap_or((0, 0));

        Some(GitInfo {
            branch,
            dirty,
            ahead,
            behind,
        })
    }
}

impl Default for SystemMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_memory_reads_proc_meminfo() {
        let collector = SystemMetricsCollector::new();
        let mem = collector.collect_memory();
        // On a Linux system /proc/meminfo should be readable.
        assert!(mem.is_some(), "should be able to read /proc/meminfo");
        let mem = mem.unwrap();
        assert!(mem.total_mb > 0.0, "total memory should be > 0");
        assert!(mem.used_mb >= 0.0, "used memory should be >= 0");
        assert!(
            mem.percentage >= 0.0 && mem.percentage <= 100.0,
            "percentage should be 0-100"
        );
    }

    #[test]
    fn test_collect_hostname_returns_something() {
        let collector = SystemMetricsCollector::new();
        let hostname = collector.collect_hostname();
        assert!(hostname.is_some(), "should be able to read hostname");
        let hostname = hostname.unwrap();
        assert!(!hostname.is_empty(), "hostname should not be empty");
    }

    #[test]
    fn test_collect_username_returns_something() {
        let collector = SystemMetricsCollector::new();
        let username = collector.collect_username();
        // $USER should be set in most environments.
        assert!(username.is_some(), "should be able to read $USER");
        let username = username.unwrap();
        assert!(!username.is_empty(), "username should not be empty");
    }

    #[test]
    fn test_collect_cpu_first_call_returns_none() {
        let mut collector = SystemMetricsCollector::new();
        // First call has no previous sample so returns None.
        let cpu = collector.collect_cpu();
        assert!(
            cpu.is_none(),
            "first CPU sample should return None (no delta)"
        );
    }

    #[test]
    fn test_collect_cpu_second_call_returns_some() {
        let mut collector = SystemMetricsCollector::new();
        collector.collect_cpu(); // prime
        // Sleep briefly to get a measurable delta.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let cpu = collector.collect_cpu();
        assert!(
            cpu.is_some(),
            "second CPU sample should return Some with usage"
        );
        let cpu = cpu.unwrap();
        assert!(
            cpu.usage >= 0.0 && cpu.usage <= 100.0,
            "CPU usage should be 0-100, got {}",
            cpu.usage
        );
    }
}
