// Process tracker – reads /proc to obtain dynamic window titles and CWD tracking.
//
// This is Linux-specific and reads from the procfs virtual filesystem.

/// Information about a running process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cwd: String,
}

/// Utility for querying process information from `/proc`.
pub struct ProcessTracker;

impl ProcessTracker {
    /// Get process info (name and cwd) from `/proc/<pid>/comm` and `/proc/<pid>/cwd`.
    pub fn get_process_info(pid: u32) -> Option<ProcessInfo> {
        let comm = std::fs::read_to_string(format!("/proc/{}/comm", pid)).ok()?;
        let cwd = std::fs::read_link(format!("/proc/{}/cwd", pid)).ok()?;
        Some(ProcessInfo {
            pid,
            name: comm.trim().to_string(),
            cwd: cwd.to_string_lossy().to_string(),
        })
    }

    /// Get the terminal foreground process group id by reading `/proc/<pid>/stat`.
    ///
    /// Field index 7 (0-based) in the stat file is `tpgid` – the terminal
    /// process group ID of the process.  We need to be careful parsing
    /// because the process name (field 1) is enclosed in parentheses and
    /// may contain spaces.
    pub fn get_foreground_process(pid: u32) -> Option<u32> {
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
        // The comm field (field 1) is wrapped in parens and may contain spaces,
        // so we find the *last* ')' and parse from there.
        let after_comm = stat.rfind(')')? + 1;
        let rest = &stat[after_comm..];
        // Fields after comm start at index 2 in the stat man-page numbering.
        // We want tpgid which is field 7 (0-based), i.e. the 6th field after comm.
        let fields: Vec<&str> = rest.split_whitespace().collect();
        // fields[0] = state (index 2), fields[5] = tpgid (index 7)
        let tpgid: i32 = fields.get(5)?.parse().ok()?;
        if tpgid > 0 { Some(tpgid as u32) } else { None }
    }

    /// Get child PIDs of a process by reading `/proc/<pid>/task/<tid>/children`.
    ///
    /// Falls back to an empty list if the file is not readable (e.g. on
    /// kernels without `CONFIG_PROC_CHILDREN`).
    #[allow(dead_code)]
    pub fn get_child_pids(pid: u32) -> Vec<u32> {
        let path = format!("/proc/{}/task/{}/children", pid, pid);
        match std::fs::read_to_string(&path) {
            Ok(content) => content
                .split_whitespace()
                .filter_map(|s| s.parse::<u32>().ok())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Read the full command line of a process from `/proc/<pid>/cmdline`.
    ///
    /// The kernel separates arguments with NUL bytes; we replace them with
    /// spaces for readability.
    #[allow(dead_code)]
    pub fn get_cmdline(pid: u32) -> Option<String> {
        let raw = std::fs::read(format!("/proc/{}/cmdline", pid)).ok()?;
        if raw.is_empty() {
            return None;
        }
        // Replace NUL separators with spaces and trim trailing NUL.
        let s: String = raw
            .iter()
            .map(|&b| if b == 0 { ' ' } else { b as char })
            .collect();
        Some(s.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_process_info_returns_some_for_own_process() {
        let pid = std::process::id();
        let info = ProcessTracker::get_process_info(pid);
        assert!(info.is_some(), "should be able to read own process info");
        let info = info.unwrap();
        assert_eq!(info.pid, pid);
        assert!(!info.name.is_empty(), "process name should not be empty");
        assert!(!info.cwd.is_empty(), "cwd should not be empty");
    }

    #[test]
    fn test_get_process_info_returns_none_for_nonexistent_pid() {
        // PID 4294967295 (u32::MAX) is extremely unlikely to exist.
        let info = ProcessTracker::get_process_info(u32::MAX);
        assert!(info.is_none());
    }

    #[test]
    fn test_get_foreground_process_parses_stat() {
        let pid = std::process::id();
        // This may or may not return Some depending on the terminal, but
        // it must not panic.
        let _result = ProcessTracker::get_foreground_process(pid);
        // At minimum, verify we can read our own stat file.
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid));
        assert!(stat.is_ok(), "should be able to read own /proc/pid/stat");
    }
}
