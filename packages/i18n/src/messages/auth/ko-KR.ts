export const koKRAuth = {
  autoIpGrantComment: "로그인 후 자동으로 허용됨",
  title: "보안 인증",
  captchaFirst: "먼저 아래의 사람 확인을 완료하세요.",
  otpPrompt: "로그인하려면 6자리 일회용 비밀번호를 입력하세요.",
  passwordPrompt: "사용자 이름과 비밀번호를 입력하여 로그인하세요.",
  ldapPrompt: "LDAP 계정과 비밀번호를 입력하여 로그인하세요.",
  notRobot: "로봇이 아닙니다",
  verified: "인증됨",
  verifying: "인증 중...",
  wait: "기다려 주세요...",
  verifyError: "인증 오류",
  turnstileMissing:
    "Turnstile이 설정되지 않았습니다. 관리자에게 사이트 키 설정을 요청하세요.",
  turnstileScriptLoadFailed: "Turnstile 스크립트를 불러오지 못했습니다.",
  turnstileRenderFailed:
    "Turnstile을 렌더링하지 못했습니다. 나중에 다시 시도하세요.",
  turnstileTimeout: "Turnstile 인증 시간이 초과되었습니다. 다시 시도하세요.",
  powUnsupportedAlgorithm: "지원되지 않는 PoW 알고리즘",
  powInvalidChallenge: "잘못된 PoW 챌린지 데이터",
  powSolveFailed:
    "PoW 연산에 실패했습니다. 페이지를 새로 고친 후 다시 시도하세요.",
  locationResolving: "위치 확인 중...",
  locationUnavailable: "위치를 알 수 없음",
  openGithub: "GitHub 프로젝트 페이지 열기",
  menu: "메뉴",
  or: "또는",
  loginWithProvider: "{provider}(으)로 로그인",
  retryAfterSeconds: "{seconds}초 후 재시도",
  verifyNow: "지금 인증",
  passwordLogin: "비밀번호로 로그인",
  totpLogin: "TOTP 로그인",
  ldapLogin: "LDAP 로그인",
  ldapProvider: "디렉터리 제공자",
  ldapProviderRequired: "디렉터리 제공자를 선택하세요",
  ldapUsername: "LDAP 사용자 이름",
  ldapPassword: "LDAP 비밀번호",
  username: "사용자 이름",
  password: "비밀번호",
  showPassword: "비밀번호 표시",
  hidePassword: "비밀번호 숨기기",
  usernamePasswordRequired: "사용자 이름과 비밀번호를 입력하세요",
  passkeyLogin: "패스키로 로그인",
  tip: "안내",
  ok: "확인",
  rememberMe: "로그인 상태 유지",
  passkeyBindTitle: "패스키 로그인 활성화",
  passkeyBindDescription:
    "다음부터 간편하게 로그인할 수 있도록 이 기기에 패스키를 등록하세요.",
  passkeyBindSkipPrompt: "다시 알리지 않기",
  passkeyBindLater: "나중에",
  passkeyBindNow: "지금 활성화",
  captchaConfigLoadFailed:
    "사람 확인 설정을 불러오지 못했습니다. 페이지를 새로 고친 후 다시 시도하세요.",
  captchaFailed: "사람 확인에 실패했습니다. 다시 시도해 주세요.",
  loggedOutLoginIpGrant:
    "브라우저 세션에서 로그아웃했습니다. 로그인할 때 허용된 IP 접근 권한도 해제되었습니다.",
  loggedOutManualWhitelist:
    "브라우저 세션에서 로그아웃했습니다. 관리자가 설정한 허용 목록은 그대로 유지됩니다.",
  loggedOutLocalExempt:
    "브라우저 세션에서 로그아웃했습니다. 이 네트워크는 계속 허용 목록 검사에서 제외됩니다.",
  loggedOutDefault: "브라우저 세션에서 로그아웃했습니다. 다시 인증하세요.",
  redirectLoopBlocked:
    "이 확인 페이지와 대상 서비스 사이에서 반복 리디렉션이 감지되어 자동 리디렉션을 일시 중지했습니다. 계속하려면 이 페이지에서 다시 확인하세요.",
  redirectTargetBlocked:
    "로그인 대상이 잘못되었거나 현재 확인 페이지를 가리켜 반복 리디렉션을 중지했습니다. 원래 서비스를 다시 열거나 관리자에게 로그인 대상 설정을 확인해 달라고 요청하세요.",
  retrySuffix: " {seconds} 초 후에 다시 시도하세요.",
  invalidOtpLength: "전체 6자리 인증 코드를 입력하세요.",
  loginFailed: "인증에 실패했습니다. 다시 시도해 주세요.",
  passkeyNoResponse: "패스키 응답이 없습니다.",
  passkeyVerifyFailed: "패스키 인증 실패",
  passkeyLoginFailed: "패스키 로그인에 실패했습니다. 다시 시도해 주세요.",
  oidcStartFailed: "외부 로그인을 시작할 수 없습니다.",
  oidcLoginFailed: "외부 로그인에 실패했습니다. 다시 시도해 주세요.",
  passkeyBindInvalid: "등록 자격 증명이 올바르지 않습니다. 다시 로그인하세요.",
  passkeyBindFailed: "패스키 등록 실패",
  passkeyCreateCancelled: "패스키 생성이 취소되었거나 시간이 초과되었습니다.",
  passkeyCreateUnavailable:
    "시스템에서 패스키를 만들 수 없습니다. 화면 잠금과 비밀번호 관리자가 활성화되어 있는지 확인한 후 다시 시도하세요.",
  passkeyAlreadyRegistered:
    "이 기기 또는 비밀번호 관리자에 이미 패스키가 있습니다. 바로 사용할 수 있습니다.",
  home: {
    statusTitles: {
      browserSession: "이 브라우저 세션이 확인되었습니다.",
      sessionMigration: "브라우저 세션이 복원되었습니다.",
      fnosFingerprintSession: "기기 식별 세션이 복원되었습니다.",
      manualWhitelist: "허용 목록으로 접근이 허용되었습니다.",
      localExempt: "현재 네트워크에서 접근할 수 있습니다.",
      fnosShare: "공유 접근이 승인되었습니다.",
      loginIpGrant: "보안 인증을 통과했습니다.",
    },
    statusDescriptions: {
      browserSession: "이 브라우저 세션에는 접근이 허용됩니다.",
      sessionMigration: "이 브라우저 세션은 네트워크 변경 후 복원되었습니다.",
      fnosFingerprintSession:
        "FNOS 기기 식별 세션을 통해 접근 권한이 복원되었습니다.",
      manualWhitelist: "현재 IP가 관리자의 허용 목록에 등록되어 있습니다.",
      localExempt: "이 네트워크 주소는 허용 목록 검사에서 제외됩니다.",
      fnosShare: "FNOS 공유 링크를 통해 접근이 승인되었습니다.",
      loginIpGrant: "현재 IP의 접근이 허용되었습니다.",
    },
    logoutHints: {
      browserSession:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하세요. 이 브라우저로 다시 접속하려면 보안 인증이 필요합니다.",
      sessionMigration:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하세요. 이 브라우저는 다시 인증해야 하며, 이전된 세션의 접근 권한도 해제됩니다.",
      fnosFingerprintSession:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하세요. 복원된 기기 식별 세션과 연결된 접근 권한이 해제됩니다.",
      loginIpGrant:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하세요. 브라우저 세션과 로그인할 때 허용된 현재 IP의 접근 권한이 해제됩니다.",
      manualWhitelist:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하세요. 브라우저 세션만 종료되며 관리자의 허용 목록은 유지됩니다.",
      localExempt:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하세요. 브라우저 세션만 종료되며 이 네트워크의 허용 목록 예외는 유지됩니다.",
      fnosShare:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하세요. 공유 접근 세션이 종료되며 다시 접속하려면 공유 링크를 열어야 합니다.",
      default:
        "접근이 더 이상 필요하지 않으면 아래에서 로그아웃하여 접근 권한을 해제하세요.",
    },
    logoutDialogDescriptions: {
      browserSession:
        "로그아웃하면 이 브라우저 세션이 종료됩니다. 다시 접속하려면 보안 인증이 필요합니다.",
      sessionMigration:
        "로그아웃하면 브라우저 세션과 이전된 세션의 접근 권한이 해제됩니다.",
      fnosFingerprintSession:
        "로그아웃하면 복원된 기기 식별 세션과 연결된 접근 권한이 해제됩니다.",
      loginIpGrant:
        "로그아웃하면 브라우저 세션과 로그인할 때 허용된 현재 IP의 접근 권한이 해제됩니다.",
      manualWhitelist:
        "로그아웃하면 브라우저 세션만 종료됩니다. 관리자의 허용 목록은 유지됩니다.",
      localExempt:
        "로그아웃하면 브라우저 세션만 종료됩니다. 이 네트워크의 허용 목록 예외는 유지됩니다.",
      fnosShare:
        "로그아웃하면 공유 접근 세션이 종료됩니다. 다시 접속하려면 공유 링크를 여세요.",
      default:
        "로그아웃하면 현재 접근 권한이 해제됩니다. 다시 접속하려면 보안 인증이 필요합니다.",
    },
    enablePasskey: "패스키 로그인 활성화",
    passkeySupportedUnbound:
      "이 브라우저는 패스키를 지원하지만 아직 등록된 패스키가 없습니다.",
    addPasskey: "다른 패스키 추가",
    passkeyAvailableAddDevice:
      "이 계정에는 이미 패스키가 있습니다. 이 기기에 동기화되지 않았다면 하나 더 추가할 수 있습니다.",
    logoutDelay: "{seconds} 초 후에 로그아웃 버튼이 나타납니다.",
    logout: "로그아웃",
    logoutConfirmTitle: "로그아웃 확인",
    confirmLogout: "로그아웃",
    passkeyTokenMissing: "등록 자격 증명을 가져올 수 없습니다.",
  },
  ldapBind: {
    title: "LDAP 계정 연결",
    description: "LDAP 신원을 확인하고 기존 TOTP 자격 증명에 연결합니다.",
    checkingInvite: "초대 링크 확인 중...",
    bindTo: "연결 대상",
    missingToken: "초대 링크에 토큰이 없습니다.",
    inviteExpired: "초대 링크가 만료되었습니다.",
    bindNow: "확인하고 연결",
    bindFailed: "LDAP 계정을 연결하지 못했습니다. 다시 시도해 주세요.",
  },
  oidcBind: {
    title: "외부 계정 연결",
    checkingInvite: "초대 링크 확인 중...",
    bindTo: "연결 대상",
    useProvider: "{provider}(으)로 연결",
    invalidInvite: "초대 링크를 사용할 수 없습니다.",
    wait: "잠시 기다려 주세요.",
    selectProvider: "로그인하여 연결할 제공자를 선택하세요.",
    missingToken: "초대 링크에 토큰이 없습니다.",
    noProviders: "사용 가능한 외부 로그인 제공자가 없습니다.",
    inviteExpired: "초대 링크가 만료되었습니다.",
    startFailed: "외부 계정 연결을 시작할 수 없습니다.",
    bindFailed: "외부 계정을 연결하지 못했습니다. 다시 시도해 주세요.",
  },
};
