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
    unsafe {
        libc::malloc_zone_pressure_relief(libc::malloc_default_zone(), 0);
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
