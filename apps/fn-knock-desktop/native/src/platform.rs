use std::path::Path;

#[cfg(windows)]
mod imp {
    use std::{
        ffi::{OsStr, OsString},
        fs,
        mem::size_of,
        os::windows::{ffi::OsStringExt, process::CommandExt},
        path::{Path, PathBuf},
        process::Command,
        sync::OnceLock,
        thread,
        time::{Duration, Instant},
    };

    use windows_service::{
        service::{ServiceAccess, ServiceState, ServiceStatus},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NOT_ALL_ASSIGNED, GetLastError,
            INVALID_HANDLE_VALUE, SetLastError,
        },
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
        },
        Networking::WinSock::AF_INET,
        Security::{
            AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_DEBUG_NAME,
            SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
        System::{
            Com::CoTaskMemFree,
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{
                GetCurrentProcess, OpenProcess, OpenProcessToken,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
            },
        },
        UI::Shell::{FOLDERID_ProgramData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const SERVICE_NAME: &str = "FnKnock";

    fn enable_debug_privilege() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| unsafe {
            let mut token = std::ptr::null_mut();
            if OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut token,
            ) == 0
            {
                return false;
            }
            let mut luid = std::mem::zeroed();
            if LookupPrivilegeValueW(std::ptr::null(), SE_DEBUG_NAME, &mut luid) == 0 {
                CloseHandle(token);
                return false;
            }
            let privileges = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            SetLastError(0);
            let adjusted = AdjustTokenPrivileges(
                token,
                0,
                &privileges,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            let error = GetLastError();
            CloseHandle(token);
            adjusted != 0 && error != ERROR_NOT_ALL_ASSIGNED
        })
    }

    pub fn program_data_dir() -> Result<PathBuf, String> {
        let mut raw = std::ptr::null_mut();
        let result = unsafe {
            SHGetKnownFolderPath(
                &FOLDERID_ProgramData,
                KF_FLAG_DEFAULT as u32,
                std::ptr::null_mut(),
                &mut raw,
            )
        };
        if result < 0 || raw.is_null() {
            return Err(format!(
                "无法解析 Windows ProgramData（HRESULT 0x{:08x}）",
                result as u32
            ));
        }
        let mut length = 0;
        unsafe {
            while *raw.add(length) != 0 {
                length += 1;
            }
        }
        let path = PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(raw, length)
        }));
        unsafe { CoTaskMemFree(raw.cast()) };
        Ok(path.join("FnKnock"))
    }

    fn open_service(access: ServiceAccess) -> Result<windows_service::service::Service, String> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .map_err(|error| format!("无法连接 Windows 服务管理器：{error}"))?;
        manager
            .open_service(SERVICE_NAME, access)
            .map_err(|error| format!("无法打开 fn-knock 服务：{error}"))
    }

    fn wait_for_state(
        service: &windows_service::service::Service,
        target: ServiceState,
        timeout: Duration,
    ) -> Result<ServiceStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let status = service
                .query_status()
                .map_err(|error| format!("查询服务状态失败：{error}"))?;
            if status.current_state == target {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "等待服务状态 {target:?} 超时，当前为 {:?}",
                    status.current_state
                ));
            }
            thread::sleep(Duration::from_millis(150));
        }
    }

    pub fn service_state(name: &str) -> String {
        if name != SERVICE_NAME {
            return "未知服务".to_string();
        }
        match open_service(ServiceAccess::QUERY_STATUS)
            .and_then(|service| service.query_status().map_err(|e| e.to_string()))
        {
            Ok(status) => match status.current_state {
                ServiceState::Stopped => "已停止",
                ServiceState::StartPending => "正在启动",
                ServiceState::StopPending => "正在停止",
                ServiceState::Running => "运行中",
                ServiceState::ContinuePending => "正在继续",
                ServiceState::PausePending => "正在暂停",
                ServiceState::Paused => "已暂停",
            }
            .to_string(),
            Err(error) => format!("不可用：{error}"),
        }
    }

    pub fn service_is_running() -> bool {
        open_service(ServiceAccess::QUERY_STATUS)
            .and_then(|service| service.query_status().map_err(|error| error.to_string()))
            .is_ok_and(|status| status.current_state == ServiceState::Running)
    }

    pub fn service_is_stopped() -> bool {
        open_service(ServiceAccess::QUERY_STATUS)
            .and_then(|service| service.query_status().map_err(|error| error.to_string()))
            .is_ok_and(|status| status.current_state == ServiceState::Stopped)
    }

    fn working_set_bytes(process_id: u32) -> Result<u64, String> {
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                0,
                process_id,
            )
        };
        if process.is_null() {
            return Err(format!("无法打开进程 {process_id} 读取内存信息"));
        }
        let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let result = unsafe {
            GetProcessMemoryInfo(
                process,
                &mut counters,
                size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
        };
        unsafe { CloseHandle(process) };
        if result == 0 {
            Err(format!("无法读取进程 {process_id} 的内存信息"))
        } else {
            Ok(counters.WorkingSetSize as u64)
        }
    }

    pub fn service_process_memory() -> Result<(u64, u64), String> {
        let _ = enable_debug_privilege();
        let service = open_service(ServiceAccess::QUERY_STATUS)?;
        let status = service.query_status().map_err(|error| error.to_string())?;
        if status.current_state != ServiceState::Running {
            return Ok((0, 0));
        }
        let service_pid = status
            .process_id
            .filter(|pid| *pid != 0)
            .ok_or("SCM 未返回服务 PID")?;
        let service_bytes = working_set_bytes(service_pid)?;

        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("无法枚举 fn-knock 网关进程".to_string());
        }
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut gateway_bytes = 0_u64;
        let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
        while has_entry {
            let name_length = entry
                .szExeFile
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..name_length]);
            if entry.th32ParentProcessID == service_pid
                && name.eq_ignore_ascii_case("fn-knock-gateway.exe")
            {
                if let Ok(bytes) = working_set_bytes(entry.th32ProcessID) {
                    gateway_bytes = gateway_bytes.saturating_add(bytes);
                }
            }
            has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        }
        unsafe { CloseHandle(snapshot) };
        if gateway_bytes == 0 {
            Err("fn-knock 网关进程尚未提供内存数据".to_string())
        } else {
            Ok((service_bytes, gateway_bytes))
        }
    }

    pub fn start_service() -> Result<(), String> {
        let service = open_service(ServiceAccess::START | ServiceAccess::QUERY_STATUS)?;
        let status = service.query_status().map_err(|error| error.to_string())?;
        if status.current_state == ServiceState::Running {
            return Ok(());
        }
        service
            .start::<&OsStr>(&[])
            .map_err(|error| format!("启动服务失败：{error}"))?;
        wait_for_state(&service, ServiceState::Running, Duration::from_secs(25))?;
        Ok(())
    }

    pub fn stop_service() -> Result<(), String> {
        let service = open_service(ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)?;
        let status = service.query_status().map_err(|error| error.to_string())?;
        if status.current_state == ServiceState::Stopped {
            return Ok(());
        }
        service
            .stop()
            .map_err(|error| format!("停止服务失败：{error}"))?;
        wait_for_state(&service, ServiceState::Stopped, Duration::from_secs(25))?;
        Ok(())
    }

    pub fn restart_service() -> Result<(), String> {
        stop_service()?;
        start_service()
    }

    pub fn write_runtime_config_and_restart(path: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = path.parent().ok_or("运行配置路径缺少父目录")?;
        fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败：{error}"))?;
        let previous = fs::read(path).ok();
        let desired_port = serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()
            .and_then(|value| value.get("admin_port").and_then(serde_json::Value::as_u64))
            .and_then(|value| u16::try_from(value).ok())
            .ok_or("运行配置缺少有效的管理端口")?;
        let previous_port = previous.as_deref().and_then(|bytes| {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .and_then(|value| value.get("admin_port").and_then(serde_json::Value::as_u64))
                .and_then(|value| u16::try_from(value).ok())
        });
        let staging = parent.join(format!(".runtime-{}.tmp", std::process::id()));
        let backup = parent.join(format!(".runtime-{}.rollback", std::process::id()));
        let mut staged =
            fs::File::create(&staging).map_err(|error| format!("创建临时配置失败：{error}"))?;
        use std::io::Write;
        staged
            .write_all(bytes)
            .and_then(|_| staged.sync_all())
            .map_err(|error| format!("写入临时配置失败：{error}"))?;
        drop(staged);
        let _ = fs::remove_file(&backup);
        if path.exists() {
            fs::rename(path, &backup).map_err(|error| format!("备份旧配置失败：{error}"))?;
        }
        if let Err(error) = fs::rename(&staging, path) {
            if backup.exists() {
                let _ = fs::rename(&backup, path);
            }
            return Err(format!("替换运行配置失败：{error}"));
        }

        let apply_result = restart_service().and_then(|_| {
            let deadline = Instant::now() + Duration::from_secs(25);
            loop {
                let (ready, detail) = crate::runtime::check_ready(desired_port);
                if ready {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(format!(
                        "新管理端口 {desired_port} 未能就绪：{}",
                        detail.unwrap_or_else(|| "readyz 超时".to_string())
                    ));
                }
                thread::sleep(Duration::from_millis(250));
            }
        });
        if let Err(apply_error) = apply_result {
            let _ = fs::remove_file(path);
            if backup.exists() {
                let _ = fs::rename(&backup, path);
            }
            let rollback_result = restart_service().and_then(|_| {
                if let Some(port) = previous_port {
                    let deadline = Instant::now() + Duration::from_secs(25);
                    while Instant::now() < deadline {
                        if crate::runtime::check_ready(port).0 {
                            return Ok(());
                        }
                        thread::sleep(Duration::from_millis(250));
                    }
                    return Err(format!("旧管理端口 {port} 回滚后未能就绪"));
                }
                Ok(())
            });
            return match rollback_result {
                Ok(()) => Err(format!("{apply_error}；已恢复旧配置")),
                Err(rollback_error) => Err(format!(
                    "{apply_error}；旧配置已恢复，但服务恢复失败：{rollback_error}"
                )),
            };
        }
        let _ = fs::remove_file(backup);
        Ok(())
    }

    pub fn reset_panel_password() -> Result<(), String> {
        stop_service()?;
        let service_exe = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .parent()
            .ok_or("管理程序路径缺少父目录")?
            .join("fn-knock-service.exe");
        let result = Command::new(&service_exe)
            .arg("reset-panel-password")
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|error| format!("无法执行密码清理：{error}"))?;
        let restart = start_service();
        if !result.success() {
            return Err(format!("密码清理失败：{result}"));
        }
        restart
    }

    fn tcp_listener_owner_pid(port: u16) -> Result<Option<u32>, String> {
        let mut byte_count = 0_u32;
        let probe = unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut byte_count,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if probe != ERROR_INSUFFICIENT_BUFFER || byte_count < size_of::<u32>() as u32 {
            return Err(format!("无法读取 TCP 监听表（{probe}）"));
        }
        let mut table = vec![0_u32; (byte_count as usize).div_ceil(size_of::<u32>())];
        let result = unsafe {
            GetExtendedTcpTable(
                table.as_mut_ptr().cast(),
                &mut byte_count,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if result != 0 {
            return Err(format!("无法读取 TCP 监听表（{result}）"));
        }
        let count = table[0] as usize;
        let rows = unsafe {
            std::slice::from_raw_parts(
                table.as_ptr().cast::<u8>().add(size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID,
                count,
            )
        };
        let loopback = u32::from_ne_bytes([127, 0, 0, 1]);
        Ok(rows
            .iter()
            .find(|row| row.dwLocalAddr == loopback && u16::from_be(row.dwLocalPort as u16) == port)
            .map(|row| row.dwOwningPid))
    }

    pub fn verify_service_listener(name: &str, port: u16) -> Result<(), String> {
        if name != SERVICE_NAME {
            return Err("服务标识不匹配".to_string());
        }
        let service = open_service(ServiceAccess::QUERY_STATUS)?;
        let status = service.query_status().map_err(|error| error.to_string())?;
        if status.current_state != ServiceState::Running {
            return Err("fn-knock 服务尚未运行".to_string());
        }
        let service_pid = status
            .process_id
            .filter(|pid| *pid != 0)
            .ok_or("SCM 未返回服务 PID")?;
        let listener_pid =
            tcp_listener_owner_pid(port)?.ok_or_else(|| format!("管理端口 {port} 尚未监听"))?;
        if listener_pid != service_pid {
            return Err(format!("管理端口 {port} 不属于 fn-knock 服务"));
        }
        Ok(())
    }

    pub fn firewall_rule_enabled() -> bool {
        false
    }
}

#[cfg(not(windows))]
mod imp {
    use std::{fs, path::Path};
    pub fn program_data_dir() -> Result<std::path::PathBuf, String> {
        Ok(std::env::temp_dir().join("FnKnock"))
    }
    pub fn service_state(_: &str) -> String {
        "仅 Windows 可用".to_string()
    }
    pub fn service_is_running() -> bool {
        false
    }
    pub fn service_is_stopped() -> bool {
        true
    }
    pub fn service_process_memory() -> Result<(u64, u64), String> {
        Ok((0, 0))
    }
    pub fn verify_service_listener(_: &str, _: u16) -> Result<(), String> {
        Ok(())
    }
    pub fn start_service() -> Result<(), String> {
        Err("仅 Windows 可用".to_string())
    }
    pub fn restart_service() -> Result<(), String> {
        Err("仅 Windows 可用".to_string())
    }
    pub fn stop_service() -> Result<(), String> {
        Err("仅 Windows 可用".to_string())
    }
    pub fn reset_panel_password() -> Result<(), String> {
        Err("仅 Windows 可用".to_string())
    }
    pub fn write_runtime_config_and_restart(path: &Path, bytes: &[u8]) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, bytes).map_err(|e| e.to_string())
    }
    pub fn firewall_rule_enabled() -> bool {
        false
    }
}

pub fn service_state(name: &str) -> String {
    imp::service_state(name)
}
pub fn program_data_dir() -> Result<std::path::PathBuf, String> {
    imp::program_data_dir()
}
pub fn verify_service_listener(name: &str, port: u16) -> Result<(), String> {
    imp::verify_service_listener(name, port)
}
pub fn start_service() -> Result<(), String> {
    imp::start_service()
}
pub fn restart_service() -> Result<(), String> {
    imp::restart_service()
}
pub fn stop_service() -> Result<(), String> {
    imp::stop_service()
}
pub fn service_is_running() -> bool {
    imp::service_is_running()
}
pub fn service_is_stopped() -> bool {
    imp::service_is_stopped()
}
pub fn service_process_memory() -> Result<(u64, u64), String> {
    imp::service_process_memory()
}
pub fn reset_panel_password() -> Result<(), String> {
    imp::reset_panel_password()
}
pub fn write_runtime_config_and_restart(path: &Path, bytes: &[u8]) -> Result<(), String> {
    imp::write_runtime_config_and_restart(path, bytes)
}
pub fn firewall_rule_enabled() -> bool {
    imp::firewall_rule_enabled()
}
