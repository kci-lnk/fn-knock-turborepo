import { zhCNAuth } from "./zh-CN";

export const zhHantAuth = {
  ...zhCNAuth,
  autoIpGrantComment: "登入後自動授權",
  title: "安全驗證",
  captchaFirst: "請先完成下方的人機驗證",
  otpPrompt: "請輸入您的六位數動態密碼完成登入",
  passwordPrompt: "請輸入使用者名稱和密碼完成登入",
  verified: "驗證通過",
  verifying: "正在驗證...",
  verifyError: "驗證錯誤",
  turnstileMissing: "當前 Turnstile 未完成配置，請聯絡管理員填寫 site key。",
  turnstileScriptLoadFailed: "Turnstile 腳本載入失敗",
  turnstileRenderFailed: "Turnstile 渲染失敗，請稍後重試",
  turnstileTimeout: "Turnstile 驗證超時，請重試",
  powUnsupportedAlgorithm: "不支援的 PoW 算法",
  powInvalidChallenge: "PoW challenge 數據無效",
  powSolveFailed: "PoW 求解失敗，請刷新頁面後重試",
  locationResolving: "屬地解析中...",
  locationUnavailable: "屬地暫未獲取",
  openGithub: "打開 GitHub 項目頁",
  menu: "選單",
  loginWithProvider: "使用 {provider} 登入",
  retryAfterSeconds: "{seconds} 秒後重試",
  verifyNow: "立即驗證",
  passwordLogin: "帳號密碼登入",
  totpLogin: "TOTP 登入",
  username: "使用者名稱",
  password: "密碼",
  showPassword: "顯示密碼",
  hidePassword: "隱藏密碼",
  usernamePasswordRequired: "請輸入使用者名稱和密碼",
  passkeyLogin: "Passkey 一鍵登入",
  rememberMe: "記住我",
  passkeyBindTitle: "開啟 Passkey 一鍵登入",
  passkeyBindDescription:
    "是否在目前裝置上綁定 Passkey？綁定後可直接一鍵登入。",
  passkeyBindSkipPrompt: "不再提醒",
  passkeyBindLater: "稍後再說",
  passkeyBindNow: "立即開啟",
  captchaConfigLoadFailed: "驗證碼設定載入失敗，請刷新頁面後重試。",
  captchaFailed: "人機驗證失敗，請重試",
  loggedOutLoginIpGrant:
    "目前瀏覽器會話已退出，登入時授予的目前 IP 訪問權限也已撤銷。",
  loggedOutManualWhitelist: "目前瀏覽器會話已退出。管理員白名單仍然有效。",
  loggedOutLocalExempt: "目前瀏覽器會話已退出。目前網路仍屬於免白名單範圍。",
  loggedOutDefault: "目前瀏覽器會話已退出，請重新驗證。",
  redirectLoopBlocked:
    "偵測到驗證頁與目標服務之間發生重複跳轉，已暫停自動跳轉。請在此頁重新驗證，驗證成功後將繼續存取目標服務。",
  redirectTargetBlocked:
    "登入跳轉目標無效或指向目前驗證頁，已阻止重複跳轉。請重新開啟原服務，或聯絡管理員檢查登入回跳設定。",
  retrySuffix: "，請在 {seconds} 秒後重試",
  invalidOtpLength: "請輸入完整的 6 位身份驗證碼",
  loginFailed: "驗證失敗，請重試",
  passkeyNoResponse: "未取得 Passkey 回應",
  passkeyVerifyFailed: "Passkey 驗證失敗",
  passkeyLoginFailed: "Passkey 登入失敗，請重試",
  oidcStartFailed: "無法發起外部登入",
  oidcLoginFailed: "外部登入失敗，請重試",
  passkeyBindInvalid: "綁定憑證無效，請重新登入",
  passkeyBindFailed: "Passkey 綁定失敗",
  home: {
    statusTitles: {
      browserSession: "目前瀏覽器會話已驗證",
      sessionMigration: "瀏覽器會話已恢復",
      fnosFingerprintSession: "裝置指紋會話已恢復",
      manualWhitelist: "白名單訪問已放行",
      localExempt: "目前網路已放行",
      fnosShare: "分享訪問已授權",
      loginIpGrant: "安全驗證已通過",
    },
    statusDescriptions: {
      browserSession: "目前瀏覽器會話已被允許訪問",
      sessionMigration: "目前瀏覽器會話已隨網路切換恢復訪問",
      fnosFingerprintSession: "目前訪問已由飛牛裝置指紋會話恢復",
      manualWhitelist: "目前 IP 已在管理員白名單中",
      localExempt: "目前網路地址屬於免白名單範圍",
      fnosShare: "目前訪問由飛牛分享鏈路授權",
      loginIpGrant: "您的 IP 已被授權訪問",
    },
    logoutHints: {
      browserSession:
        "如果不再需要訪問，請點擊下方按鈕登出。登出後目前瀏覽器需要重新驗證才能再次進入。",
      sessionMigration:
        "如果不再需要訪問，請點擊下方按鈕登出。登出後目前瀏覽器需要重新驗證，並會撤銷本次會話遷移關聯的授權。",
      fnosFingerprintSession:
        "如果不再需要訪問，請點擊下方按鈕登出。登出後目前恢復的裝置指紋會話會結束，並撤銷關聯授權。",
      loginIpGrant:
        "如果不再需要訪問，請點擊下方按鈕登出。登出後目前瀏覽器會話會結束，登入時授予的目前 IP 訪問權限也會一併撤銷。",
      manualWhitelist:
        "如果不再需要訪問，請點擊下方按鈕登出。登出只會結束目前瀏覽器會話，管理員白名單不會被移除。",
      localExempt:
        "如果不再需要訪問，請點擊下方按鈕登出。登出只會結束目前瀏覽器會話，免白名單網路訪問範圍不會改變。",
      fnosShare:
        "如果不再需要訪問，請點擊下方按鈕登出。登出後目前分享訪問會話會結束，需要重新進入分享鏈路。",
      default: "如果不再需要訪問，請點擊下方按鈕登出並撤銷您的授權。",
    },
    logoutDialogDescriptions: {
      browserSession:
        "登出後將結束目前瀏覽器會話，需要重新驗證後才能再次進入。",
      sessionMigration:
        "登出後將結束目前瀏覽器會話，並撤銷本次會話遷移關聯的授權。",
      fnosFingerprintSession:
        "登出後將結束目前恢復的裝置指紋會話，並撤銷關聯授權。",
      loginIpGrant:
        "登出後將結束目前瀏覽器會話，並撤銷這次登入授予的目前 IP 訪問權限。",
      manualWhitelist:
        "登出後只會結束目前瀏覽器會話，管理員配置的白名單不會被移除。",
      localExempt:
        "登出後只會結束目前瀏覽器會話，目前網路的免白名單屬性不會改變。",
      fnosShare:
        "登出後將結束目前分享訪問會話，如需再次訪問請重新進入分享鏈路。",
      default: "登出後將撤銷目前訪問授權，需要重新驗證後才能再次進入。",
    },
    enablePasskey: "開啟 Passkey 一鍵登入",
    passkeySupportedUnbound: "目前瀏覽器支援 Passkey，但尚未綁定",
    logoutDelay: "登出按鈕將在 {seconds} 秒後顯示",
    logout: "登出",
    logoutConfirmTitle: "確認登出",
    confirmLogout: "確認登出",
    passkeyTokenMissing: "無法取得綁定憑證",
  },
  oidcBind: {
    title: "綁定外部帳號",
    checkingInvite: "正在檢查邀請連結...",
    bindTo: "綁定到",
    useProvider: "使用 {provider} 綁定",
    invalidInvite: "邀請連結不可用",
    wait: "請稍候",
    selectProvider: "選擇一個提供商完成登入並綁定",
    missingToken: "邀請連結缺少 token",
    noProviders: "目前沒有可用的外部登入提供商",
    inviteExpired: "邀請連結已失效",
    startFailed: "無法發起外部帳號綁定",
    bindFailed: "外部帳號綁定失敗，請重試",
  },
};
