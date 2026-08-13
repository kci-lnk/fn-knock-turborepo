#[cfg(target_os = "linux")]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let Ok(content) = std::fs::read_to_string("/proc/meminfo") else {
        return (None, None);
    };
    let read_kib = |name: &str| {
        content
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
            .map(|value| value.saturating_mul(1024))
    };
    (read_kib("MemTotal:"), read_kib("MemAvailable:"))
}

pub(crate) fn effective_memory_bytes() -> (Option<u64>, Option<u64>) {
    let (host_total, host_available) = host_memory_bytes();
    #[cfg(target_os = "linux")]
    {
        if let Some((limit, cgroup_available)) = linux_cgroup_memory_bytes() {
            return (
                Some(host_total.map_or(limit, |value| value.min(limit))),
                Some(host_available.map_or(cgroup_available, |value| value.min(cgroup_available))),
            );
        }
    }
    (host_total, host_available)
}

#[cfg(target_os = "linux")]
fn linux_cgroup_memory_bytes() -> Option<(u64, u64)> {
    let cgroup = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    for layout in cgroup_memory_layouts(&cgroup, &mountinfo) {
        if let Some(sample) = read_cgroup_memory_layout(&layout) {
            return Some(sample);
        }
    }
    None
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, PartialEq, Eq)]
struct CgroupMemoryLayout {
    mount_point: std::path::PathBuf,
    leaf: std::path::PathBuf,
    limit_file: &'static str,
    usage_file: &'static str,
}

#[cfg(any(target_os = "linux", test))]
fn cgroup_memory_layouts(cgroup: &str, mountinfo: &str) -> Vec<CgroupMemoryLayout> {
    let groups = cgroup.lines().filter_map(|line| {
        let mut fields = line.splitn(3, ':');
        let _hierarchy = fields.next()?;
        let controllers = fields.next()?;
        let path = fields.next()?;
        Some((controllers, std::path::PathBuf::from(path)))
    });
    let groups = groups.collect::<Vec<_>>();
    let mut layouts = Vec::new();
    for line in mountinfo.lines() {
        let Some((mount_fields, fs_fields)) = line.split_once(" - ") else {
            continue;
        };
        let mount_fields = mount_fields.split_whitespace().collect::<Vec<_>>();
        let fs_fields = fs_fields.split_whitespace().collect::<Vec<_>>();
        if mount_fields.len() < 5 || fs_fields.len() < 3 {
            continue;
        }
        let fs_type = fs_fields[0];
        let is_v2 = fs_type == "cgroup2";
        let is_v1_memory = fs_type == "cgroup"
            && fs_fields[2]
                .split(',')
                .any(|controller| controller == "memory");
        if !is_v2 && !is_v1_memory {
            continue;
        }
        let group_path = groups.iter().find_map(|(controllers, path)| {
            if (is_v2 && controllers.is_empty())
                || (is_v1_memory
                    && controllers
                        .split(',')
                        .any(|controller| controller == "memory"))
            {
                Some(path)
            } else {
                None
            }
        });
        let Some(group_path) = group_path else {
            continue;
        };
        let mount_root = std::path::Path::new(mount_fields[3]);
        let mount_point = std::path::PathBuf::from(mount_fields[4]);
        let relative = group_path
            .strip_prefix(mount_root)
            .unwrap_or_else(|_| group_path.strip_prefix("/").unwrap_or(group_path));
        layouts.push(CgroupMemoryLayout {
            leaf: mount_point.join(relative),
            mount_point,
            limit_file: if is_v2 {
                "memory.max"
            } else {
                "memory.limit_in_bytes"
            },
            usage_file: if is_v2 {
                "memory.current"
            } else {
                "memory.usage_in_bytes"
            },
        });
    }
    layouts
}

#[cfg(target_os = "linux")]
fn read_cgroup_memory_layout(layout: &CgroupMemoryLayout) -> Option<(u64, u64)> {
    if !layout.leaf.starts_with(&layout.mount_point) {
        return None;
    }
    let mut directory = Some(layout.leaf.as_path());
    let mut effective_limit: Option<u64> = None;
    let mut effective_available: Option<u64> = None;
    while let Some(path) = directory {
        if !path.starts_with(&layout.mount_point) {
            break;
        }
        let limit = read_finite_cgroup_value(&path.join(layout.limit_file));
        let usage = read_cgroup_value(&path.join(layout.usage_file));
        if let Some(limit) = limit {
            effective_limit = Some(effective_limit.map_or(limit, |value| value.min(limit)));
            if let Some(usage) = usage {
                let available = limit.saturating_sub(usage);
                effective_available =
                    Some(effective_available.map_or(available, |value| value.min(available)));
            }
        }
        if path == layout.mount_point {
            break;
        }
        directory = path.parent();
    }
    let limit = effective_limit?;
    Some((limit, effective_available.unwrap_or(limit)))
}

#[cfg(target_os = "linux")]
fn read_cgroup_value(path: &std::path::Path) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
}

#[cfg(target_os = "linux")]
fn read_finite_cgroup_value(path: &std::path::Path) -> Option<u64> {
    read_cgroup_value(path).filter(|value| *value < (1_u64 << 60))
}

#[cfg(windows)]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: MEMORYSTATUSEX is a plain C data structure for which an
    // all-zero bit pattern is valid; dwLength is initialized immediately.
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    // SAFETY: status points to writable, correctly sized MEMORYSTATUSEX
    // storage whose required dwLength field has been initialized.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        (None, None)
    } else {
        (Some(status.ullTotalPhys), Some(status.ullAvailPhys))
    }
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let (Some(page_size), Some(total_pages)) = (
        positive_sysconf(libc::_SC_PAGESIZE),
        positive_sysconf(libc::_SC_PHYS_PAGES),
    ) else {
        return (None, None);
    };
    let total = total_pages.saturating_mul(page_size);
    // SAFETY: vm_statistics64_data_t is a C structure consisting of integer
    // counters; zero is a valid initialized value for every field.
    let mut statistics: libc::vm_statistics64_data_t = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: statistics and count are valid writable out-parameters, and the
    // cast exposes exactly the C integer storage expected by host_statistics64.
    let result = unsafe {
        libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            (&raw mut statistics).cast::<libc::integer_t>(),
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return (Some(total), None);
    }
    let free = u64::from(statistics.free_count);
    let inactive = u64::from(statistics.inactive_count);
    let speculative = u64::from(statistics.speculative_count);
    let available_pages = free.saturating_add(inactive).saturating_add(speculative);
    (
        Some(total),
        Some(available_pages.saturating_mul(page_size).min(total)),
    )
}

#[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows)))]
pub(crate) fn host_memory_bytes() -> (Option<u64>, Option<u64>) {
    let (Some(page_size), Some(total_pages)) = (
        positive_sysconf(libc::_SC_PAGESIZE),
        positive_sysconf(libc::_SC_PHYS_PAGES),
    ) else {
        return (None, None);
    };
    (Some(total_pages.saturating_mul(page_size)), None)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn positive_sysconf(name: libc::c_int) -> Option<u64> {
    // SAFETY: sysconf takes a constant selector and has no pointer arguments
    // or ownership requirements.
    let value = unsafe { libc::sysconf(name) };
    u64::try_from(value).ok().filter(|value| *value > 0)
}

#[cfg(unix)]
pub(crate) fn process_file_descriptor_limit() -> Option<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: limit is valid writable storage for getrlimit's output and the
    // resource selector is a libc-defined constant.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0 {
        // rlim_t is u64 on most supported Unix targets but c_ulong on some
        // 32-bit libc variants, so keep the checked conversion portable.
        #[allow(clippy::useless_conversion)]
        u64::try_from(limit.rlim_cur).ok()
    } else {
        None
    }
}

#[cfg(not(unix))]
pub(crate) fn process_file_descriptor_limit() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_nested_cgroup_v2_mount_path() {
        let layouts = cgroup_memory_layouts(
            "0::/user.slice/fn-knock.service\n",
            "29 23 0:26 / /sys/fs/cgroup rw,nosuid,nodev,noexec,relatime - cgroup2 cgroup rw\n",
        );
        assert_eq!(
            layouts,
            vec![CgroupMemoryLayout {
                mount_point: "/sys/fs/cgroup".into(),
                leaf: "/sys/fs/cgroup/user.slice/fn-knock.service".into(),
                limit_file: "memory.max",
                usage_file: "memory.current",
            }]
        );
    }

    #[test]
    fn maps_namespaced_cgroup_mount_roots_and_v1_memory_controller() {
        let v2 = cgroup_memory_layouts(
            "0::/docker/abc/workload\n",
            "29 23 0:26 /docker/abc /sys/fs/cgroup rw - cgroup2 cgroup rw\n",
        );
        assert_eq!(v2[0].leaf, std::path::Path::new("/sys/fs/cgroup/workload"));

        let v1 = cgroup_memory_layouts(
            "5:cpu,cpuacct:/docker/abc\n6:memory:/docker/abc\n",
            "31 23 0:28 / /sys/fs/cgroup/memory rw - cgroup cgroup rw,memory\n",
        );
        assert_eq!(
            v1,
            vec![CgroupMemoryLayout {
                mount_point: "/sys/fs/cgroup/memory".into(),
                leaf: "/sys/fs/cgroup/memory/docker/abc".into(),
                limit_file: "memory.limit_in_bytes",
                usage_file: "memory.usage_in_bytes",
            }]
        );
    }
}
