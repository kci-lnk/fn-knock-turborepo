#[cfg(not(windows))]
pub fn run() {
    eprintln!("fn-knock desktop controller is only available on Windows");
}

#[cfg(windows)]
mod windows_ui {
    use std::{
        ffi::c_void,
        mem::size_of,
        ptr,
        sync::{
            Mutex, OnceLock,
            atomic::{AtomicBool, AtomicIsize, Ordering},
        },
        thread,
    };

    use windows_sys::Win32::{
        Foundation::{
            ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
        },
        Graphics::Gdi::{
            COLOR_WINDOW, CreateFontW, DeleteObject, GetMonitorInfoW, GetSysColorBrush,
            MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, SetBkMode, SetTextColor,
            TRANSPARENT, UpdateWindow,
        },
        System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW},
        UI::{
            Controls::{
                ICC_LINK_CLASS, ICC_PROGRESS_CLASS, INITCOMMONCONTROLSEX, InitCommonControls,
                InitCommonControlsEx, NM_CLICK, NM_RETURN, NMHDR, SetWindowTheme,
            },
            HiDpi::{GetDpiForSystem, GetDpiForWindow},
            Input::KeyboardAndMouse::EnableWindow,
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
                Shell_NotifyIconW, ShellExecuteW,
            },
            WindowsAndMessaging::{
                AppendMenuW, BS_DEFPUSHBUTTON, BS_FLAT, BS_GROUPBOX, BS_PUSHBUTTON, CREATESTRUCTW,
                CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
                DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, GetCursorPos, GetDlgItem,
                GetMessageW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW,
                IDI_APPLICATION, KillTimer, LoadCursorW, LoadIconW, MB_ICONERROR,
                MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO, MF_SEPARATOR, MF_STRING, MSG,
                MessageBoxW, MoveWindow, PostMessageW, PostQuitMessage, RegisterClassW, SW_HIDE,
                SW_SHOW, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW,
                SetForegroundWindow, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow,
                TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TrackPopupMenu, TranslateMessage,
                WM_APP, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY,
                WM_DPICHANGED, WM_LBUTTONUP, WM_NOTIFY, WM_RBUTTONUP, WM_TIMER, WM_USER, WNDCLASSW,
                WS_BORDER, WS_CAPTION, WS_CHILD, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
                WS_TABSTOP, WS_VISIBLE,
            },
        },
    };

    use crate::{platform, runtime, update};

    const CLASS_NAME: &str = "FnKnockNativeController";
    const ABOUT_CLASS_NAME: &str = "FnKnockNativeAbout";
    const WINDOW_TITLE: &str = "Knock 敲门 · Windows 管理程序";
    const OFFICIAL_URL: &str = "https://www.fnknock.cn/";
    const GITHUB_URL: &str = "https://github.com/kci-lnk/fn-knock-turborepo";
    const TRAY_MESSAGE: u32 = WM_APP + 1;
    const OPERATION_COMPLETE_MESSAGE: u32 = WM_APP + 2;
    const PBM_SETMARQUEE: u32 = WM_USER + 10;
    const ID_STATUS_TITLE: i32 = 100;
    const ID_STATUS_DETAIL: i32 = 106;
    const ID_MEMORY_LABEL: i32 = 109;
    const ID_VERSION_LABEL: i32 = 107;
    const ID_TITLE_LABEL: i32 = 108;
    const ID_ADMIN_PORT: i32 = 101;
    const ID_PROXY_PORT: i32 = 102;
    const ID_BACKEND_PORT: i32 = 103;
    const ID_AUTH_PORT: i32 = 104;
    const ID_GRPC_PORT: i32 = 105;
    const ID_OPEN_ADMIN: i32 = 201;
    const ID_START: i32 = 202;
    const ID_RESTART: i32 = 203;
    const ID_SAVE: i32 = 204;
    const ID_RESET_PASSWORD: i32 = 205;
    const ID_REFRESH: i32 = 206;
    const ID_CHECK_UPDATE: i32 = 207;
    const ID_STOP: i32 = 208;
    const ID_PROGRESS: i32 = 209;
    const ID_OFFICIAL_LINK: i32 = 210;
    const ID_GITHUB_LINK: i32 = 211;
    const ID_ABOUT_CLOSE: i32 = 212;
    const ID_TRAY_OPEN: usize = 301;
    const ID_TRAY_ADMIN: usize = 302;
    const ID_TRAY_RESTART: usize = 303;
    const ID_TRAY_QUIT: usize = 304;
    const ID_TRAY_UPDATE: usize = 305;
    const ID_TRAY_START: usize = 306;
    const ID_TRAY_STOP: usize = 307;
    const ID_TRAY_ABOUT: usize = 308;
    const APP_ICON_RESOURCE_ID: usize = 1;
    const MEMORY_TIMER_ID: usize = 1;

    static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);
    static ABOUT_WINDOW: AtomicIsize = AtomicIsize::new(0);
    static UI_FONT: AtomicIsize = AtomicIsize::new(0);
    static TITLE_FONT: AtomicIsize = AtomicIsize::new(0);
    static STATUS_FONT: AtomicIsize = AtomicIsize::new(0);
    static STATUS_READY: AtomicBool = AtomicBool::new(false);
    static OPERATION_BUSY: AtomicBool = AtomicBool::new(false);
    static CONTROL_LAYOUTS: OnceLock<Mutex<Vec<ControlLayout>>> = OnceLock::new();

    #[derive(Clone, Copy)]
    struct ControlLayout {
        parent: isize,
        hwnd: isize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    struct OperationResult {
        result: Result<(), String>,
        success_message: &'static str,
    }

    fn layouts() -> &'static Mutex<Vec<ControlLayout>> {
        CONTROL_LAYOUTS.get_or_init(|| Mutex::new(Vec::new()))
    }

    fn scale(value: i32, dpi: u32) -> i32 {
        ((i64::from(value) * i64::from(dpi) + 48) / 96) as i32
    }

    fn centered_position(anchor: RECT, work: RECT, width: i32, height: i32) -> (i32, i32) {
        let x = anchor.left + ((anchor.right - anchor.left - width) / 2);
        let y = anchor.top + ((anchor.bottom - anchor.top - height) / 2);
        (
            x.clamp(work.left, (work.right - width).max(work.left)),
            y.clamp(work.top, (work.bottom - height).max(work.top)),
        )
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    unsafe fn create_font(height: i32, weight: i32) -> isize {
        let face = wide("Segoe UI");
        unsafe {
            CreateFontW(
                height,
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                1,
                0,
                0,
                5,
                0,
                face.as_ptr(),
            ) as isize
        }
    }

    unsafe fn initialize_fonts(dpi: u32) -> [isize; 3] {
        [
            UI_FONT.swap(
                unsafe { create_font(-scale(16, dpi), 400) },
                Ordering::AcqRel,
            ),
            TITLE_FONT.swap(
                unsafe { create_font(-scale(28, dpi), 600) },
                Ordering::AcqRel,
            ),
            STATUS_FONT.swap(
                unsafe { create_font(-scale(18, dpi), 600) },
                Ordering::AcqRel,
            ),
        ]
    }

    unsafe fn delete_fonts(fonts: [isize; 3]) {
        for handle in fonts {
            if handle != 0 {
                unsafe { DeleteObject(handle as *mut c_void) };
            }
        }
    }

    unsafe fn destroy_fonts() {
        unsafe {
            delete_fonts([
                UI_FONT.swap(0, Ordering::AcqRel),
                TITLE_FONT.swap(0, Ordering::AcqRel),
                STATUS_FONT.swap(0, Ordering::AcqRel),
            ])
        };
    }

    unsafe fn set_font(hwnd: HWND, font: isize) {
        if !hwnd.is_null() && font != 0 {
            unsafe { SendMessageW(hwnd, 0x0030, font as usize, 1) };
        }
    }

    unsafe fn set_text(hwnd: HWND, value: &str) {
        let value = wide(value);
        unsafe { SetWindowTextW(hwnd, value.as_ptr()) };
    }

    unsafe fn show_control(hwnd: HWND, id: i32, visible: bool) {
        unsafe {
            ShowWindow(
                GetDlgItem(hwnd, id),
                if visible { SW_SHOW } else { SW_HIDE },
            )
        };
    }

    unsafe fn move_action(hwnd: HWND, id: i32, x: i32, width: i32) {
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        unsafe {
            MoveWindow(
                GetDlgItem(hwnd, id),
                scale(x, dpi),
                scale(470, dpi),
                scale(width, dpi),
                scale(38, dpi),
                1,
            )
        };
    }

    unsafe fn layout_controls(hwnd: HWND, dpi: u32) {
        if let Ok(controls) = layouts().lock() {
            for control in controls.iter() {
                if control.parent != hwnd as isize {
                    continue;
                }
                unsafe {
                    MoveWindow(
                        control.hwnd as HWND,
                        scale(control.x, dpi),
                        scale(control.y, dpi),
                        scale(control.width, dpi),
                        scale(control.height, dpi),
                        1,
                    )
                };
                unsafe { set_font(control.hwnd as HWND, UI_FONT.load(Ordering::Acquire)) };
            }
        }
        unsafe {
            set_font(
                GetDlgItem(hwnd, ID_TITLE_LABEL),
                TITLE_FONT.load(Ordering::Acquire),
            );
            set_font(
                GetDlgItem(hwnd, ID_STATUS_TITLE),
                STATUS_FONT.load(Ordering::Acquire),
            );
        }
    }

    fn remove_control_layouts(parent: HWND) {
        if let Ok(mut controls) = layouts().lock() {
            controls.retain(|control| control.parent != parent as isize);
        }
    }

    unsafe fn center_window(hwnd: HWND, owner: HWND) {
        let mut window_rect: RECT = unsafe { std::mem::zeroed() };
        if unsafe { GetWindowRect(hwnd, &mut window_rect) } == 0 {
            return;
        }
        let monitor_target = if owner.is_null() { hwnd } else { owner };
        let monitor = unsafe { MonitorFromWindow(monitor_target, MONITOR_DEFAULTTONEAREST) };
        let mut monitor_info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };
        if unsafe { GetMonitorInfoW(monitor, &mut monitor_info) } == 0 {
            return;
        }
        let width = window_rect.right - window_rect.left;
        let height = window_rect.bottom - window_rect.top;
        let anchor = if owner.is_null() {
            monitor_info.rcWork
        } else {
            let mut owner_rect: RECT = unsafe { std::mem::zeroed() };
            if unsafe { GetWindowRect(owner, &mut owner_rect) } == 0 {
                monitor_info.rcWork
            } else {
                owner_rect
            }
        };
        let (x, y) = centered_position(anchor, monitor_info.rcWork, width, height);
        unsafe {
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE,
            )
        };
    }

    unsafe fn message(hwnd: HWND, text: &str, error: bool) {
        let text = wide(text);
        let title = wide(if error {
            "fn-knock 操作失败"
        } else {
            "Knock 敲门"
        });
        unsafe {
            MessageBoxW(
                hwnd,
                text.as_ptr(),
                title.as_ptr(),
                MB_OK
                    | if error {
                        MB_ICONERROR
                    } else {
                        MB_ICONINFORMATION
                    },
            )
        };
    }

    unsafe fn set_busy(hwnd: HWND, busy: bool, label: &str) {
        for id in [
            ID_ADMIN_PORT,
            ID_PROXY_PORT,
            ID_BACKEND_PORT,
            ID_AUTH_PORT,
            ID_GRPC_PORT,
            ID_OPEN_ADMIN,
            ID_START,
            ID_STOP,
            ID_RESTART,
            ID_SAVE,
            ID_RESET_PASSWORD,
            ID_REFRESH,
            ID_CHECK_UPDATE,
        ] {
            unsafe { EnableWindow(GetDlgItem(hwnd, id), if busy { 0 } else { 1 }) };
        }
        let progress = unsafe { GetDlgItem(hwnd, ID_PROGRESS) };
        unsafe {
            ShowWindow(progress, if busy { SW_SHOW } else { SW_HIDE });
            SendMessageW(progress, PBM_SETMARQUEE, if busy { 1 } else { 0 }, 24);
        }
        if busy {
            STATUS_READY.store(false, Ordering::Release);
            unsafe { set_text(GetDlgItem(hwnd, ID_STATUS_TITLE), label) };
            unsafe {
                set_text(GetDlgItem(hwnd, ID_STATUS_DETAIL), "正在执行操作，请稍候…")
            };
        }
    }

    unsafe fn begin_operation<F>(
        hwnd: HWND,
        label: &str,
        success_message: &'static str,
        operation: F,
    ) where
        F: FnOnce() -> Result<(), String> + Send + 'static,
    {
        if OPERATION_BUSY.swap(true, Ordering::AcqRel) {
            return;
        }
        unsafe { set_busy(hwnd, true, label) };
        let window = hwnd as isize;
        thread::spawn(move || {
            let payload = Box::new(OperationResult {
                result: operation(),
                success_message,
            });
            let raw = Box::into_raw(payload);
            let posted = unsafe {
                PostMessageW(window as HWND, OPERATION_COMPLETE_MESSAGE, 0, raw as LPARAM)
            };
            if posted == 0 {
                unsafe { drop(Box::from_raw(raw)) };
                OPERATION_BUSY.store(false, Ordering::Release);
            }
        });
    }

    unsafe fn create_control(
        parent: HWND,
        class: &str,
        text: &str,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        id: i32,
    ) -> HWND {
        let use_explorer_theme =
            class == "EDIT" || (class == "BUTTON" && style & 0x0f != BS_GROUPBOX as u32);
        let dpi = unsafe { GetDpiForWindow(parent) }.max(96);
        let class = wide(class);
        let text = wide(text);
        let control = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                text.as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                scale(x, dpi),
                scale(y, dpi),
                scale(width, dpi),
                scale(height, dpi),
                parent,
                id as usize as HMENU,
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        if !control.is_null() {
            if let Ok(mut controls) = layouts().lock() {
                controls.push(ControlLayout {
                    parent: parent as isize,
                    hwnd: control as isize,
                    x,
                    y,
                    width,
                    height,
                });
            }
            unsafe { set_font(control, UI_FONT.load(Ordering::Acquire)) };
            if use_explorer_theme {
                let theme = wide("Explorer");
                unsafe { SetWindowTheme(control, theme.as_ptr(), ptr::null()) };
            }
        }
        control
    }

    unsafe fn refresh(hwnd: HWND) {
        let status = runtime::collect_status();
        STATUS_READY.store(status.ready, Ordering::Release);
        let status_title = if status.ready {
            "●  服务运行正常"
        } else {
            "●  服务需要处理"
        };
        let status_detail = format!(
            "Windows 服务：{}    管理后台：127.0.0.1:{}\r\n{}",
            status.service_state,
            status.config.admin_port,
            if status.ready {
                "网关、认证与管理组件均已就绪"
            } else {
                status.ready_detail.as_deref().unwrap_or("服务尚未就绪")
            }
        );
        unsafe { set_text(GetDlgItem(hwnd, ID_STATUS_TITLE), status_title) };
        unsafe { set_text(GetDlgItem(hwnd, ID_STATUS_DETAIL), &status_detail) };
        unsafe {
            set_text(
                GetDlgItem(hwnd, ID_VERSION_LABEL),
                &format!("fn-knock {} · Windows x86_64", status.version),
            )
        };
        unsafe {
            set_text(
                GetDlgItem(hwnd, ID_ADMIN_PORT),
                &status.config.admin_port.to_string(),
            )
        };
        unsafe {
            set_text(
                GetDlgItem(hwnd, ID_PROXY_PORT),
                &status.config.proxy_port.to_string(),
            )
        };
        unsafe {
            set_text(
                GetDlgItem(hwnd, ID_BACKEND_PORT),
                &status.config.backend_port.to_string(),
            )
        };
        unsafe {
            set_text(
                GetDlgItem(hwnd, ID_AUTH_PORT),
                &status.config.auth_port.to_string(),
            )
        };
        unsafe {
            set_text(
                GetDlgItem(hwnd, ID_GRPC_PORT),
                &status.config.grpc_port.to_string(),
            )
        };
        unsafe {
            show_control(hwnd, ID_OPEN_ADMIN, status.ready);
            show_control(hwnd, ID_START, status.service_stopped);
            show_control(hwnd, ID_STOP, status.service_running);
            show_control(hwnd, ID_RESTART, status.service_running);
            if status.service_running {
                move_action(hwnd, ID_OPEN_ADMIN, 24, 174);
                move_action(hwnd, ID_STOP, 210, 132);
                move_action(hwnd, ID_RESTART, 354, 132);
                move_action(hwnd, ID_REFRESH, 498, 132);
                set_text(GetDlgItem(hwnd, ID_SAVE), "保存并重启服务");
            } else if status.service_stopped {
                move_action(hwnd, ID_START, 24, 174);
                move_action(hwnd, ID_REFRESH, 210, 132);
                set_text(GetDlgItem(hwnd, ID_SAVE), "保存并启动服务");
            } else {
                show_control(hwnd, ID_START, false);
                move_action(hwnd, ID_REFRESH, 24, 174);
                set_text(GetDlgItem(hwnd, ID_SAVE), "保存端口设置");
            }
        }
        unsafe { refresh_memory(hwnd, status.service_running) };
    }

    unsafe fn refresh_memory(hwnd: HWND, service_running: bool) {
        let label = unsafe { GetDlgItem(hwnd, ID_MEMORY_LABEL) };
        if !service_running {
            unsafe { ShowWindow(label, SW_HIDE) };
            return;
        }
        match platform::service_process_memory() {
            Ok((service, gateway)) => {
                let mib = 1024.0 * 1024.0;
                unsafe {
                    set_text(
                        label,
                        &format!(
                            "内存占用：服务 {:.1} MB  ·  网关 {:.1} MB  ·  合计 {:.1} MB",
                            service as f64 / mib,
                            gateway as f64 / mib,
                            service.saturating_add(gateway) as f64 / mib,
                        ),
                    )
                };
                unsafe { ShowWindow(label, SW_SHOW) };
            }
            Err(_) => unsafe {
                ShowWindow(label, SW_HIDE);
            },
        }
    }

    unsafe fn read_port(hwnd: HWND, id: i32, label: &str) -> Result<u16, String> {
        let control = unsafe { GetDlgItem(hwnd, id) };
        let length = unsafe { GetWindowTextLengthW(control) };
        let mut buffer = vec![0_u16; length as usize + 1];
        unsafe { GetWindowTextW(control, buffer.as_mut_ptr(), buffer.len() as i32) };
        let value = String::from_utf16_lossy(&buffer[..length as usize]);
        value
            .parse::<u16>()
            .map_err(|_| format!("{label}必须是 1–65535 的整数"))
    }

    unsafe fn read_port_config(hwnd: HWND) -> Result<runtime::RuntimeConfig, String> {
        let mut config = runtime::load_public_runtime_config().unwrap_or_default();
        config.admin_port = unsafe { read_port(hwnd, ID_ADMIN_PORT, "管理端口")? };
        config.proxy_port = unsafe { read_port(hwnd, ID_PROXY_PORT, "代理端口")? };
        config.backend_port = unsafe { read_port(hwnd, ID_BACKEND_PORT, "Rust API 端口")? };
        config.auth_port = unsafe { read_port(hwnd, ID_AUTH_PORT, "认证端口")? };
        config.grpc_port = unsafe { read_port(hwnd, ID_GRPC_PORT, "Go gRPC 端口")? };
        config.onboarding_complete = true;
        config.validate()?;
        Ok(config)
    }

    fn open_url(value: &str) -> Result<(), String> {
        let url = wide(value);
        let verb = wide("open");
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                verb.as_ptr(),
                url.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        } as isize;
        if result <= 32 {
            Err(format!("无法打开链接（ShellExecute={result}）"))
        } else {
            Ok(())
        }
    }

    fn open_admin() -> Result<(), String> {
        let config = runtime::load_public_runtime_config()?;
        open_url(&format!("http://127.0.0.1:{}", config.admin_port))
    }

    unsafe fn handle_link_notification(hwnd: HWND, lparam: LPARAM) -> bool {
        if lparam == 0 {
            return false;
        }
        let header = unsafe { &*(lparam as *const NMHDR) };
        if header.code != NM_CLICK && header.code != NM_RETURN {
            return false;
        }
        let result = match header.idFrom as i32 {
            ID_OFFICIAL_LINK => open_url(OFFICIAL_URL),
            ID_GITHUB_LINK => open_url(GITHUB_URL),
            _ => return false,
        };
        if let Err(error) = result {
            unsafe { message(hwnd, &error, true) };
        }
        true
    }

    unsafe fn check_update(hwnd: HWND) -> Result<(), String> {
        let Some(offer) = update::check()? else {
            unsafe { message(hwnd, "当前已经是最新稳定版本。", false) };
            return Ok(());
        };
        let prompt = wide(&format!(
            "发现 fn-knock {}。\r\n\r\n{}\r\n\r\n现在下载并安装吗？",
            offer.version, offer.notes
        ));
        let title = wide("Knock 敲门 · 更新");
        if unsafe {
            MessageBoxW(
                hwnd,
                prompt.as_ptr(),
                title.as_ptr(),
                MB_YESNO | MB_ICONINFORMATION,
            )
        } == 6
        {
            update::install(&offer)?;
            unsafe {
                message(hwnd, "更新安装器已启动，管理程序将退出。", false);
                remove_tray(hwnd);
                DestroyWindow(hwnd);
            }
        }
        Ok(())
    }

    unsafe fn add_tray(hwnd: HWND) {
        let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        data.hIcon = unsafe {
            LoadIconW(
                GetModuleHandleW(ptr::null()),
                APP_ICON_RESOURCE_ID as *const u16,
            )
        };
        if data.hIcon.is_null() {
            data.hIcon = unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) };
        }
        let tip = wide(&format!(
            "Knock 敲门 · fn-knock {}",
            env!("CARGO_PKG_VERSION")
        ));
        let count = tip.len().min(data.szTip.len());
        data.szTip[..count].copy_from_slice(&tip[..count]);
        unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
    }

    unsafe fn remove_tray(hwnd: HWND) {
        let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
    }

    unsafe extern "system" fn about_window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => {
                unsafe {
                    let title = create_control(
                        hwnd,
                        "STATIC",
                        "Knock 敲门",
                        0,
                        28,
                        24,
                        390,
                        38,
                        ID_TITLE_LABEL,
                    );
                    set_font(title, TITLE_FONT.load(Ordering::Acquire));
                    create_control(
                        hwnd,
                        "STATIC",
                        &format!("fn-knock {} · Windows x86_64", env!("CARGO_PKG_VERSION")),
                        0,
                        30,
                        72,
                        390,
                        24,
                        0,
                    );
                    create_control(
                        hwnd,
                        "STATIC",
                        "本机网关服务与管理程序",
                        0,
                        30,
                        106,
                        390,
                        24,
                        0,
                    );
                    create_control(
                        hwnd,
                        "SysLink",
                        "<a href=\"https://www.fnknock.cn/\">官方网站  www.fnknock.cn</a>",
                        WS_TABSTOP | 0x0001,
                        30,
                        150,
                        390,
                        26,
                        ID_OFFICIAL_LINK,
                    );
                    create_control(
                        hwnd,
                        "SysLink",
                        "<a href=\"https://github.com/kci-lnk/fn-knock-turborepo\">GitHub 项目</a>",
                        WS_TABSTOP | 0x0001,
                        30,
                        184,
                        390,
                        26,
                        ID_GITHUB_LINK,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "关闭",
                        BS_DEFPUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        328,
                        227,
                        104,
                        34,
                        ID_ABOUT_CLOSE,
                    );
                }
                0
            }
            WM_COMMAND if (wparam & 0xffff) as i32 == ID_ABOUT_CLOSE => {
                unsafe { DestroyWindow(hwnd) };
                0
            }
            WM_NOTIFY => {
                unsafe { handle_link_notification(hwnd, lparam) };
                0
            }
            WM_CTLCOLORSTATIC => {
                let device_context = wparam as *mut c_void;
                unsafe { SetBkMode(device_context, TRANSPARENT as i32) };
                unsafe { GetSysColorBrush(COLOR_WINDOW) as LRESULT }
            }
            WM_DPICHANGED => {
                let dpi = (wparam & 0xffff) as u32;
                let suggested = unsafe { &*(lparam as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        ptr::null_mut(),
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    layout_controls(hwnd, dpi.max(96));
                }
                0
            }
            WM_CLOSE => {
                unsafe { DestroyWindow(hwnd) };
                0
            }
            WM_DESTROY => {
                ABOUT_WINDOW.store(0, Ordering::Release);
                remove_control_layouts(hwnd);
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    unsafe fn show_about(owner: HWND) {
        let existing = ABOUT_WINDOW.load(Ordering::Acquire) as HWND;
        if !existing.is_null() {
            unsafe {
                ShowWindow(existing, SW_SHOW);
                SetForegroundWindow(existing);
            }
            return;
        }
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        let class_name = wide(ABOUT_CLASS_NAME);
        let title = wide("关于 Knock 敲门");
        let dpi = unsafe { GetDpiForWindow(owner) }.max(96);
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                scale(470, dpi),
                scale(310, dpi),
                owner,
                ptr::null_mut(),
                instance,
                ptr::null(),
            )
        };
        if !hwnd.is_null() {
            ABOUT_WINDOW.store(hwnd as isize, Ordering::Release);
            unsafe {
                center_window(hwnd, owner);
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
            }
        }
    }

    unsafe fn tray_menu(hwnd: HWND) {
        let menu = unsafe { CreatePopupMenu() };
        let mut items = vec![(ID_TRAY_OPEN, "打开管理程序")];
        let running = platform::service_is_running();
        if running {
            items.push((ID_TRAY_ADMIN, "打开管理后台"));
        }
        if !OPERATION_BUSY.load(Ordering::Acquire) {
            if running {
                items.push((ID_TRAY_STOP, "停止服务"));
                items.push((ID_TRAY_RESTART, "重启服务"));
            } else if platform::service_is_stopped() {
                items.push((ID_TRAY_START, "启动服务"));
            }
        }
        items.push((ID_TRAY_UPDATE, "检查更新"));
        items.push((ID_TRAY_ABOUT, "关于"));
        for (id, label) in items {
            let label = wide(label);
            unsafe { AppendMenuW(menu, MF_STRING, id, label.as_ptr()) };
        }
        unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null()) };
        let version = wide(&format!("版本 {}", env!("CARGO_PKG_VERSION")));
        unsafe { AppendMenuW(menu, MF_STRING, 0, version.as_ptr()) };
        let quit = wide("退出管理程序");
        unsafe { AppendMenuW(menu, MF_STRING, ID_TRAY_QUIT, quit.as_ptr()) };
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut point);
            SetForegroundWindow(hwnd)
        };
        let command = unsafe {
            TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
                point.x,
                point.y,
                0,
                hwnd,
                ptr::null(),
            )
        };
        unsafe { DestroyMenu(menu) };
        match command as usize {
            ID_TRAY_OPEN => unsafe {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
            },
            ID_TRAY_ADMIN => {
                if let Err(error) = open_admin() {
                    unsafe { message(hwnd, &error, true) };
                }
            }
            ID_TRAY_RESTART => {
                unsafe {
                    begin_operation(
                        hwnd,
                        "正在重启服务…",
                        "fn-knock 服务已重新启动。",
                        platform::restart_service,
                    )
                };
            }
            ID_TRAY_START => {
                unsafe {
                    begin_operation(
                        hwnd,
                        "正在启动服务…",
                        "fn-knock 服务已启动。",
                        platform::start_service,
                    )
                };
            }
            ID_TRAY_STOP => {
                unsafe {
                    begin_operation(
                        hwnd,
                        "正在停止服务…",
                        "fn-knock 服务已停止。",
                        platform::stop_service,
                    )
                };
            }
            ID_TRAY_UPDATE => {
                if let Err(error) = unsafe { check_update(hwnd) } {
                    unsafe { message(hwnd, &error, true) };
                }
            }
            ID_TRAY_ABOUT => unsafe { show_about(hwnd) },
            ID_TRAY_QUIT => unsafe {
                DestroyWindow(hwnd);
            },
            _ => {}
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => {
                let _create = lparam as *const CREATESTRUCTW;
                unsafe {
                    let _ = initialize_fonts(GetDpiForWindow(hwnd).max(96));
                    let title = create_control(
                        hwnd,
                        "STATIC",
                        "Knock 敲门",
                        0,
                        28,
                        18,
                        430,
                        38,
                        ID_TITLE_LABEL,
                    );
                    set_font(title, TITLE_FONT.load(Ordering::Acquire));
                    create_control(
                        hwnd,
                        "STATIC",
                        "fn-knock 本机服务与端口管理",
                        0,
                        30,
                        58,
                        430,
                        22,
                        0,
                    );
                    create_control(hwnd, "STATIC", "", 0, 500, 30, 185, 22, ID_VERSION_LABEL);

                    create_control(
                        hwnd,
                        "BUTTON",
                        "运行状态",
                        BS_GROUPBOX as u32,
                        24,
                        92,
                        660,
                        150,
                        0,
                    );
                    let status_title = create_control(
                        hwnd,
                        "STATIC",
                        "正在读取状态…",
                        0,
                        48,
                        121,
                        560,
                        28,
                        ID_STATUS_TITLE,
                    );
                    set_font(status_title, STATUS_FONT.load(Ordering::Acquire));
                    create_control(hwnd, "STATIC", "", 0, 48, 157, 610, 44, ID_STATUS_DETAIL);
                    create_control(
                        hwnd,
                        "STATIC",
                        "内存占用：正在读取…",
                        0,
                        48,
                        207,
                        610,
                        22,
                        ID_MEMORY_LABEL,
                    );

                    create_control(
                        hwnd,
                        "BUTTON",
                        "端口设置",
                        BS_GROUPBOX as u32,
                        24,
                        254,
                        660,
                        194,
                        0,
                    );
                    create_control(hwnd, "STATIC", "管理后台", 0, 48, 287, 86, 22, 0);
                    create_control(
                        hwnd,
                        "EDIT",
                        "7991",
                        WS_BORDER | ES_AUTOHSCROLL as u32 | WS_TABSTOP,
                        138,
                        282,
                        112,
                        30,
                        ID_ADMIN_PORT,
                    );
                    create_control(hwnd, "STATIC", "代理入口", 0, 292, 287, 86, 22, 0);
                    create_control(
                        hwnd,
                        "EDIT",
                        "7999",
                        WS_BORDER | ES_AUTOHSCROLL as u32 | WS_TABSTOP,
                        382,
                        282,
                        112,
                        30,
                        ID_PROXY_PORT,
                    );
                    create_control(
                        hwnd,
                        "STATIC",
                        "高级端口（通常无需修改）",
                        0,
                        48,
                        334,
                        230,
                        22,
                        0,
                    );
                    create_control(hwnd, "STATIC", "Rust API", 0, 48, 369, 70, 22, 0);
                    create_control(
                        hwnd,
                        "EDIT",
                        "7998",
                        WS_BORDER | ES_AUTOHSCROLL as u32 | WS_TABSTOP,
                        120,
                        364,
                        74,
                        29,
                        ID_BACKEND_PORT,
                    );
                    create_control(hwnd, "STATIC", "认证", 0, 215, 369, 42, 22, 0);
                    create_control(
                        hwnd,
                        "EDIT",
                        "7997",
                        WS_BORDER | ES_AUTOHSCROLL as u32 | WS_TABSTOP,
                        260,
                        364,
                        74,
                        29,
                        ID_AUTH_PORT,
                    );
                    create_control(hwnd, "STATIC", "Go gRPC", 0, 356, 369, 65, 22, 0);
                    create_control(
                        hwnd,
                        "EDIT",
                        "7996",
                        WS_BORDER | ES_AUTOHSCROLL as u32 | WS_TABSTOP,
                        425,
                        364,
                        74,
                        29,
                        ID_GRPC_PORT,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "保存并重启服务",
                        BS_PUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        516,
                        361,
                        142,
                        34,
                        ID_SAVE,
                    );

                    create_control(
                        hwnd,
                        "BUTTON",
                        "打开管理后台",
                        BS_DEFPUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        24,
                        470,
                        174,
                        38,
                        ID_OPEN_ADMIN,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "启动服务",
                        BS_PUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        210,
                        470,
                        132,
                        38,
                        ID_START,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "停止服务",
                        BS_PUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        210,
                        470,
                        132,
                        38,
                        ID_STOP,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "重启服务",
                        BS_PUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        354,
                        470,
                        132,
                        38,
                        ID_RESTART,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "刷新状态",
                        BS_PUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        498,
                        470,
                        132,
                        38,
                        ID_REFRESH,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "清除管理密码",
                        BS_PUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        24,
                        525,
                        174,
                        36,
                        ID_RESET_PASSWORD,
                    );
                    create_control(
                        hwnd,
                        "BUTTON",
                        "检查更新",
                        BS_PUSHBUTTON as u32 | BS_FLAT as u32 | WS_TABSTOP,
                        210,
                        525,
                        132,
                        36,
                        ID_CHECK_UPDATE,
                    );
                    create_control(
                        hwnd,
                        "STATIC",
                        "关闭窗口后，fn-knock 将继续在系统托盘运行。",
                        0,
                        354,
                        533,
                        310,
                        22,
                        0,
                    );
                    create_control(
                        hwnd,
                        "SysLink",
                        "<a href=\"https://www.fnknock.cn/\">官方网站</a>",
                        WS_TABSTOP | 0x0001,
                        354,
                        565,
                        110,
                        24,
                        ID_OFFICIAL_LINK,
                    );
                    create_control(
                        hwnd,
                        "SysLink",
                        "<a href=\"https://github.com/kci-lnk/fn-knock-turborepo\">GitHub 项目</a>",
                        WS_TABSTOP | 0x0001,
                        480,
                        565,
                        150,
                        24,
                        ID_GITHUB_LINK,
                    );
                    let progress = create_control(
                        hwnd,
                        "msctls_progress32",
                        "",
                        0x08,
                        24,
                        615,
                        660,
                        8,
                        ID_PROGRESS,
                    );
                    ShowWindow(progress, SW_HIDE);
                    add_tray(hwnd);
                    refresh(hwnd);
                    SetTimer(hwnd, MEMORY_TIMER_ID, 10_000, None);
                }
                0
            }
            WM_COMMAND => {
                let id = (wparam & 0xffff) as i32;
                if OPERATION_BUSY.load(Ordering::Acquire) {
                    return 0;
                }
                match id {
                    ID_OPEN_ADMIN => {
                        if let Err(error) = open_admin() {
                            unsafe { message(hwnd, &error, true) };
                        }
                    }
                    ID_START => unsafe {
                        begin_operation(
                            hwnd,
                            "正在启动服务…",
                            "fn-knock 服务已启动。",
                            platform::start_service,
                        )
                    },
                    ID_STOP => unsafe {
                        begin_operation(
                            hwnd,
                            "正在停止服务…",
                            "fn-knock 服务已停止。",
                            platform::stop_service,
                        )
                    },
                    ID_RESTART => unsafe {
                        begin_operation(
                            hwnd,
                            "正在重启服务…",
                            "fn-knock 服务已重新启动。",
                            platform::restart_service,
                        )
                    },
                    ID_SAVE => match unsafe { read_port_config(hwnd) } {
                        Ok(config) => unsafe {
                            begin_operation(
                                hwnd,
                                "正在应用端口并验证服务…",
                                "端口配置已生效，服务已通过就绪检查。",
                                move || runtime::save_runtime_config(&config),
                            )
                        },
                        Err(error) => unsafe { message(hwnd, &error, true) },
                    },
                    ID_RESET_PASSWORD => {
                        let prompt = wide("将清除管理密码、登录会话与失败退避状态。确定继续吗？");
                        let title = wide("Knock 敲门");
                        if unsafe {
                            MessageBoxW(
                                hwnd,
                                prompt.as_ptr(),
                                title.as_ptr(),
                                MB_YESNO | MB_ICONWARNING,
                            )
                        } == 6
                        {
                            unsafe {
                                begin_operation(
                                    hwnd,
                                    "正在清除管理密码…",
                                    "管理密码与现有登录会话已清除，服务已恢复。",
                                    platform::reset_panel_password,
                                )
                            };
                        }
                    }
                    ID_REFRESH => unsafe { refresh(hwnd) },
                    ID_CHECK_UPDATE => {
                        if let Err(error) = unsafe { check_update(hwnd) } {
                            unsafe { message(hwnd, &error, true) };
                        }
                    }
                    value if value == ID_TRAY_ABOUT as i32 => unsafe { show_about(hwnd) },
                    _ => return 0,
                }
                0
            }
            WM_NOTIFY => {
                unsafe { handle_link_notification(hwnd, lparam) };
                0
            }
            WM_TIMER if wparam == MEMORY_TIMER_ID => {
                if !OPERATION_BUSY.load(Ordering::Acquire) && platform::service_is_running() {
                    unsafe { refresh_memory(hwnd, true) };
                }
                0
            }
            OPERATION_COMPLETE_MESSAGE => {
                let payload = unsafe { Box::from_raw(lparam as *mut OperationResult) };
                OPERATION_BUSY.store(false, Ordering::Release);
                unsafe {
                    set_busy(hwnd, false, "");
                    refresh(hwnd);
                }
                match payload.result {
                    Ok(()) => unsafe { message(hwnd, payload.success_message, false) },
                    Err(error) => unsafe { message(hwnd, &error, true) },
                }
                0
            }
            TRAY_MESSAGE => {
                match lparam as u32 {
                    WM_LBUTTONUP => unsafe {
                        ShowWindow(hwnd, SW_SHOW);
                        SetForegroundWindow(hwnd);
                    },
                    WM_RBUTTONUP => unsafe { tray_menu(hwnd) },
                    _ => {}
                }
                0
            }
            WM_CTLCOLORSTATIC => {
                let device_context = wparam as *mut c_void;
                unsafe { SetBkMode(device_context, TRANSPARENT as i32) };
                let control = lparam as HWND;
                if control == unsafe { GetDlgItem(hwnd, ID_STATUS_TITLE) } {
                    let color = if STATUS_READY.load(Ordering::Acquire) {
                        0x0041_781c
                    } else {
                        0x0032_32b4
                    };
                    unsafe { SetTextColor(device_context, color) };
                } else if control == unsafe { GetDlgItem(hwnd, ID_VERSION_LABEL) } {
                    unsafe { SetTextColor(device_context, 0x0064_6464) };
                }
                unsafe { GetSysColorBrush(COLOR_WINDOW) as LRESULT }
            }
            WM_DPICHANGED => {
                let dpi = (wparam & 0xffff) as u32;
                let suggested = unsafe { &*(lparam as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        ptr::null_mut(),
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    let old_fonts = initialize_fonts(dpi.max(96));
                    layout_controls(hwnd, dpi.max(96));
                    refresh(hwnd);
                    delete_fonts(old_fonts);
                }
                0
            }
            WM_CLOSE => {
                unsafe { ShowWindow(hwnd, SW_HIDE) };
                0
            }
            WM_DESTROY => {
                unsafe {
                    KillTimer(hwnd, MEMORY_TIMER_ID);
                    remove_tray(hwnd);
                    destroy_fonts();
                    PostQuitMessage(0)
                };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    pub fn run() {
        unsafe {
            InitCommonControls();
            let common_controls = INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_LINK_CLASS | ICC_PROGRESS_CLASS,
            };
            InitCommonControlsEx(&common_controls);
            let mutex_name = wide(if cfg!(debug_assertions) {
                "Local\\FnKnockNativeControllerDebug"
            } else {
                "Global\\FnKnockNativeController"
            });
            let _mutex = CreateMutexW(ptr::null(), 1, mutex_name.as_ptr());
            if GetLastError() == ERROR_ALREADY_EXISTS {
                return;
            }
            let instance = GetModuleHandleW(ptr::null());
            let class_name = wide(CLASS_NAME);
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hIcon: {
                    let icon = LoadIconW(instance, APP_ICON_RESOURCE_ID as *const u16);
                    if icon.is_null() {
                        LoadIconW(ptr::null_mut(), IDI_APPLICATION)
                    } else {
                        icon
                    }
                },
                hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                hbrBackground: (COLOR_WINDOW as usize + 1) as *mut c_void,
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&class) == 0 {
                return;
            }
            let about_class_name = wide(ABOUT_CLASS_NAME);
            let about_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(about_window_proc),
                hInstance: instance,
                hIcon: class.hIcon,
                hCursor: class.hCursor,
                hbrBackground: class.hbrBackground,
                lpszClassName: about_class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&about_class) == 0 {
                return;
            }
            let title = wide(WINDOW_TITLE);
            let dpi = GetDpiForSystem().max(96);
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                scale(728, dpi),
                scale(666, dpi),
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            MAIN_WINDOW.store(hwnd as isize, Ordering::Release);
            center_window(hwnd, ptr::null_mut());
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{centered_position, scale};
        use windows_sys::Win32::Foundation::RECT;

        #[test]
        fn logical_layout_scales_for_common_windows_dpi_values() {
            assert_eq!(scale(100, 96), 100);
            assert_eq!(scale(100, 120), 125);
            assert_eq!(scale(100, 144), 150);
            assert_eq!(scale(100, 192), 200);
        }

        #[test]
        fn windows_are_centered_and_clamped_to_the_work_area() {
            let work = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            };
            assert_eq!(centered_position(work, work, 728, 666), (596, 187));
            let owner = RECT {
                left: 10,
                top: 10,
                right: 310,
                bottom: 210,
            };
            assert_eq!(centered_position(owner, work, 470, 340), (0, 0));
        }
    }
}

#[cfg(windows)]
pub use windows_ui::run;
