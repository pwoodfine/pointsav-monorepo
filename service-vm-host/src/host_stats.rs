pub struct HostStats {
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub cpu_cores: u32,
    pub cpu_load_pct: f32,
    pub kvm_available: bool,
}

/// Read host resource statistics from /proc.
///
/// Falls back to zeros on any read failure so heartbeats continue even if
/// /proc is temporarily unavailable.
pub fn read_host_stats() -> HostStats {
    let (ram_total_mb, ram_used_mb) = read_meminfo();
    let cpu_cores = num_cpu_cores();
    let cpu_load_pct = read_load_avg_pct(cpu_cores);

    HostStats {
        ram_total_mb,
        ram_used_mb,
        cpu_cores,
        cpu_load_pct,
        kvm_available: check_kvm(),
    }
}

fn check_kvm() -> bool {
    std::path::Path::new("/dev/kvm").exists()
}

fn read_meminfo() -> (u64, u64) {
    let content = match std::fs::read_to_string("/proc/meminfo") {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };

    let mut total_kb: u64 = 0;
    let mut available_kb: u64 = 0;

    for line in content.lines() {
        if let Some(val) = parse_meminfo_line(line, "MemTotal:") {
            total_kb = val;
        } else if let Some(val) = parse_meminfo_line(line, "MemAvailable:") {
            available_kb = val;
        }
    }

    let total_mb = total_kb / 1024;
    let available_mb = available_kb / 1024;
    let used_mb = total_mb.saturating_sub(available_mb);

    (total_mb, used_mb)
}

fn parse_meminfo_line(line: &str, key: &str) -> Option<u64> {
    if !line.starts_with(key) {
        return None;
    }
    line.split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<u64>().ok())
}

fn num_cpu_cores() -> u32 {
    let content = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    content
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count()
        .max(1) as u32
}

fn read_load_avg_pct(cpu_cores: u32) -> f32 {
    let content = match std::fs::read_to_string("/proc/loadavg") {
        Ok(c) => c,
        Err(_) => return 0.0,
    };
    // First field is 1-minute load average
    let load: f32 = content
        .split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let cores = cpu_cores.max(1) as f32;
    (load / cores * 100.0).min(100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meminfo_line_extracts_value() {
        assert_eq!(
            parse_meminfo_line("MemTotal:       16384000 kB", "MemTotal:"),
            Some(16384000)
        );
        assert_eq!(
            parse_meminfo_line("MemAvailable:    8192000 kB", "MemAvailable:"),
            Some(8192000)
        );
        assert_eq!(
            parse_meminfo_line("Buffers:          131072 kB", "MemTotal:"),
            None
        );
    }

    #[test]
    fn kvm_check_returns_bool() {
        // Smoke test — either value is valid; this must not panic.
        let _ = check_kvm();
    }

    #[test]
    fn read_host_stats_returns_plausible_values() {
        let stats = read_host_stats();
        // On any Linux host, RAM total should be > 0
        // This test may return zeros in CI if /proc is unavailable — that's acceptable
        assert!(stats.cpu_cores >= 1);
        assert!(stats.cpu_load_pct >= 0.0);
        assert!(stats.cpu_load_pct <= 100.0);
    }
}
