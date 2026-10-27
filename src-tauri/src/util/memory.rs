//! Best-effort return of freed heap pages to the OS.
//!
//! Long extractions (images, PDFs, Office documents) can leave large freed
//! allocations in the process allocator. Calling this after each unit of work
//! keeps the daemon's resident set close to its baseline. It is always a no-op
//! on unsupported targets and never fails.

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn release_memory() {
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(target_os = "macos")]
pub fn release_memory() {
    // `malloc_zone_pressure_relief` is declared in <malloc/malloc.h>, not in
    // the libc crate's macOS bindings. Declare the two symbols directly so the
    // universal macOS build links without a C shim.
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut core::ffi::c_void, goal: usize) -> usize;
        fn malloc_default_zone() -> *mut core::ffi::c_void;
    }
    unsafe {
        let _ = malloc_zone_pressure_relief(malloc_default_zone(), 0);
    }
}

#[cfg(windows)]
pub fn release_memory() {
    // `EmptyWorkingSet` trims the process working set; `GetCurrentProcess`
    // returns a pseudo-handle that does not need closing.
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn EmptyWorkingSet(h_process: *mut core::ffi::c_void) -> i32;
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
    }
    unsafe {
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

#[cfg(not(any(
    all(target_os = "linux", target_env = "gnu"),
    target_os = "macos",
    windows
)))]
pub fn release_memory() {}

#[cfg(test)]
mod tests {
    #[test]
    fn release_memory_is_callable() {
        super::release_memory();
        super::release_memory();
    }
}

/// Terminates the WebKit/WebView helper processes that survive the destruction
/// of the search window, so the hidden app keeps only the Rust side in memory.
///
/// These helpers are children of the main process. Killing them is safe once
/// the window (and its webview) has been destroyed; showing the window again
/// spawns fresh ones.
pub fn kill_webview_processes() {
    let pid = std::process::id();

    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let pid_str = name.to_string_lossy();
                if !pid_str.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let Ok(child_pid) = pid_str.parse::<i32>() else {
                    continue;
                };
                if child_pid <= 0 || child_pid == pid as i32 {
                    continue;
                }
                let stat = std::fs::read_to_string(format!("/proc/{}/stat", child_pid)).unwrap_or_default();
                let Some(idx) = stat.rfind(')') else { continue };
                let rest = &stat[idx + 1..];
                let fields: Vec<&str> = rest.split_whitespace().collect();
                // fields[0] = state, fields[1] = ppid
                if fields.len() < 2 || fields[1] != pid.to_string() {
                    continue;
                }
                let comm = std::fs::read_to_string(format!("/proc/{}/comm", child_pid))
                    .unwrap_or_default();
                let comm = comm.trim();
                if comm.contains("WebKit") || comm.contains("webkit") {
                    unsafe {
                        libc::kill(child_pid, libc::SIGKILL);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("pkill")
            .args(["-P", &pid.to_string(), "WebKit"])
            .status();
    }

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args([
                "/F",
                "/FI",
                &format!("PPID eq {}", pid),
                "/FI",
                "IMAGENAME eq msedgewebview2.exe",
            ])
            .status();
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = pid;
    }
}
