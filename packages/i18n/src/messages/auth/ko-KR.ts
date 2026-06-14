export const koKRAuth = {
  autoIpGrantComment: "로그인 후 자동 승인됨",
  title: "보안 검증",
  captchaFirst: "먼저 아래 사람의 확인을 완료하세요.",
  otpPrompt: "로그인하려면 6자리 일회용 비밀번호를 입력하세요.",
  notRobot: "나는 로봇이 아니다",
  verified: "확인됨",
  verifying: "확인 중...",
  wait: "기다려 주세요...",
  verifyError: "확인 오류",
  turnstileMissing:
    "Turnstile가 구성되지 않았습니다. 사이트 키 설정은 관리자에게 문의하세요.",
  turnstileScriptLoadFailed: "Turnstile 스크립트를 로드하지 못했습니다.",
  turnstileRenderFailed:
    "Turnstile을 렌더링하지 못했습니다. 나중에 다시 시도하세요.",
  turnstileTimeout: "회전식 문 확인 시간이 초과되었습니다. 다시 시도해 보세요.",
  powUnsupportedAlgorithm: "지원되지 않는 PoW 알고리즘",
  powInvalidChallenge: "잘못된 PoW 챌린지 데이터",
  powSolveFailed:
    "PoW 해결에 실패했습니다. 페이지를 새로 고치고 다시 시도하세요.",
  locationResolving: "위치 확인 중...",
  locationUnavailable: "위치를 알 수 없음",
  openGithub: "GitHub 프로젝트 페이지 열기",
  menu: "메뉴",
  or: "또는",
  loginWithProvider: "{provider}으로 로그인",
  retryAfterSeconds: "{seconds}에서 재시도",
  verifyNow: "지금 확인",
  passkeyLogin: "암호키로 로그인",
  tip: "공지사항",
  ok: "알았어",
  rememberMe: "나를 기억해",
  passkeyBindTitle: "패스키 로그인 활성화",
  passkeyBindDescription:
    "다음에 한 번의 작업으로 로그인할 수 있도록 이 장치에 암호 키를 바인딩하세요.",
  passkeyBindSkipPrompt: "다시 알림 안함",
  passkeyBindLater: "어쩌면 나중에",
  passkeyBindNow: "지금 활성화",
  captchaConfigLoadFailed:
    "보안문자 구성을 로드하지 못했습니다. 페이지를 새로 고치고 다시 시도하세요.",
  captchaFailed: "사람이 직접 확인하지 못했습니다. 다시 시도해 주세요.",
  loggedOutLoginIpGrant:
    "브라우저 세션이 로그아웃되었습니다. 로그인 시 부여된 IP 액세스도 취소되었습니다.",
  loggedOutManualWhitelist:
    "브라우저 세션이 로그아웃되었습니다. 관리자 화이트리스트는 아직 활성화되어 있습니다.",
  loggedOutLocalExempt:
    "브라우저 세션이 로그아웃되었습니다. 이 네트워크는 여전히 허용 목록 확인에서 제외됩니다.",
  loggedOutDefault: "브라우저 세션이 로그아웃되었습니다. 다시 확인하세요.",
  retrySuffix: " {seconds} 초 후에 다시 시도하세요.",
  invalidOtpLength: "전체 6자리 인증 코드를 입력하세요.",
  loginFailed: "확인에 실패했습니다. 다시 시도해 주세요.",
  passkeyNoResponse: "암호 키 응답이 반환되지 않았습니다.",
  passkeyVerifyFailed: "비밀번호 확인 실패",
  passkeyLoginFailed: "패스키 로그인에 실패했습니다. 다시 시도해 주세요.",
  oidcStartFailed: "외부 로그인을 시작할 수 없습니다.",
  oidcLoginFailed: "외부 로그인에 실패했습니다. 다시 시도해 주세요.",
  passkeyBindInvalid: "바인딩 자격 증명이 잘못되었습니다. 다시 로그인하세요.",
  passkeyBindFailed: "패스키 바인딩 실패",
  home: {
    statusTitles: {
      browserSession: "이 브라우저 세션이 확인되었습니다.",
      sessionMigration: "브라우저 세션이 복원되었습니다.",
      fnosFingerprintSession: "기기 지문 세션이 복원되었습니다.",
      manualWhitelist: "화이트리스트 액세스가 허용됨",
      localExempt: "현재 네트워크가 허용됨",
      fnosShare: "공유 액세스 승인됨",
      loginIpGrant: "보안 인증 통과",
    },
    statusDescriptions: {
      browserSession: "이 브라우저 세션에는 액세스가 허용됩니다.",
      sessionMigration: "이 브라우저 세션은 네트워크 변경 후 복원되었습니다.",
      fnosFingerprintSession:
        "이 액세스는 FNOS 기기 지문 세션에 의해 복원되었습니다.",
      manualWhitelist: "현재 IP는 관리자 화이트리스트에 있습니다",
      localExempt: "이 네트워크 주소는 허용 목록 확인에서 제외됩니다.",
      fnosShare: "이 액세스는 FNOS 공유 링크에 의해 승인되었습니다.",
      loginIpGrant: "귀하의 IP는 액세스 권한이 부여되었습니다",
    },
    logoutHints: {
      browserSession:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하세요. 이 브라우저는 입력하기 전에 다시 확인해야 합니다.",
      sessionMigration:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하세요. 이 브라우저는 다시 확인해야 하며 이 세션 이전과 관련된 승인이 취소됩니다.",
      fnosFingerprintSession:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하세요. 복원된 기기 지문 세션이 종료되고 연결된 승인이 취소됩니다.",
      loginIpGrant:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하세요. 이 브라우저 세션이 종료되고 로그인 시 부여된 현재 IP 액세스도 취소됩니다.",
      manualWhitelist:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하세요. 이 브라우저 세션만 종료됩니다. 관리자 화이트리스트는 그대로 유지됩니다.",
      localExempt:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하세요. 이 브라우저 세션만 종료됩니다. 이 네트워크의 허용 목록 예외는 변경되지 않습니다.",
      fnosShare:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하세요. 이 공유 액세스 세션이 종료되므로 공유 링크를 다시 열어야 합니다.",
      default:
        "더 이상 액세스할 필요가 없으면 아래에서 로그아웃하고 승인을 취소하세요.",
    },
    logoutDialogDescriptions: {
      browserSession:
        "로그아웃하면 이 브라우저 세션이 종료됩니다. 입장 전 다시 한번 확인을 하셔야 합니다.",
      sessionMigration:
        "로그아웃하면 이 브라우저 세션이 종료되고 이 세션 이전과 관련된 승인이 취소됩니다.",
      fnosFingerprintSession:
        "로그아웃하면 복원된 장치 지문 세션이 종료되고 연결된 승인이 취소됩니다.",
      loginIpGrant:
        "로그아웃하면 이 브라우저 세션이 종료되고 이 로그인으로 부여된 현재 IP 액세스가 취소됩니다.",
      manualWhitelist:
        "로그아웃하면 이 브라우저 세션만 종료됩니다. 관리자 허용 목록은 그대로 유지됩니다.",
      localExempt:
        "로그아웃하면 이 브라우저 세션만 종료됩니다. 이 네트워크의 허용 목록 예외는 변경되지 않습니다.",
      fnosShare:
        "로그아웃하면 이 공유 액세스 세션이 종료됩니다. 나중에 액세스하려면 공유 링크를 다시여세요.",
      default:
        "로그아웃하면 현재 액세스 권한이 취소됩니다. 입장 전 다시 한번 확인을 하셔야 합니다.",
    },
    enablePasskey: "패스키 로그인 활성화",
    passkeySupportedUnbound:
      "이 브라우저는 Passkey를 지원하지만 아직 바인딩된 것은 없습니다.",
    logoutDelay: "{seconds} 초 후에 로그아웃 버튼이 나타납니다.",
    logout: "로그아웃",
    logoutConfirmTitle: "로그아웃 확인",
    confirmLogout: "로그아웃",
    passkeyTokenMissing: "바인딩 자격 증명을 가져올 수 없습니다.",
  },
  oidcBind: {
    title: "외부 계정 바인딩",
    checkingInvite: "초대 링크 확인 중...",
    bindTo: "바인딩 대상",
    useProvider: "{provider}으로 바인딩",
    invalidInvite: "초대 링크를 사용할 수 없습니다.",
    wait: "기다려주세요",
    selectProvider: "로그인하고 바인딩할 제공업체를 선택하세요.",
    missingToken: "초대 링크에 토큰이 없습니다.",
    noProviders: "외부 로그인 제공업체를 사용할 수 없습니다.",
    inviteExpired: "초대 링크가 만료되었습니다",
    startFailed: "외부 계정 바인딩을 시작할 수 없습니다.",
    bindFailed: "외부 계정 바인딩에 실패했습니다. 다시 시도해 주세요.",
  },
};
