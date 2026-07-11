use std::path::Path;

#[cfg(windows)]
mod imp {
    use std::{
        ffi::OsString,
        mem::size_of,
        os::windows::ffi::OsStringExt,
        path::{Path, PathBuf},
        process::Command,
        thread,
        time::{Duration, Instant},
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    use windows_sys::Win32::{
        Foundation::ERROR_INSUFFICIENT_BUFFER,
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
        },
        Networking::WinSock::AF_INET,
        System::{Com::CoTaskMemFree, SystemInformation::GetSystemDirectoryW},
        UI::Shell::{FOLDERID_ProgramData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };

    fn powershell_escape(value: &str) -> String {
        value.replace('\'', "''")
    }

    fn powershell_path() -> Result<PathBuf, String> {
        let mut buffer = vec![0_u16; 32_768];
        // SAFETY: buffer is writable for the exact capacity passed to the API.
        let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 || length as usize >= buffer.len() {
            return Err("failed to resolve the trusted Windows system directory".to_string());
        }
        buffer.truncate(length as usize);
        Ok(PathBuf::from(OsString::from_wide(&buffer))
            .join(r"WindowsPowerShell\v1.0\powershell.exe"))
    }

    fn command_succeeds_with_timeout(command: &mut Command, timeout: Duration) -> bool {
        let Ok(mut child) = command.spawn() else {
            return false;
        };
        let deadline = Instant::now() + timeout;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                Ok(None) | Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
        }
    }

    pub fn program_data_dir() -> Result<PathBuf, String> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: SHGetKnownFolderPath initializes raw on success. The returned
        // allocation belongs to the COM task allocator and is released below.
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
                "failed to resolve the Windows ProgramData known folder (HRESULT 0x{:08x})",
                result as u32
            ));
        }
        let mut length = 0_usize;
        // SAFETY: a successful call returns a NUL-terminated UTF-16 string.
        unsafe {
            while *raw.add(length) != 0 {
                length += 1;
            }
        }
        // SAFETY: raw points to length initialized UTF-16 code units.
        let path = PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(raw, length)
        }));
        // SAFETY: raw was allocated by SHGetKnownFolderPath.
        unsafe { CoTaskMemFree(raw.cast()) };
        if !path.is_absolute() {
            return Err("Windows ProgramData known folder is not absolute".to_string());
        }
        Ok(path.join("FnKnock"))
    }

    fn encode_powershell(script: &str) -> String {
        let bytes = script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        STANDARD.encode(bytes)
    }

    fn run_elevated_script(_label: &str, script: &str) -> Result<(), String> {
        // Never stage privileged PowerShell in the user-writable temp directory:
        // another process running as that user could replace it while UAC is open.
        // -EncodedCommand is UTF-16LE base64, so it is also safe from path quoting
        // and ArgumentList tokenisation issues.
        let guarded = format!(
            "$ErrorActionPreference = 'Stop'\ntry {{\n{script}\nexit 0\n}} catch {{\nWrite-Error $_\nexit 1\n}}\n"
        );
        let encoded = encode_powershell(&guarded);
        let powershell = powershell_path()?;
        let elevated_powershell = powershell_escape(&powershell.display().to_string());
        let launcher = format!(
            "$ErrorActionPreference='Stop'; try {{ $process = Start-Process -FilePath '{elevated_powershell}' -Verb RunAs -Wait -PassThru -ArgumentList @('-NoProfile','-NonInteractive','-EncodedCommand','{encoded}'); exit $process.ExitCode }} catch {{ Write-Error $_; exit 1223 }}"
        );
        let status = Command::new(&powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &launcher])
            .status()
            .map_err(|error| format!("failed to request administrator access: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "administrator operation was cancelled or failed ({status})"
            ))
        }
    }

    pub fn service_state(name: &str) -> String {
        let manager =
            match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
                Ok(manager) => manager,
                Err(error) => return format!("SCM 不可用：{error}"),
            };
        let service = match manager.open_service(name, ServiceAccess::QUERY_STATUS) {
            Ok(service) => service,
            Err(_) => return "未安装".to_string(),
        };
        match service.query_status().map(|status| status.current_state) {
            Ok(ServiceState::Stopped) => "已停止".to_string(),
            Ok(ServiceState::StartPending) => "正在启动".to_string(),
            Ok(ServiceState::StopPending) => "正在停止".to_string(),
            Ok(ServiceState::Running) => "运行中".to_string(),
            Ok(ServiceState::ContinuePending) => "正在继续".to_string(),
            Ok(ServiceState::PausePending) => "正在暂停".to_string(),
            Ok(ServiceState::Paused) => "已暂停".to_string(),
            Err(error) => format!("查询失败：{error}"),
        }
    }

    fn tcp_listener_owner_pid(port: u16) -> Result<Option<u32>, String> {
        let mut byte_count = 0_u32;
        // SAFETY: the first call intentionally supplies a null table to obtain
        // the required buffer size, per GetExtendedTcpTable's contract.
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
            return Err(format!(
                "failed to size the Windows TCP listener table ({probe})"
            ));
        }
        // A u32 backing allocation provides the alignment required by the MIB
        // table (which contains only u32 fields).
        let word_count = (byte_count as usize).div_ceil(size_of::<u32>());
        let mut table = vec![0_u32; word_count];
        // SAFETY: table is writable for byte_count bytes and correctly aligned.
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
            return Err(format!(
                "failed to query the Windows TCP listener table ({result})"
            ));
        }
        let entry_count = table[0] as usize;
        let required = size_of::<u32>()
            .checked_add(
                entry_count
                    .checked_mul(size_of::<MIB_TCPROW_OWNER_PID>())
                    .ok_or_else(|| "Windows TCP listener table is too large".to_string())?,
            )
            .ok_or_else(|| "Windows TCP listener table is too large".to_string())?;
        if required > byte_count as usize || required > table.len() * size_of::<u32>() {
            return Err("Windows TCP listener table is truncated".to_string());
        }
        // SAFETY: the bounds check above covers every fixed-size row following
        // the leading entry count, and the u32 allocation satisfies alignment.
        let rows = unsafe {
            std::slice::from_raw_parts(
                table.as_ptr().cast::<u8>().add(size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID,
                entry_count,
            )
        };
        let loopback = u32::from_ne_bytes([127, 0, 0, 1]);
        Ok(rows
            .iter()
            .find(|row| row.dwLocalAddr == loopback && u16::from_be(row.dwLocalPort as u16) == port)
            .map(|row| row.dwOwningPid))
    }

    pub fn verify_service_listener(name: &str, port: u16) -> Result<(), String> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .map_err(|error| format!("SCM 不可用：{error}"))?;
        let service = manager
            .open_service(name, ServiceAccess::QUERY_STATUS)
            .map_err(|error| format!("FnKnock 服务未安装：{error}"))?;
        let status = service
            .query_status()
            .map_err(|error| format!("无法查询 FnKnock 服务：{error}"))?;
        if status.current_state != ServiceState::Running {
            return Err("FnKnock 服务尚未运行".to_string());
        }
        let service_pid = status
            .process_id
            .filter(|pid| *pid != 0)
            .ok_or_else(|| "SCM 未返回 FnKnock 服务进程".to_string())?;
        let listener_pid = tcp_listener_owner_pid(port)?
            .ok_or_else(|| format!("管理端口 {port} 没有 TCP 监听进程"))?;
        if listener_pid != service_pid {
            return Err(format!("管理端口 {port} 不属于正在运行的 FnKnock 服务"));
        }
        Ok(())
    }

    pub fn start_service() -> Result<(), String> {
        run_elevated_script(
            "start-service",
            "$ErrorActionPreference = 'Stop'\nStart-Service -Name 'FnKnock'\n",
        )
    }

    pub fn restart_service() -> Result<(), String> {
        run_elevated_script(
            "restart-service",
            "$ErrorActionPreference = 'Stop'\nRestart-Service -Name 'FnKnock' -Force\n",
        )
    }

    pub fn write_runtime_config_and_restart(_path: &Path, bytes: &[u8]) -> Result<(), String> {
        let contents = STANDARD.encode(bytes);
        let script = format!(
            "$programData = [Environment]::GetFolderPath([Environment+SpecialFolder]::CommonApplicationData)\nif ([string]::IsNullOrWhiteSpace($programData) -or -not [IO.Path]::IsPathRooted($programData) -or $programData.StartsWith('\\\\')) {{ throw 'Windows ProgramData known folder is invalid' }}\n$root = Join-Path $programData 'FnKnock'\n$parent = Join-Path $root 'config'\n$target = Join-Path $parent 'runtime.json'\nforeach ($required in @($programData, $root, $parent)) {{ $item = Get-Item -LiteralPath $required -Force -ErrorAction Stop; if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {{ throw ('refusing reparse-point runtime path: ' + $required) }} }}\nif (Test-Path -LiteralPath $target) {{ $targetItem = Get-Item -LiteralPath $target -Force; if (($targetItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {{ throw 'refusing reparse-point runtime configuration' }} }}\n$temp = Join-Path $parent ('runtime-' + [guid]::NewGuid().ToString('N') + '.tmp')\ntry {{ [System.IO.File]::WriteAllBytes($temp, [Convert]::FromBase64String('{}')); if (Test-Path -LiteralPath $target) {{ [System.IO.File]::Replace($temp, $target, $null) }} else {{ [System.IO.File]::Move($temp, $target) }} }} finally {{ if (Test-Path -LiteralPath $temp) {{ Remove-Item -LiteralPath $temp -Force }} }}\nRestart-Service -Name 'FnKnock' -Force\n",
            contents,
        );
        run_elevated_script("save-runtime", &script)
    }

    pub fn firewall_rule_enabled() -> bool {
        let Ok(powershell) = powershell_path() else {
            return false;
        };
        let mut command = Command::new(powershell);
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$rule = Get-NetFirewallRule -DisplayName 'FnKnock Gateway' -ErrorAction SilentlyContinue | Where-Object Enabled -eq 'True'; if ($rule) { exit 0 } else { exit 1 }",
        ]);
        command_succeeds_with_timeout(&mut command, Duration::from_secs(2))
    }
}

#[cfg(not(windows))]
mod imp {
    use std::{fs, path::Path};

    pub fn program_data_dir() -> Result<std::path::PathBuf, String> {
        Ok(std::env::temp_dir().join("FnKnock"))
    }

    pub fn service_state(_name: &str) -> String {
        "仅 Windows 可用".to_string()
    }

    pub fn verify_service_listener(_name: &str, _port: u16) -> Result<(), String> {
        Ok(())
    }

    pub fn start_service() -> Result<(), String> {
        Err("FnKnock SCM service is only available on Windows".to_string())
    }

    pub fn restart_service() -> Result<(), String> {
        Err("FnKnock SCM service is only available on Windows".to_string())
    }

    pub fn write_runtime_config_and_restart(path: &Path, bytes: &[u8]) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let staging = path.with_extension("json.tmp");
        fs::write(&staging, bytes)
            .map_err(|error| format!("failed to write {}: {error}", staging.display()))?;
        fs::rename(&staging, path)
            .map_err(|error| format!("failed to replace {}: {error}", path.display()))
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

pub fn write_runtime_config_and_restart(path: &Path, bytes: &[u8]) -> Result<(), String> {
    imp::write_runtime_config_and_restart(path, bytes)
}

pub fn firewall_rule_enabled() -> bool {
    imp::firewall_rule_enabled()
}
