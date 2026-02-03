fn main() {
    #[cfg(target_os = "macos")]
    {
        use libc::{getrusage, rusage, RUSAGE_SELF};
        unsafe {
            let mut usage: rusage = std::mem::zeroed();
            if getrusage(RUSAGE_SELF, &mut usage) == 0 {
                println!("max_rss_kb={}", usage.ru_maxrss);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                println!("{}", line);
            }
        }
    }
}
