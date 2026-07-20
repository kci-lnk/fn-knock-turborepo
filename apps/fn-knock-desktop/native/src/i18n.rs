use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Locale {
    ZhCn = 0,
    ZhHant = 1,
    En = 2,
    KoKr = 3,
    JaJp = 4,
}

impl Locale {
    pub const ALL: [Self; 5] = [Self::ZhCn, Self::ZhHant, Self::En, Self::KoKr, Self::JaJp];

    pub const fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::ZhHant => "zh-Hant",
            Self::En => "en",
            Self::KoKr => "ko-KR",
            Self::JaJp => "ja-JP",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::ZhCn => "中文简体",
            Self::ZhHant => "中文正體",
            Self::En => "English",
            Self::KoKr => "한국어",
            Self::JaJp => "日本語",
        }
    }

    pub fn from_tag(value: &str) -> Option<Self> {
        let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
        if normalized.is_empty() {
            return None;
        }
        match normalized.as_str() {
            "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant" | "zh-hant-tw" | "zh-hant-hk" => {
                Some(Self::ZhHant)
            }
            "zh" | "zh-cn" | "zh-sg" | "zh-my" | "zh-hans" | "zh-hans-cn" => Some(Self::ZhCn),
            "en" => Some(Self::En),
            "ko" | "ko-kr" => Some(Self::KoKr),
            "ja" | "ja-jp" => Some(Self::JaJp),
            _ if normalized.starts_with("zh-hant-") => Some(Self::ZhHant),
            _ if normalized.starts_with("zh-") => Some(Self::ZhCn),
            _ if normalized.starts_with("en-") => Some(Self::En),
            _ if normalized.starts_with("ko-") => Some(Self::KoKr),
            _ if normalized.starts_with("ja-") => Some(Self::JaJp),
            _ => None,
        }
    }
}

static ACTIVE_LOCALE: AtomicU8 = AtomicU8::new(Locale::ZhCn as u8);
static FOLLOW_WINDOWS: AtomicBool = AtomicBool::new(true);

pub fn active_locale() -> Locale {
    match ACTIVE_LOCALE.load(Ordering::Acquire) {
        1 => Locale::ZhHant,
        2 => Locale::En,
        3 => Locale::KoKr,
        4 => Locale::JaJp,
        _ => Locale::ZhCn,
    }
}

pub fn follows_windows() -> bool {
    FOLLOW_WINDOWS.load(Ordering::Acquire)
}

fn set_active(locale: Locale, follows_windows: bool) {
    ACTIVE_LOCALE.store(locale as u8, Ordering::Release);
    FOLLOW_WINDOWS.store(follows_windows, Ordering::Release);
}

fn preference_path() -> Option<PathBuf> {
    dirs::config_dir().map(|directory| directory.join("FnKnock").join("windows-manager-locale.txt"))
}

fn read_preference() -> Option<Locale> {
    let raw = fs::read_to_string(preference_path()?).ok()?;
    let value = raw.trim();
    if value.eq_ignore_ascii_case("auto") {
        None
    } else {
        Locale::from_tag(value)
    }
}

fn write_preference(locale: Option<Locale>) -> Result<(), String> {
    let path = preference_path().ok_or_else(|| tr("无法定位当前用户的配置目录").to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| tr("语言偏好路径无效").to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("{}：{error}", tr("无法创建语言偏好目录")))?;
    fs::write(path, locale.map_or("auto", Locale::code))
        .map_err(|error| format!("{}：{error}", tr("无法保存语言偏好")))
}

pub fn initialize() -> Locale {
    if let Some(locale) = read_preference() {
        set_active(locale, false);
        return locale;
    }
    let locale = detect_system_locale().unwrap_or(Locale::ZhCn);
    set_active(locale, true);
    locale
}

pub fn choose(locale: Option<Locale>) -> Result<Locale, String> {
    let resolved = locale.or_else(detect_system_locale).unwrap_or(Locale::ZhCn);
    set_active(resolved, locale.is_none());
    write_preference(locale)?;
    Ok(resolved)
}

#[cfg(windows)]
fn detect_system_locale() -> Option<Locale> {
    use windows_sys::Win32::Globalization::{
        GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
    };

    unsafe {
        let mut language_count = 0_u32;
        let mut buffer_length = 0_u32;
        let _ = GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut language_count,
            std::ptr::null_mut(),
            &mut buffer_length,
        );
        if buffer_length > 1 {
            let mut buffer = vec![0_u16; buffer_length as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut language_count,
                buffer.as_mut_ptr(),
                &mut buffer_length,
            ) != 0
            {
                for raw in buffer
                    .split(|value| *value == 0)
                    .filter(|part| !part.is_empty())
                {
                    if let Some(locale) = Locale::from_tag(&String::from_utf16_lossy(raw)) {
                        return Some(locale);
                    }
                }
            }
        }

        let mut locale_name = [0_u16; 85];
        let length = GetUserDefaultLocaleName(locale_name.as_mut_ptr(), locale_name.len() as i32);
        (length > 1)
            .then(|| String::from_utf16_lossy(&locale_name[..length as usize - 1]))
            .and_then(|value| Locale::from_tag(&value))
    }
}

#[cfg(not(windows))]
fn detect_system_locale() -> Option<Locale> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find_map(|value| Locale::from_tag(value.split('.').next().unwrap_or(&value)))
}

pub fn language_button_label() -> String {
    format!("🌐 {}", active_locale().display_name())
}

pub fn label_separator() -> &'static str {
    match active_locale() {
        Locale::En | Locale::KoKr => ": ",
        Locale::ZhCn | Locale::ZhHant | Locale::JaJp => "：",
    }
}

pub fn tr(key: &'static str) -> &'static str {
    tr_for(active_locale(), key)
}

pub fn tr_for(locale: Locale, key: &'static str) -> &'static str {
    let Some(values) = translations(key) else {
        return key;
    };
    match locale {
        Locale::ZhCn => key,
        Locale::ZhHant => values[0],
        Locale::En => values[1],
        Locale::KoKr => values[2],
        Locale::JaJp => values[3],
    }
}

// Keys are written in Simplified Chinese so a missing translation naturally
// follows the product-wide zh-CN fallback contract.
fn translations(key: &'static str) -> Option<[&'static str; 4]> {
    Some(match key {
        "Windows 管理程序" => [
            "Windows 管理程式",
            "Windows Manager",
            "Windows 관리 프로그램",
            "Windows Manager",
        ],
        "fn-knock 操作失败" => [
            "fn-knock 操作失敗",
            "fn-knock operation failed",
            "fn-knock 작업 실패",
            "fn-knock の操作に失敗しました",
        ],
        "正在执行操作，请稍候…" => [
            "正在執行操作，請稍候…",
            "Working… Please wait.",
            "작업을 수행 중입니다. 잠시 기다려 주세요…",
            "処理中です。しばらくお待ちください…",
        ],
        "服务运行正常" => [
            "服務運作正常",
            "Service is healthy",
            "서비스 정상 작동 중",
            "サービスは正常です",
        ],
        "服务需要处理" => [
            "服務需要檢查",
            "Service needs attention",
            "서비스 확인 필요",
            "サービスを確認してください",
        ],
        "Windows 服务" => [
            "Windows 服務",
            "Windows Service",
            "Windows 서비스",
            "Windows サービス",
        ],
        "管理后台" => ["管理介面", "Admin UI", "관리 콘솔", "管理コンソール"],
        "网关、认证与管理组件均已就绪" => [
            "閘道、認證與管理元件均已就緒",
            "Gateway, authentication, and admin components are ready",
            "게이트웨이, 인증 및 관리 구성 요소가 준비되었습니다",
            "ゲートウェイ、認証、管理コンポーネントはすべて準備完了です",
        ],
        "服务尚未就绪" => [
            "服務尚未就緒",
            "Service is not ready",
            "서비스가 아직 준비되지 않았습니다",
            "サービスの準備が完了していません",
        ],
        "保存并重启服务" => [
            "儲存並重啟服務",
            "Save & Restart",
            "저장 후 서비스 재시작",
            "保存して再起動",
        ],
        "保存并启动服务" => [
            "儲存並啟動服務",
            "Save & Start",
            "저장 후 서비스 시작",
            "保存して起動",
        ],
        "保存端口设置" => [
            "儲存連接埠設定",
            "Save Ports",
            "포트 설정 저장",
            "ポート設定を保存",
        ],
        "内存占用" => ["記憶體用量", "Memory", "메모리 사용량", "メモリ使用量"],
        "服务" => ["服務", "Service", "서비스", "サービス"],
        "网关" => ["閘道", "Gateway", "게이트웨이", "ゲートウェイ"],
        "合计" => ["合計", "Total", "합계", "合計"],
        "管理端口" => [
            "管理連接埠",
            "Admin UI port",
            "관리 콘솔 포트",
            "管理コンソールポート",
        ],
        "代理端口" => [
            "代理連接埠",
            "Reverse proxy port",
            "리버스 프록시 포트",
            "リバースプロキシポート",
        ],
        "Rust API 端口" => [
            "Rust API 連接埠",
            "Rust API port",
            "Rust API 포트",
            "Rust API ポート",
        ],
        "认证端口" => [
            "認證連接埠",
            "Authentication port",
            "인증 포트",
            "認証ポート",
        ],
        "Go gRPC 端口" => [
            "Go gRPC 連接埠",
            "Go gRPC port",
            "Go gRPC 포트",
            "Go gRPC ポート",
        ],
        "必须是 1–65535 的整数" => [
            "必須是 1–65535 的整數",
            " must be an integer from 1 to 65535",
            "는 1~65535 사이의 정수여야 합니다",
            "は 1～65535 の整数を入力してください",
        ],
        "无法打开链接" => [
            "無法開啟連結",
            "Could not open link",
            "링크를 열 수 없습니다",
            "リンクを開けませんでした",
        ],
        "当前已经是最新稳定版本。" => [
            "目前已是最新穩定版本。",
            "You already have the latest stable release.",
            "이미 최신 안정 버전을 사용 중입니다.",
            "最新の安定版を使用しています。",
        ],
        "检测到必须安装的重要更新。" => [
            "檢測到必須安裝的重要更新。",
            "An important required update is available.",
            "필수 중요 업데이트가 있습니다.",
            "インストール必須の重要なアップデートがあります。",
        ],
        "检测到版本更新。" => [
            "檢測到版本更新。",
            "A new release is available.",
            "새 버전이 있습니다.",
            "新しいバージョンがあります。",
        ],
        "当前版本" => [
            "目前版本",
            "Current version",
            "현재 버전",
            "現在のバージョン",
        ],
        "最新版本" => ["最新版本", "Latest version", "최신 버전", "最新バージョン"],
        "确认要更新吗？" => [
            "確認要更新嗎？",
            "Install this update now?",
            "지금 업데이트하시겠습니까?",
            "今すぐアップデートしますか？",
        ],
        "更新" => ["更新", "Update", "업데이트", "アップデート"],
        "更新安装器已启动，管理程序将退出。" => [
            "更新安裝程式已啟動，管理程式將結束。",
            "The updater has started. Windows Manager will now exit.",
            "업데이트 설치 프로그램이 시작되었습니다. Windows 관리 프로그램을 종료합니다.",
            "アップデーターを起動しました。Windows Manager を終了します。",
        ],
        "本机网关服务与管理程序" => [
            "本機閘道服務與管理程式",
            "Local gateway service and manager",
            "로컬 게이트웨이 서비스 및 관리 프로그램",
            "ローカルゲートウェイサービスと管理ツール",
        ],
        "官方网站" => [
            "官方網站",
            "Official Website",
            "공식 웹사이트",
            "公式サイト",
        ],
        "GitHub 项目" => [
            "GitHub 專案",
            "GitHub Project",
            "GitHub 프로젝트",
            "GitHub プロジェクト",
        ],
        "关闭" => ["關閉", "Close", "닫기", "閉じる"],
        "关于" => ["關於", "About", "정보", "製品情報"],
        "关于 Knock 敲门" => [
            "關於 Knock 敲門",
            "About Knock",
            "Knock 정보",
            "Knock について",
        ],
        "打开管理程序" => [
            "開啟管理程式",
            "Open Windows Manager",
            "Windows 관리 프로그램 열기",
            "Windows Manager を開く",
        ],
        "打开管理后台" => [
            "開啟管理介面",
            "Open Admin UI",
            "관리 콘솔 열기",
            "管理コンソールを開く",
        ],
        "停止服务" => ["停止服務", "Stop Service", "서비스 중지", "サービスを停止"],
        "重启服务" => [
            "重啟服務",
            "Restart Service",
            "서비스 재시작",
            "サービスを再起動",
        ],
        "启动服务" => ["啟動服務", "Start Service", "서비스 시작", "サービスを起動"],
        "检查更新" => [
            "檢查更新",
            "Check for Updates",
            "업데이트 확인",
            "アップデートを確認",
        ],
        "版本" => ["版本", "Version", "버전", "バージョン"],
        "退出管理程序" => [
            "結束管理程式",
            "Exit Windows Manager",
            "Windows 관리 프로그램 종료",
            "Windows Manager を終了",
        ],
        "正在重启服务…" => [
            "正在重啟服務…",
            "Restarting service…",
            "서비스 재시작 중…",
            "サービスを再起動しています…",
        ],
        "fn-knock 服务已重新启动。" => [
            "fn-knock 服務已重新啟動。",
            "The fn-knock service has restarted.",
            "fn-knock 서비스가 재시작되었습니다.",
            "fn-knock サービスを再起動しました。",
        ],
        "正在启动服务…" => [
            "正在啟動服務…",
            "Starting service…",
            "서비스 시작 중…",
            "サービスを起動しています…",
        ],
        "fn-knock 服务已启动。" => [
            "fn-knock 服務已啟動。",
            "The fn-knock service has started.",
            "fn-knock 서비스가 시작되었습니다.",
            "fn-knock サービスを起動しました。",
        ],
        "正在停止服务…" => [
            "正在停止服務…",
            "Stopping service…",
            "서비스 중지 중…",
            "サービスを停止しています…",
        ],
        "fn-knock 服务已停止。" => [
            "fn-knock 服務已停止。",
            "The fn-knock service has stopped.",
            "fn-knock 서비스가 중지되었습니다.",
            "fn-knock サービスを停止しました。",
        ],
        "fn-knock 本机服务与端口管理" => [
            "fn-knock 本機服務與連接埠管理",
            "Local Service & Port Management",
            "로컬 서비스 및 포트 관리",
            "ローカルサービスとポートの管理",
        ],
        "运行状态" => [
            "執行狀態",
            "Runtime Status",
            "실행 상태",
            "ランタイムの状態",
        ],
        "正在读取状态…" => [
            "正在讀取狀態…",
            "Loading status…",
            "상태를 불러오는 중…",
            "状態を読み込んでいます…",
        ],
        "内存占用：正在读取…" => [
            "記憶體用量：正在讀取…",
            "Memory: loading…",
            "메모리 사용량: 불러오는 중…",
            "メモリ使用量：読み込み中…",
        ],
        "端口设置" => [
            "連接埠設定",
            "Port Configuration",
            "포트 설정",
            "ポート設定",
        ],
        "代理入口" => [
            "反向代理",
            "Reverse Proxy",
            "리버스 프록시",
            "リバースプロキシ",
        ],
        "高级端口（通常无需修改）" => [
            "進階連接埠（通常無需修改）",
            "Advanced ports (usually unchanged)",
            "고급 포트(일반적으로 변경 불필요)",
            "詳細ポート（通常は変更不要）",
        ],
        "认证" => ["認證", "Auth", "인증", "認証"],
        "刷新状态" => [
            "重新整理狀態",
            "Refresh Status",
            "상태 새로고침",
            "状態を更新",
        ],
        "清除管理密码" => [
            "清除管理密碼",
            "Clear Admin Password",
            "관리자 비밀번호 초기화",
            "管理者パスワードを消去",
        ],
        "关闭窗口后，fn-knock 将继续在系统托盘运行。" => [
            "關閉後 fn-knock 會留在系統匣。",
            "Closing keeps fn-knock in the system tray.",
            "창을 닫아도 fn-knock은 시스템 트레이에서 실행됩니다.",
            "閉じても fn-knock は通知領域で動作します。",
        ],
        "正在应用端口并验证服务…" => [
            "正在套用連接埠並驗證服務…",
            "Applying ports and validating service…",
            "포트 설정 적용 및 서비스 확인 중…",
            "ポートを適用してサービスを検証しています…",
        ],
        "端口配置已生效，服务已通过就绪检查。" => [
            "連接埠設定已生效，服務已通過就緒檢查。",
            "Port configuration is active and the service passed its readiness check.",
            "포트 설정이 적용되었으며 서비스 준비 상태 확인을 통과했습니다.",
            "ポート設定を適用し、サービスの準備完了チェックに成功しました。",
        ],
        "将清除管理密码、登录会话与失败退避状态。确定继续吗？" => [
            "將清除管理密碼、登入 Session 與失敗退避狀態。確定繼續嗎？",
            "This clears the admin password, login sessions, and failed-login backoff state. Continue?",
            "관리자 비밀번호, 로그인 세션 및 로그인 실패 백오프 상태를 초기화합니다. 계속하시겠습니까?",
            "管理者パスワード、ログインセッション、ログイン失敗時のバックオフ状態を消去します。続行しますか？",
        ],
        "正在清除管理密码…" => [
            "正在清除管理密碼…",
            "Clearing admin password…",
            "관리자 비밀번호 초기화 중…",
            "管理者パスワードを消去しています…",
        ],
        "管理密码与现有登录会话已清除，服务已恢复。" => [
            "管理密碼與現有登入 Session 已清除，服務已恢復。",
            "The admin password and active login sessions were cleared; the service is back online.",
            "관리자 비밀번호와 기존 로그인 세션이 초기화되었으며 서비스가 복구되었습니다.",
            "管理者パスワードと既存のログインセッションを消去し、サービスを復旧しました。",
        ],
        "跟随 Windows" => [
            "跟隨 Windows",
            "Use Windows language",
            "Windows 언어 사용",
            "Windows の言語を使用",
        ],
        "语言" => ["語言", "Language", "언어", "言語"],
        "无法定位当前用户的配置目录" => [
            "無法定位目前使用者的設定目錄",
            "Could not locate the current user's configuration directory",
            "현재 사용자의 설정 디렉터리를 찾을 수 없습니다",
            "現在のユーザーの設定フォルダーが見つかりません",
        ],
        "语言偏好路径无效" => [
            "語言偏好路徑無效",
            "The language preference path is invalid",
            "언어 기본 설정 경로가 올바르지 않습니다",
            "言語設定のパスが無効です",
        ],
        "无法创建语言偏好目录" => [
            "無法建立語言偏好目錄",
            "Could not create the language preference directory",
            "언어 기본 설정 디렉터리를 만들 수 없습니다",
            "言語設定フォルダーを作成できません",
        ],
        "无法保存语言偏好" => [
            "無法儲存語言偏好",
            "Could not save the language preference",
            "언어 기본 설정을 저장할 수 없습니다",
            "言語設定を保存できません",
        ],
        "未知服务" => [
            "未知服務",
            "Unknown service",
            "알 수 없는 서비스",
            "不明なサービス",
        ],
        "已停止" => ["已停止", "Stopped", "중지됨", "停止"],
        "正在启动" => ["正在啟動", "Starting", "시작 중", "起動中"],
        "正在停止" => ["正在停止", "Stopping", "중지 중", "停止中"],
        "运行中" => ["運作中", "Running", "실행 중", "実行中"],
        "正在继续" => ["正在繼續", "Resuming", "재개 중", "再開中"],
        "正在暂停" => ["正在暫停", "Pausing", "일시 중지 중", "一時停止中"],
        "已暂停" => ["已暫停", "Paused", "일시 중지됨", "一時停止"],
        "不可用" => ["無法使用", "Unavailable", "사용할 수 없음", "利用不可"],
        "仅 Windows 可用" => [
            "僅 Windows 可用",
            "Available on Windows only",
            "Windows에서만 사용할 수 있습니다",
            "Windows でのみ利用できます",
        ],
        "无法解析 Windows ProgramData" => [
            "無法解析 Windows ProgramData",
            "Could not resolve Windows ProgramData",
            "Windows ProgramData 경로를 확인할 수 없습니다",
            "Windows ProgramData のパスを取得できません",
        ],
        "无法连接 Windows 服务管理器" => [
            "無法連線 Windows 服務控制管理員",
            "Could not connect to Windows Service Control Manager",
            "Windows 서비스 제어 관리자에 연결할 수 없습니다",
            "Windows サービスコントロールマネージャーに接続できません",
        ],
        "无法打开 fn-knock 服务" => [
            "無法開啟 fn-knock 服務",
            "Could not open the fn-knock service",
            "fn-knock 서비스를 열 수 없습니다",
            "fn-knock サービスを開けません",
        ],
        "查询服务状态失败" => [
            "查詢服務狀態失敗",
            "Could not query service status",
            "서비스 상태를 확인할 수 없습니다",
            "サービスの状態を取得できません",
        ],
        "等待服务状态" => [
            "等待服務狀態",
            "Waiting for service state",
            "서비스 상태 대기",
            "サービス状態",
        ],
        "超时" => [
            "逾時",
            "timed out",
            "시간 초과",
            "の待機がタイムアウトしました",
        ],
        "当前状态为" => ["目前狀態為", "current state:", "현재 상태:", "現在の状態:"],
        "无法打开进程以读取内存信息" => [
            "無法開啟處理程序以讀取記憶體資訊：",
            "Could not open process to read memory information:",
            "메모리 정보를 읽기 위해 프로세스를 열 수 없습니다:",
            "メモリ情報を読み取るためのプロセスを開けません:",
        ],
        "无法读取进程内存信息" => [
            "無法讀取處理程序記憶體資訊：",
            "Could not read memory information for process:",
            "프로세스 메모리 정보를 읽을 수 없습니다:",
            "プロセスのメモリ情報を読み取れません:",
        ],
        "SCM 未返回服务 PID" => [
            "SCM 未傳回服務 PID",
            "SCM did not return a service PID",
            "SCM에서 서비스 PID를 반환하지 않았습니다",
            "SCM からサービス PID が返されませんでした",
        ],
        "无法枚举 fn-knock 网关进程" => [
            "無法列舉 fn-knock 閘道處理程序",
            "Could not enumerate fn-knock gateway processes",
            "fn-knock 게이트웨이 프로세스를 열거할 수 없습니다",
            "fn-knock ゲートウェイプロセスを列挙できません",
        ],
        "fn-knock 网关进程尚未提供内存数据" => [
            "fn-knock 閘道處理程序尚未提供記憶體資料",
            "Memory data is not yet available for the fn-knock gateway process",
            "fn-knock 게이트웨이 프로세스의 메모리 데이터를 아직 사용할 수 없습니다",
            "fn-knock ゲートウェイプロセスのメモリ情報はまだ取得できません",
        ],
        "启动服务失败" => [
            "啟動服務失敗",
            "Could not start the service",
            "서비스를 시작할 수 없습니다",
            "サービスを起動できません",
        ],
        "停止服务失败" => [
            "停止服務失敗",
            "Could not stop the service",
            "서비스를 중지할 수 없습니다",
            "サービスを停止できません",
        ],
        "运行配置路径缺少父目录" => [
            "運行設定路徑缺少上層目錄",
            "The runtime configuration path has no parent directory",
            "런타임 설정 경로에 상위 디렉터리가 없습니다",
            "ランタイム設定のパスに親フォルダーがありません",
        ],
        "创建配置目录失败" => [
            "建立設定目錄失敗",
            "Could not create the configuration directory",
            "설정 디렉터리를 만들 수 없습니다",
            "設定フォルダーを作成できません",
        ],
        "运行配置缺少有效的管理端口" => [
            "運行設定缺少有效的管理連接埠",
            "The runtime configuration has no valid Admin UI port",
            "런타임 설정에 유효한 관리 콘솔 포트가 없습니다",
            "ランタイム設定に有効な管理コンソールポートがありません",
        ],
        "创建临时配置失败" => [
            "建立暫存設定失敗",
            "Could not create the staged configuration",
            "임시 설정 파일을 만들 수 없습니다",
            "一時設定ファイルを作成できません",
        ],
        "写入临时配置失败" => [
            "寫入暫存設定失敗",
            "Could not write the staged configuration",
            "임시 설정 파일을 쓸 수 없습니다",
            "一時設定ファイルに書き込めません",
        ],
        "备份旧配置失败" => [
            "備份舊設定失敗",
            "Could not back up the previous configuration",
            "기존 설정을 백업할 수 없습니다",
            "以前の設定をバックアップできません",
        ],
        "替换运行配置失败" => [
            "替換運行設定失敗",
            "Could not replace the runtime configuration",
            "런타임 설정을 교체할 수 없습니다",
            "ランタイム設定を置き換えられません",
        ],
        "新管理端口" => [
            "新的管理連接埠",
            "New Admin UI port",
            "새 관리 콘솔 포트",
            "新しい管理コンソールポート",
        ],
        "未能就绪" => [
            "未能就緒",
            "did not become ready",
            "가 준비 상태가 되지 않았습니다",
            "が準備完了になりませんでした",
        ],
        "readyz 超时" => [
            "readyz 逾時",
            "readyz timed out",
            "readyz 시간 초과",
            "readyz がタイムアウトしました",
        ],
        "旧管理端口" => [
            "舊的管理連接埠",
            "Previous Admin UI port",
            "기존 관리 콘솔 포트",
            "以前の管理コンソールポート",
        ],
        "回滚后未能就绪" => [
            "回復後未能就緒",
            "did not become ready after rollback",
            "롤백 후 준비 상태가 되지 않았습니다",
            "ロールバック後に準備完了になりませんでした",
        ],
        "已恢复旧配置" => [
            "已恢復舊設定",
            "the previous configuration was restored",
            "기존 설정을 복원했습니다",
            "以前の設定を復元しました",
        ],
        "旧配置已恢复，但服务恢复失败" => [
            "舊設定已恢復，但服務恢復失敗",
            "The previous configuration was restored, but service recovery failed",
            "기존 설정은 복원했지만 서비스를 복구하지 못했습니다",
            "以前の設定は復元しましたが、サービスを復旧できませんでした",
        ],
        "管理程序路径缺少父目录" => [
            "管理程式路徑缺少上層目錄",
            "The Windows Manager path has no parent directory",
            "Windows 관리 프로그램 경로에 상위 디렉터리가 없습니다",
            "Windows Manager のパスに親フォルダーがありません",
        ],
        "无法执行密码清理" => [
            "無法執行密碼清除",
            "Could not run the password reset command",
            "비밀번호 초기화 명령을 실행할 수 없습니다",
            "パスワード消去コマンドを実行できません",
        ],
        "密码清理失败" => [
            "密碼清除失敗",
            "Password reset failed",
            "비밀번호 초기화 실패",
            "パスワードの消去に失敗しました",
        ],
        "无法读取 TCP 监听表" => [
            "無法讀取 TCP 監聽清單",
            "Could not read the TCP listener table",
            "TCP 리스너 테이블을 읽을 수 없습니다",
            "TCP リスナーテーブルを読み取れません",
        ],
        "服务标识不匹配" => [
            "服務識別不符",
            "Service identity mismatch",
            "서비스 ID가 일치하지 않습니다",
            "サービスの識別情報が一致しません",
        ],
        "fn-knock 服务尚未运行" => [
            "fn-knock 服務尚未運行",
            "The fn-knock service is not running",
            "fn-knock 서비스가 실행 중이 아닙니다",
            "fn-knock サービスは実行されていません",
        ],
        "尚未监听" => [
            "尚未監聽",
            "is not listening",
            "에서 수신 대기 중이 아닙니다",
            "は待ち受けていません",
        ],
        "不属于 fn-knock 服务" => [
            "不屬於 fn-knock 服務",
            "is not owned by the fn-knock service",
            "는 fn-knock 서비스에 속하지 않습니다",
            "は fn-knock サービスのものではありません",
        ],
        "不支持的运行配置架构" => [
            "不支援的運行設定結構",
            "Unsupported runtime configuration schema",
            "지원되지 않는 런타임 설정 스키마",
            "未対応のランタイム設定スキーマ",
        ],
        "端口必须介于 1 和 65535 之间" => [
            "連接埠必須介於 1 和 65535 之間",
            "Ports must be between 1 and 65535",
            "포트는 1~65535 사이여야 합니다",
            "ポート番号は 1～65535 の範囲で指定してください",
        ],
        "内部端口必须大于或等于 1024" => [
            "內部連接埠必須大於或等於 1024",
            "Internal ports must be 1024 or higher",
            "내부 포트는 1024 이상이어야 합니다",
            "内部ポートは 1024 以上にしてください",
        ],
        "五个运行端口不能重复" => [
            "五個運行連接埠不能重複",
            "The five runtime ports must be unique",
            "5개의 런타임 포트는 서로 달라야 합니다",
            "5 つのランタイムポートには異なる番号を指定してください",
        ],
        "无法读取" => [
            "無法讀取",
            "Could not read",
            "읽을 수 없습니다:",
            "読み取れません:",
        ],
        "无效配置" => [
            "無效設定",
            "Invalid configuration:",
            "잘못된 설정:",
            "無効な設定:",
        ],
        "无法编码运行配置" => [
            "無法編碼運行設定",
            "Could not encode the runtime configuration",
            "런타임 설정을 인코딩할 수 없습니다",
            "ランタイム設定をエンコードできません",
        ],
        "管理服务未响应" => [
            "管理服務未回應",
            "Admin service did not respond",
            "관리 서비스가 응답하지 않습니다",
            "管理サービスから応答がありません",
        ],
        "就绪检查发送失败" => [
            "就緒檢查傳送失敗",
            "Could not send readiness probe",
            "준비 상태 확인 요청을 보낼 수 없습니다",
            "準備完了プローブを送信できません",
        ],
        "就绪检查读取失败" => [
            "就緒檢查讀取失敗",
            "Could not read readiness response",
            "준비 상태 확인 응답을 읽을 수 없습니다",
            "準備完了レスポンスを読み取れません",
        ],
        "就绪检查期间 FnKnock 服务或管理端口所有者发生变化" => [
            "就緒檢查期間 FnKnock 服務或管理連接埠擁有者發生變化",
            "The FnKnock service or Admin UI port owner changed during the readiness probe",
            "준비 상태 확인 중 FnKnock 서비스 또는 관리 콘솔 포트 소유자가 변경되었습니다",
            "準備完了チェック中に FnKnock サービスまたは管理コンソールポートの所有プロセスが変わりました",
        ],
        "就绪响应未确认完整运行组件" => [
            "就緒回應未確認完整運行元件",
            "The readiness response did not confirm all runtime components",
            "준비 상태 응답에서 모든 런타임 구성 요소를 확인하지 못했습니다",
            "準備完了レスポンスで全ランタイムコンポーネントを確認できませんでした",
        ],
        "管理" => ["管理", "Admin", "관리", "管理"],
        "代理" => ["代理", "Proxy", "프록시", "プロキシ"],
        "检查更新失败" => [
            "檢查更新失敗",
            "Could not check for updates",
            "업데이트를 확인할 수 없습니다",
            "アップデートを確認できません",
        ],
        "无法初始化更新网络客户端" => [
            "無法初始化更新網路用戶端",
            "Could not initialize the update network client",
            "업데이트 네트워크 클라이언트를 초기화할 수 없습니다",
            "アップデート用ネットワーククライアントを初期化できません",
        ],
        "下载更新安装包失败" => [
            "下載更新安裝套件失敗",
            "Could not download the update installer",
            "업데이트 설치 프로그램을 다운로드할 수 없습니다",
            "アップデートインストーラーをダウンロードできません",
        ],
        "更新清单无效" => [
            "更新資訊清單無效",
            "The update manifest is invalid",
            "업데이트 매니페스트가 올바르지 않습니다",
            "アップデートマニフェストが無効です",
        ],
        "更新清单缺少 Windows x86_64 安装包" => [
            "更新資訊清單缺少 Windows x86_64 安裝程式",
            "The update manifest has no Windows x86_64 installer",
            "업데이트 매니페스트에 Windows x86_64 설치 프로그램이 없습니다",
            "アップデートマニフェストに Windows x86_64 インストーラーがありません",
        ],
        "更新下载地址不受信任" => [
            "更新下載網址不受信任",
            "The update download URL is not trusted",
            "업데이트 다운로드 URL을 신뢰할 수 없습니다",
            "アップデートのダウンロード URL は信頼できません",
        ],
        "更新安装包大小不匹配" => [
            "更新安裝套件大小不符",
            "Update installer size mismatch",
            "업데이트 설치 프로그램 크기가 일치하지 않습니다",
            "アップデートインストーラーのサイズが一致しません",
        ],
        "更新安装包 SHA-256 不匹配" => [
            "更新安裝套件 SHA-256 不符",
            "Update installer SHA-256 mismatch",
            "업데이트 설치 프로그램의 SHA-256이 일치하지 않습니다",
            "アップデートインストーラーの SHA-256 が一致しません",
        ],
        "启动更新安装器失败" => [
            "啟動更新安裝程式失敗",
            "Could not start the update installer",
            "업데이트 설치 프로그램을 시작할 수 없습니다",
            "アップデートインストーラーを起動できません",
        ],
        "已尝试主更新目录和临时备用目录" => [
            "已嘗試主要更新目錄和暫存備用目錄",
            "Tried the primary update directory and the temporary fallback directory",
            "기본 업데이트 디렉터리와 임시 대체 디렉터리를 모두 시도했습니다",
            "更新用の既定フォルダーと一時フォールバックフォルダーを試しました",
        ],
        "创建更新目录失败" => [
            "建立更新目錄失敗",
            "Could not create the update directory",
            "업데이트 디렉터리를 만들 수 없습니다",
            "アップデートフォルダーを作成できません",
        ],
        "创建更新安装包失败" => [
            "建立更新安裝套件失敗",
            "Could not create the update installer",
            "업데이트 설치 프로그램을 만들 수 없습니다",
            "アップデートインストーラーを作成できません",
        ],
        "写入更新安装包失败" => [
            "寫入更新安裝套件失敗",
            "Could not write the update installer",
            "업데이트 설치 프로그램을 쓸 수 없습니다",
            "アップデートインストーラーを書き込めません",
        ],
        "同步更新安装包失败" => [
            "同步更新安裝套件失敗",
            "Could not flush the update installer to disk",
            "업데이트 설치 프로그램을 디스크에 동기화할 수 없습니다",
            "アップデートインストーラーをディスクに同期できません",
        ],
        "提交更新安装包失败" => [
            "提交更新安裝套件失敗",
            "Could not finalize the update installer",
            "업데이트 설치 프로그램을 확정할 수 없습니다",
            "アップデートインストーラーを確定できません",
        ],
        "Windows 错误" => [
            "Windows 錯誤",
            "Windows error",
            "Windows 오류",
            "Windows エラー",
        ],
        "安装包或所在目录损坏且无法读取" => [
            "安裝套件或所在目錄損壞且無法讀取",
            "The installer or its directory is corrupted and unreadable",
            "설치 프로그램 또는 해당 디렉터리가 손상되어 읽을 수 없습니다",
            "インストーラーまたはそのフォルダーが破損しており読み取れません",
        ],
        "创建安装器进程失败" => [
            "建立安裝程式處理程序失敗",
            "Could not create the installer process",
            "설치 프로그램 프로세스를 만들 수 없습니다",
            "インストーラープロセスを作成できません",
        ],
        "重新读取更新安装包失败" => [
            "重新讀取更新安裝套件失敗",
            "Could not re-read the update installer",
            "업데이트 설치 프로그램을 다시 읽을 수 없습니다",
            "アップデートインストーラーを再読み込みできません",
        ],
        "读取更新安装包属性失败" => [
            "讀取更新安裝套件屬性失敗",
            "Could not read update installer metadata",
            "업데이트 설치 프로그램 메타데이터를 읽을 수 없습니다",
            "アップデートインストーラーのメタデータを読み取れません",
        ],
        "更新安装包落盘后的大小不匹配" => [
            "更新安裝套件寫入磁碟後大小不符",
            "Staged update installer size mismatch",
            "디스크에 저장된 업데이트 설치 프로그램의 크기가 일치하지 않습니다",
            "保存したアップデートインストーラーのサイズが一致しません",
        ],
        "更新安装包落盘后的 SHA-256 不匹配" => [
            "更新安裝套件寫入磁碟後 SHA-256 不符",
            "Staged update installer SHA-256 mismatch",
            "디스크에 저장된 업데이트 설치 프로그램의 SHA-256이 일치하지 않습니다",
            "保存したアップデートインストーラーの SHA-256 が一致しません",
        ],
        "更新安装包不是有效的 Windows 可执行文件" => [
            "更新安裝套件不是有效的 Windows 可執行檔",
            "The update installer is not a valid Windows executable",
            "업데이트 설치 프로그램이 올바른 Windows 실행 파일이 아닙니다",
            "アップデートインストーラーは有効な Windows 実行ファイルではありません",
        ],
        "更新安装包不是有效的 Windows 可执行文件（缺少 MZ 文件头）" => [
            "更新安裝套件不是有效的 Windows 可執行檔（缺少 MZ 檔頭）",
            "The update installer is not a valid Windows executable (missing MZ header)",
            "업데이트 설치 프로그램이 올바른 Windows 실행 파일이 아닙니다(MZ 헤더 없음)",
            "アップデートインストーラーは有効な Windows 実行ファイルではありません（MZ ヘッダーなし）",
        ],
        "更新安装包不是有效的 Windows 可执行文件（PE 文件头越界）" => [
            "更新安裝套件不是有效的 Windows 可執行檔（PE 檔頭超出範圍）",
            "The update installer is not a valid Windows executable (PE header is out of bounds)",
            "업데이트 설치 프로그램이 올바른 Windows 실행 파일이 아닙니다(PE 헤더 범위 초과)",
            "アップデートインストーラーは有効な Windows 実行ファイルではありません（PE ヘッダーが範囲外）",
        ],
        "读取 Windows PE 文件头失败" => [
            "讀取 Windows PE 檔頭失敗",
            "Could not read the Windows PE header",
            "Windows PE 헤더를 읽을 수 없습니다",
            "Windows PE ヘッダーを読み取れません",
        ],
        "更新安装包不是有效的 Windows 可执行文件（PE 签名无效）" => [
            "更新安裝套件不是有效的 Windows 可執行檔（PE 簽章無效）",
            "The update installer is not a valid Windows executable (invalid PE signature)",
            "업데이트 설치 프로그램이 올바른 Windows 실행 파일이 아닙니다(잘못된 PE 서명)",
            "アップデートインストーラーは有効な Windows 実行ファイルではありません（PE シグネチャが無効）",
        ],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{Locale, tr_for, translations};

    #[test]
    fn normalizes_supported_windows_language_tags() {
        assert_eq!(Locale::from_tag("zh-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_tag("zh-Hant-TW"), Some(Locale::ZhHant));
        assert_eq!(Locale::from_tag("zh_HK"), Some(Locale::ZhHant));
        assert_eq!(Locale::from_tag("en-US"), Some(Locale::En));
        assert_eq!(Locale::from_tag("ko"), Some(Locale::KoKr));
        assert_eq!(Locale::from_tag("ja-JP"), Some(Locale::JaJp));
        assert_eq!(Locale::from_tag("fr-FR"), None);
        assert_eq!(
            Locale::from_tag("fr-FR").unwrap_or(Locale::ZhCn),
            Locale::ZhCn
        );
    }

    #[test]
    fn unknown_keys_fall_back_to_simplified_chinese_source() {
        assert_eq!(tr_for(Locale::En, "简体中文回退"), "简体中文回退");
    }

    #[test]
    fn domain_terms_use_native_community_wording() {
        assert_eq!(tr_for(Locale::ZhHant, "网关"), "閘道");
        assert_eq!(tr_for(Locale::ZhHant, "管理后台"), "管理介面");
        assert_eq!(tr_for(Locale::En, "管理后台"), "Admin UI");
        assert_eq!(tr_for(Locale::KoKr, "代理入口"), "리버스 프록시");
        assert_eq!(tr_for(Locale::JaJp, "运行状态"), "ランタイムの状態");
    }

    #[test]
    fn every_desktop_translation_key_has_all_four_non_default_locales() {
        for (name, source) in [
            ("native.rs", include_str!("native.rs")),
            ("platform.rs", include_str!("platform.rs")),
            ("runtime.rs", include_str!("runtime.rs")),
            ("update.rs", include_str!("update.rs")),
        ] {
            let mut remainder = source;
            while let Some(index) = remainder.find("i18n::tr(") {
                remainder = &remainder[index + "i18n::tr(".len()..];
                let candidate = remainder.trim_start();
                if let Some(quoted) = candidate.strip_prefix('"') {
                    let end = quoted
                        .find('"')
                        .unwrap_or_else(|| panic!("unterminated translation key in {name}"));
                    let key = &quoted[..end];
                    let translated = translations(key).unwrap_or_else(|| {
                        panic!("missing desktop translation for {key:?} in {name}")
                    });
                    assert!(
                        translated.iter().all(|value| !value.trim().is_empty()),
                        "empty desktop translation for {key:?} in {name}"
                    );
                }
            }
        }
    }
}
