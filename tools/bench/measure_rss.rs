use std::env;
use std::process::exit;

fn main() {
    let pid_arg = env::args().nth(1);
    let pid = match pid_arg.as_deref() {
        Some(value) => match value.parse::<i32>() {
            Ok(pid) if pid > 0 => Some(pid),
            _ => {
                eprintln!("Invalid pid: {value}");
                exit(1);
            }
        },
        None => None,
    };

    #[cfg(target_os = "macos")]
    {
        use libc::{
            getrusage, proc_pid_rusage, rusage, rusage_info_v2, RUSAGE_INFO_V2, RUSAGE_SELF,
        };

        if let Some(pid) = pid {
            unsafe {
                let mut info: rusage_info_v2 = std::mem::zeroed();
                let result = proc_pid_rusage(
                    pid as libc::pid_t,
                    RUSAGE_INFO_V2 as i32,
                    &mut info as *mut _ as *mut _,
                );
                if result != 0 {
                    eprintln!("Failed to read RSS for pid {pid} via proc_pid_rusage");
                    exit(1);
                }
                println!("rss_bytes={}", info.ri_resident_size);
            }
        } else {
            unsafe {
                let mut usage: rusage = std::mem::zeroed();
                if getrusage(RUSAGE_SELF, &mut usage) != 0 {
                    eprintln!("Failed to read RSS for current process via getrusage");
                    exit(1);
                }
                println!("rss_bytes={}", usage.ru_maxrss);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let status_path = match pid {
            Some(pid) => format!("/proc/{pid}/status"),
            None => "/proc/self/status".to_string(),
        };
        let status = std::fs::read_to_string(&status_path).unwrap_or_else(|err| {
            eprintln!("Failed to read {status_path}: {err}");
            exit(1);
        });
        let mut rss_kb = None;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let mut parts = line.split_whitespace();
                let _label = parts.next();
                rss_kb = parts.next().and_then(|value| value.parse::<u64>().ok());
                break;
            }
        }
        let rss_kb = rss_kb.unwrap_or_else(|| {
            eprintln!("VmRSS not found in {status_path}");
            exit(1);
        });
        println!("rss_bytes={}", rss_kb * 1024);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        eprintln!("Unsupported platform for RSS measurement");
        exit(1);
    }
}
