export const koKRServer = {
  success: "성공",
  notFound: "찾을 수 없음",
  apiPathNotFound: "API 경로를 찾을 수 없습니다.",
  invalidLocale: "지원되지 않는 로케일",
  dockerAdminDenied:
    "Docker 관리 패널은 개인 네트워크 또는 신뢰할 수 있는 프록시 액세스만 허용합니다.",
  dockerAdminDeniedTitle: "액세스가 거부되었습니다.",
  dockerAdminDeniedDescription:
    "Docker 관리 패널은 기본적으로 호스트, LAN, VPN 또는 구성된 신뢰할 수 있는 역방향 프록시에서의 액세스만 허용합니다. 직접적인 공개 액세스는 거부됩니다.",
  dockerAdminCurrentIp: "감지된 소스 IP: {ip}",
  dockerAdminProxyRequired: "{port} 관리 항목을 통해 관리 API에 액세스합니다.",
  dockerAdminLoginRequired: "먼저 Docker 관리 패널에 로그인하세요.",
  captchaUnavailable: "보안 문자 서비스를 일시적으로 사용할 수 없습니다",
  tooManyAttempts: "시도 횟수가 너무 많습니다. 나중에 다시 시도해 주세요.",
  tooManyAttemptsWithRetry:
    "시도 횟수가 너무 많습니다. {seconds}초 후에 다시 시도해 주세요.",
  loginCredentialMissing: "서버에 로그인 자격 증명이 구성되어 있지 않습니다.",
  invalidOtpWithRetry:
    "인증코드가 올바르지 않습니다. {seconds}초 후에 다시 시도하세요.",
  invalidPasswordWithRetry:
    "사용자 이름 또는 비밀번호가 올바르지 않습니다. {seconds}초 후 다시 시도하세요.",
  runtimeProfile: {
    capabilities: {
      default: "현재 런타임은 이 기능을 지원하지 않습니다.",
      direct_mode_available: {
        docker: "Docker 배포는 호스트 직접 방화벽 모드를 지원하지 않습니다.",
        platform: "현재 런타임은 호스트 직접 방화벽 모드를 지원하지 않습니다.",
        permission: "현재 프로세스에는 호스트 직접 방화벽 기능이 없습니다.",
      },
      host_firewall_available: {
        docker: "Docker 배포는 호스트 방화벽 관리를 지원하지 않습니다.",
        platform: "현재 런타임은 호스트 방화벽 관리를 지원하지 않습니다.",
        permission: "현재 프로세스에는 호스트 방화벽 관리 기능이 없습니다.",
      },
      smart_connect_available: {
        docker:
          "Docker 배포는 아직 Smart Connect를 지원하지 않습니다. 호스트 dnsmasq 및 포트 53에 따라 다릅니다.",
        platform: "현재 런타임은 아직 Smart Connect를 지원하지 않습니다.",
        permission:
          "현재 프로세스에는 Smart Connect에 필요한 호스트 관리 기능이 없습니다.",
      },
      fnos_certificate_sync_available: {
        docker: "Docker 배포는 FNOS SSL 인증서 동기화를 지원하지 않습니다",
        platform: "FNOS SSL 인증서 동기화는 FPK 배포에서만 사용할 수 있습니다",
        permission:
          "현재 프로세스에 FNOS SSL 인증서 동기화에 필요한 root 권한이 없습니다",
      },
      system_clock_sync_available: {
        docker: "Docker 배포는 호스트 시스템 시간 동기화를 지원하지 않습니다.",
        platform: "현재 런타임은 시스템 시간 동기화를 지원하지 않습니다.",
        permission:
          "현재 프로세스에는 시스템 시간 동기화에 필요한 호스트 권한이 없습니다.",
      },
      self_update_available: {
        docker:
          "Docker 배포는 인앱 FPK 업데이트를 지원하지 않습니다. 새 이미지를 가져와 업그레이드",
        openwrt:
          "OpenWrt 배포는 인앱 FPK 업데이트를 지원하지 않습니다. 장치 아키텍처에 맞는 IPK를 opkg로 설치해 업그레이드하세요.",
        deployment: "현재 배포 유형은 인앱 업데이트를 지원하지 않습니다.",
      },
      terminal_available: {
        docker: "Docker 배포는 웹 터미널을 지원하지 않습니다.",
        openwrt: "OpenWrt 배포는 아직 웹 터미널을 지원하지 않습니다.",
        platform: "현재 런타임은 웹 터미널을 지원하지 않습니다.",
      },
      shared_root_available: {
        missing: "현재 런타임에서는 공유 디렉터리 마운트를 사용할 수 없습니다.",
      },
    },
  },
  systemClock: {
    unknown: "알 수 없음",
    actionSeparator: "; ",
    listSeparator: ", ",
    duration: {
      seconds: "{seconds}",
      minutes: "{minutes} 분",
      minutesSeconds: "{minutes} 최소 {seconds}",
    },
    networkCheckFailed: "온라인으로 시스템 시간을 확인하지 못했습니다.",
    issues: {
      timezone: {
        title: "시스템 시간대가 베이징 시간이 아닙니다.",
        message:
          "현재 시스템 시간대는 {timezone}입니다. {expected}이어야 합니다.",
      },
      timeMismatch: {
        title: "시스템 시간이 온라인 확인과 다름",
        message: "시스템 시간은 온라인 확인과 {drift} 정도 다릅니다.",
      },
    },
    statusRefreshed: "시스템 시간 상태가 새로 고쳐졌습니다.",
    syncFailed: "시스템 시간 동기화 실패",
    networkTimeUnavailable: "네트워크에서 표준시를 가져올 수 없습니다.",
    sourceFetchFailed: "{source}에서 시간을 가져오지 못했습니다.",
    missingDateHeader:
      "{source}이 사용 가능한 날짜 헤더를 반환하지 않았습니다.",
    invalidDateHeader: "{source}이 구문 분석할 수 없는 시간을 반환했습니다.",
    commandFailed: "{command}을 실행하지 못했습니다.",
    timezoneSet: "시스템 시간대를 {timezone}으로 설정하세요.",
    missingZoneinfoFile: "시스템 시간대 파일이 누락되었습니다: {path}",
    timezoneWritten: "시스템 시간대 {timezone}을 작성했습니다.",
    clockAdjusted: "시스템 시간이 조정되었습니다.",
    ntpEnabled: "자동 NTP 시간 동기화 활성화",
    serviceRestarted: "{service}을 다시 시작했습니다.",
  },
  updateRoutes: {
    downloadStarted: "업데이트 패키지 다운로드가 시작되었습니다.",
    downloadStartFailed: "다운로드를 시작하지 못했습니다.",
    installStarted: "업데이트 설치가 시작되었습니다.",
    installStartFailed: "설치를 시작하지 못했습니다.",
    checkAndDownloadStarted:
      "업데이트 확인이 시작되었으며 다운로드가 대기 중입니다.",
    startFailed: "시작하지 못했습니다.",
    loadStatusFailed: "업데이트 상태를 불러오지 못했습니다.",
    loadConfirmationFailed: "업데이트 확인 정보를 불러오지 못했습니다.",
  },
  gatewayHostResponse: {
    runTypes: {
      direct: "직접 모드",
      reverseProxy: "역방향 프록시 모드",
      subdomain: "하위 도메인 모드",
    },
    unavailableReason:
      "하위 도메인 모드만 사용할 수 있습니다. 현재 모드: {mode}.",
    editSubdomainOnly:
      "호스트 응답은 하위 도메인 매핑 모드에서만 편집할 수 있습니다.",
    syncFailed: "게이트웨이 호스트 응답 구성을 동기화하지 못했습니다.",
    hostRoutesSyncFailed: "호스트 경로를 동기화하지 못했습니다.",
    updateFailed: "게이트웨이 호스트 응답을 업데이트하지 못했습니다.",
    updateFailedRolledBack:
      "게이트웨이 호스트 응답을 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
    updateFailedRollbackFailed: "{error}; 롤백 실패: {rollbackError}",
    restoreConfigFailed: "호스트 응답 구성을 복원하지 못했습니다.",
    restoreRuntimeFailed: "호스트 응답 런타임 상태를 복원하지 못했습니다.",
    restoreGatewayRuntimeFailed:
      "게이트웨이 호스트 응답 런타임 상태를 복원하지 못했습니다.",
  },
  admin: {
    runTypes: {
      direct: "직접 모드",
      reverseProxy: "역방향 프록시 모드",
      subdomain: "하위 도메인 모드",
    },
    validation: {
      required: "{label}이 필요합니다",
      httpUrlRequired: "{label}은 http:// 또는 https://로 시작해야 합니다.",
      proxyTargetUrlRequired:
        "{label}은 http://, https://, ws:// 또는 wss://로 시작하고 호스트를 포함해야 합니다.",
      invalidFormat: "{label} 형식이 잘못되었습니다.",
    },
    rollback: {
      failed: "{message}; 롤백 실패: {rollbackError}",
      restoreConfigFailed: "이전 구성을 복원하지 못했습니다.",
      restoreSmartConnectFailed:
        "이전 Smart Connect 런타임 상태를 복원하지 못했습니다.",
      restoreRuntimeFailed: "이전 런타임 상태를 복원하지 못했습니다.",
      restoreProtocolConfigFailed: "프로토콜 매핑 구성을 복원하지 못했습니다.",
      restoreProtocolFeatureFailed:
        "프로토콜 매핑 기능 스위치를 복원하지 못했습니다.",
      restoreProtocolRuntimeFailed:
        "프로토콜 매핑 런타임 상태를 복원하지 못했습니다.",
      restoreVisibilityConfigFailed: "공개 상태 구성을 복원하지 못했습니다.",
      restoreVisibilityRuntimeFailed:
        "가시성 런타임 CIDR을 복원하지 못했습니다.",
      restoreGatewayVisibilityFailed:
        "게이트웨이 가시성 런타임 상태를 복원하지 못했습니다.",
      restoreProxyHeadersConfigFailed:
        "프록시 헤더 구성을 복원하지 못했습니다.",
      restoreProxyHeadersRuntimeFailed:
        "프록시 헤더 런타임 상태를 복원하지 못했습니다.",
      restoreGatewayProxyHeadersRuntimeFailed:
        "게이트웨이 프록시 헤더 런타임 상태를 복원하지 못했습니다.",
      restorePortalFailed: "포털 디스플레이 런타임 상태를 복원하지 못했습니다.",
    },
    dockerPanel: {
      passwordNotNeeded:
        "현재 실행 모드에는 Docker 관리자 패널 비밀번호가 필요하지 않습니다.",
      setPasswordFailed: "관리자 패널 비밀번호를 설정하지 못했습니다.",
      passwordChangeUnsupported:
        "현재 실행 모드는 Docker 관리자 패널 비밀번호 변경을 지원하지 않습니다.",
      changePasswordFailed: "관리자 패널 비밀번호를 변경하지 못했습니다.",
      tooManyAttemptsWithRetry:
        "시도 횟수가 너무 많습니다. {seconds} 초 후에 다시 시도하세요.",
      tooManyAttempts: "시도 횟수가 너무 많습니다. 나중에 다시 시도하세요.",
      passwordSetupRequired:
        "관리자 패널 비밀번호가 설정되지 않았습니다. 먼저 최초 설정을 완료하세요.",
      passwordIncorrectWithRetry:
        "관리자 패널 비밀번호가 올바르지 않습니다. {seconds} 초 후에 다시 시도하세요.",
    },
    adminPanelRoutes: {
      signInRequired: "먼저 관리자 패널에 로그인하세요.",
      verifySessionFailed: "관리자 패널 세션을 확인하지 못했습니다.",
      loadStateFailed: "관리자 패널 상태를 불러오지 못했습니다.",
      loadConfigFailed: "구성을 불러오지 못했습니다.",
      loadLocaleFailed: "언어 설정을 불러오지 못했습니다.",
      loadAppearanceFailed: "외관 설정을 불러오지 못했습니다.",
      saveLocaleFailed: "언어 설정을 저장하지 못했습니다.",
      saveAppearanceFailed: "외관 설정을 저장하지 못했습니다.",
      loadPasswordFailed: "관리자 패널 비밀번호를 불러오지 못했습니다.",
      createSessionFailed: "관리자 패널 세션을 생성하지 못했습니다.",
      verifyPasswordFailed: "관리자 패널 비밀번호를 확인하지 못했습니다.",
      checkLoginRateLimitFailed: "로그인 빈도 제한을 확인하지 못했습니다.",
    },
    runType: {
      switchFailed: "실행 모드를 전환하지 못했습니다.",
      switchFailedRolledBack:
        "실행 모드를 전환하지 못했습니다. 구성이 롤백되었습니다.",
    },
    firewall: {
      whitelistSynced: ", 동기화된 {count} 화이트리스트 IP",
      exemptPorts: ", 진입 포트 {ports} 유지",
      resetSuccess:
        "{runType}{whitelistMessage}{exemptPortsMessage}에 대한 방화벽 재설정",
      resetFailed: "방화벽을 재설정하지 못했습니다.",
      clearSuccess:
        "방화벽 규칙이 지워지고 {port} 포트에 대한 기록 리디렉션이 제거되었습니다.",
      clearFailed: "방화벽을 지우지 못했습니다.",
    },
    protocolMapping: {
      subdomainOnly:
        "프로토콜 매핑은 하위 도메인 모드에서만 활성화할 수 있습니다.",
      updateFeatureFailed:
        "프로토콜 매핑 기능 스위치를 업데이트하지 못했습니다.",
      updateFeatureFailedRolledBack:
        "프로토콜 매핑 기능 스위치를 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
    },
    smartConnect: {
      subdomainOnly:
        "Smart Connect는 하위 도메인 모드에서만 활성화할 수 있습니다.",
      updateFailed: "Smart Connect를 업데이트하지 못했습니다.",
      updateFailedRolledBack:
        "Smart Connect를 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
    },
    fnosPortIcon: {
      syncFailed:
        "FNOS 포트 아이콘 하이재킹 구성을 게이트웨이에 동기화하지 못했습니다.",
    },
    fnosNetworkTuning: {
      unavailable:
        "현재 런타임은 FNOS FPK 네트워크 최적화를 지원하지 않습니다.",
      updateFailed: "FNOS FPK 네트워크 최적화를 업데이트하지 못했습니다.",
      errors: {
        bbrNotSupported: "호스트 커널이 tcp_bbr을 제공하지 않습니다.",
        bbrEnableVerificationFailed:
          "BBR 활성화를 요청했지만 현재 커널 상태가 bbr/fq가 아닙니다.",
        bbrRollbackCongestionFailed:
          "BBR 롤백이 이전 혼잡 제어 값을 복원하지 못했습니다.",
        bbrRollbackQdiscFailed:
          "BBR 롤백이 이전 기본 큐 규칙을 복원하지 못했습니다.",
        bbrRollbackStillBbrFailed:
          "BBR 롤백 후에도 혼잡 제어가 bbr 상태입니다.",
        mtuEnableVerificationFailed:
          "MTU probing 활성화를 요청했지만 tcp_mtu_probing이 1이 아닙니다.",
        mtuRollbackFailed: "MTU probing 롤백이 예상 값을 복원하지 못했습니다.",
        emptyPatch: "FNOS FPK 네트워크 최적화 옵션을 하나 이상 변경하세요.",
        setSysctlFailed: "{key} 설정에 실패했습니다.",
        rollbackFailed: "{message}; 롤백 실패: {error}",
      },
      blocked: {
        deployment:
          "FNOS FPK 네트워크 최적화는 FPK 배포에서만 사용할 수 있습니다.",
        platform: "FNOS FPK 네트워크 최적화에는 Linux 호스트가 필요합니다.",
        permission: "FNOS FPK 네트워크 최적화에는 root 권한이 필요합니다.",
      },
    },
    gateway: {
      syncAuthCacheFailed:
        "인증 캐시 구성을 게이트웨이에 동기화하지 못했습니다.",
      syncThrottleFailed:
        "게이트웨이 제한 구성을 게이트웨이에 동기화하지 못했습니다.",
      syncCrawlerBlockerFailed:
        "크롤러 차단 구성을 게이트웨이에 동기화하지 못했습니다.",
      updateFailed: "게이트웨이 구성을 업데이트하지 못했습니다.",
      updateFailedRolledBack:
        "게이트웨이 구성을 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
    },
    proxyMappings: {
      payloadObjectRequired: "경로 프록시 매핑은 객체여야 합니다.",
      targetInvalid:
        "경로 프록시 대상은 http://, https://, ws:// 또는 wss://로 시작하고 호스트를 포함해야 합니다.",
      syncRulesFailed: "경로 프록시 라우트를 동기화하지 못했습니다.",
      restoreRulesFailed: "경로 프록시 라우트를 복원하지 못했습니다.",
      updateFailed: "경로 프록시 매핑을 업데이트하지 못했습니다.",
      updateFailedRolledBack:
        "경로 프록시 매핑을 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
    },
    gatewayVisibility: {
      updateFailed: "게이트웨이 공개 상태를 업데이트하지 못했습니다.",
      updateFailedRolledBack:
        "게이트웨이 공개 상태를 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
    },
    gatewayProxyHeaders: {
      subdomainOnly:
        "프록시 헤더는 하위 도메인 매핑 모드에서만 편집할 수 있습니다.",
      updateFailed: "게이트웨이 프록시 헤더를 업데이트하지 못했습니다.",
      updateFailedRolledBack:
        "게이트웨이 프록시 헤더를 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
    },
    gatewaySettingsRoutes: {
      loadGatewaySettingsFailed: "게이트웨이 설정을 불러오지 못했습니다.",
      payloadObjectRequired: "게이트웨이 요청 내용은 객체여야 합니다.",
      loadConfigFailed: "구성을 불러오지 못했습니다.",
      saveGatewaySettingsFailed: "게이트웨이 설정을 저장하지 못했습니다.",
      syncGatewaySettingsFailed: "게이트웨이 설정 동기화 실패: {message}",
      responseReloadFailed:
        "게이트웨이 설정은 저장되었지만 응답을 다시 불러오지 못했습니다.",
      loadGatewayVisibilityFailed:
        "게이트웨이 표시 상태를 불러오지 못했습니다.",
      loadRuntimeFailed: "런타임 상태를 불러오지 못했습니다.",
      loadGatewayProxyHeadersFailed:
        "게이트웨이 프록시 헤더를 불러오지 못했습니다.",
      loadGatewayHostResponseFailed:
        "게이트웨이 Host 응답을 불러오지 못했습니다.",
    },
    runtimeConfigRoutes: {
      loadCaptchaFailed: "캡차 설정을 불러오지 못했습니다.",
      saveCaptchaFailed: "캡차 설정을 저장하지 못했습니다.",
      loadTerminalFeatureFailed: "터미널 기능 설정을 불러오지 못했습니다.",
      saveTerminalFeatureFailed: "터미널 기능 설정을 저장하지 못했습니다.",
      invalidRunType: "run_type이 올바르지 않습니다.",
      loadProtocolMappingFeatureFailed:
        "프로토콜 매핑 기능 설정을 불러오지 못했습니다.",
      loadSmartConnectDetailsFailed:
        "Smart Connect 세부 정보를 불러오지 못했습니다.",
      loadFnosShareBypassFailed: "FNOS 공유 우회 설정을 불러오지 못했습니다.",
      saveFnosShareBypassFailed: "FNOS 공유 우회 설정을 저장하지 못했습니다.",
      loadFnosPortIconHijackFailed:
        "FNOS 포트 아이콘 하이재킹 설정을 불러오지 못했습니다.",
      loadAutoHttpsFailed: "자동 HTTPS 설정을 불러오지 못했습니다.",
      saveAutoHttpsFailed: "자동 HTTPS 설정을 저장하지 못했습니다.",
      saveAutoManageFirewallFailed:
        "방화벽 자동 관리 설정을 저장하지 못했습니다.",
      loadConfigFailed: "구성을 불러오지 못했습니다.",
      loadDefaultRouteFailed: "기본 라우트를 불러오지 못했습니다.",
      saveDefaultRouteFailed: "기본 라우트를 저장하지 못했습니다.",
      unsupportedTunnelType: "지원하지 않는 터널 유형입니다.",
      saveDefaultTunnelFailed: "기본 터널을 저장하지 못했습니다.",
      upstreamUnavailable: "업스트림 서비스를 사용할 수 없습니다.",
      proxyProtocolForceBooleanRequired:
        "proxy_protocol_force는 불리언이어야 합니다.",
      loadRunModePromptPreferencesFailed:
        "실행 모드 안내 기본 설정을 불러오지 못했습니다.",
      saveRunModePromptPreferencesFailed:
        "실행 모드 안내 기본 설정을 저장하지 못했습니다.",
      loadWelcomeGuideFailed: "환영 가이드 상태를 불러오지 못했습니다.",
      saveWelcomeGuideFailed: "환영 가이드 상태를 저장하지 못했습니다.",
    },
    captcha: {
      turnstileKeysRequired:
        "Cloudflare Turnstile이 활성화되면 site_key와 secret_key가 모두 필요합니다.",
    },
    ipLocation: {
      ipLookupUrlLabel: "IP 조회 데이터베이스 URL",
      cidrUrlLabel: "CIDR 데이터베이스 URL",
      loadSettingsFailed: "IP 위치 API 설정을 불러오지 못했습니다.",
      saveSettingsFailed: "IP 위치 API 설정을 저장하지 못했습니다.",
      modeInvalid: "모드는 online 또는 custom이어야 합니다.",
    },
    connectionTest: {
      httpStatus: "서비스가 HTTP 상태 {status}을 반환했습니다.",
      invalidData: "서비스가 잘못된 데이터를 반환했습니다.",
      success: "연결 성공",
      timeout: "연결 시간이 초과되었습니다.",
      failed: "연결 실패",
    },
    autoHttps: {
      dockerUnsupported: "Docker 빌드에서는 자동 HTTPS가 지원되지 않습니다.",
      openWrtUnsupported: "OpenWrt 빌드에서는 자동 HTTPS가 지원되지 않습니다.",
      startFailed: "자동 HTTPS를 시작하지 못했습니다.",
    },
    hostMappings: {
      payloadObjectRequired: "호스트 매핑은 객체여야 합니다.",
      hostRequired: "호스트 매핑에는 도메인이 필요합니다.",
      duplicateHost: "호스트 매핑 도메인 {host}이(가) 중복되었습니다.",
      protocolModeInvalid:
        "호스트 매핑 {host}의 HTTPS 프로토콜은 auto, http1 또는 http2여야 합니다.",
      backendProtocolUnsupported:
        "게이트웨이 백엔드가 {host}의 HTTPS 프로토콜 {mode}을(를) 적용하지 못했습니다. 게이트웨이 백엔드를 업그레이드하세요.",
      visibilityInvalid:
        "호스트 매핑 {host}의 가시성 설정이 잘못되었습니다: {message}",
      backendVisibilityUnsupported:
        "게이트웨이 백엔드가 {host}의 가시성 규칙을 적용하지 못했습니다. 게이트웨이 백엔드를 업그레이드하세요.",
      revisionConflict:
        "다른 페이지에서 호스트 매핑이 업데이트되었습니다. 새로 고친 후 다시 시도하세요.",
      targetInvalid:
        "호스트 매핑 {host} 대상은 http://, https://, ws:// 또는 wss://로 시작하고 호스트를 포함해야 합니다.",
      singleAuthPortMapping:
        "하나의 호스트 매핑만 인증 서비스로 AUTH_PORT를 가리킬 수 있습니다.",
      authMappingMustBePublic:
        "인증 서비스 {host}은 공개 상태로 유지되어야 하며 자체 인증 또는 엄격한 화이트리스트를 활성화할 수 없습니다. 그렇지 않으면 로그인 항목에 접근할 수 없게 됩니다.",
      authMappingBasicAuthForbidden:
        "인증 서비스 {host}에서 자격 증명 삽입을 활성화할 수 없습니다.",
      basicAuthInvalid:
        "호스트 매핑 {host} 자격 증명 주입에는 사용자 이름과 비밀번호가 필요하며 사용자 이름에는 콜론을 포함할 수 없습니다.",
      locationPathRequired:
        "호스트 매핑 {host} 경로 규칙에는 경로가 필요합니다.",
      locationPathMustStartSlash:
        "호스트 매핑 {host} 경로 규칙 {path}은 /로 시작해야 합니다.",
      locationRootForbidden:
        "호스트 매핑 {host}은 루트 경로/를 경로 규칙으로 사용할 수 없습니다.",
      locationReservedPath:
        "호스트 매핑 {host} 경로 규칙 {path}은 예약된 경로를 사용합니다.",
      locationDuplicate:
        "호스트 매핑 {host}에 중복된 경로 규칙 {path}이 있습니다.",
      locationTargetRequired:
        "호스트 매핑 {host} 경로 규칙 {path}에는 대상이 필요합니다.",
      locationTargetInvalid:
        "호스트 매핑 {host} 경로 규칙 {path} 대상은 http://, https://, ws:// 또는 wss://로 시작하고 호스트를 포함해야 합니다.",
      locationStatusInvalid:
        "호스트 매핑 {host} 경로 규칙 {path} 응답 상태는 100에서 599 사이여야 합니다.",
      locationHeaderInvalid:
        "호스트 매핑 {host} 경로 규칙 {path}에 잘못된 응답 헤더 {header}이 포함되어 있습니다.",
      locationHeaderForbidden:
        "호스트 매핑 {host} 경로 규칙 {path}은 응답 헤더 {header}을 사용자 지정할 수 없습니다.",
      syncHostRulesFailed: "호스트 경로를 동기화하지 못했습니다.",
      syncAuthConfigFailed: "인증 게이트웨이 구성을 동기화하지 못했습니다.",
      updateFailed: "호스트 매핑을 업데이트하지 못했습니다.",
      updateFailedRolledBack:
        "호스트 매핑을 업데이트하지 못했습니다. 구성이 롤백되었습니다.",
      metadataFailed: "대상 제목을 새로 고치지 못했습니다.",
      onlyHttpTargetsSupported: "http/https 대상만 지원됩니다.",
      metadataUpstreamStatus: "업스트림이 상태 {status}을 반환했습니다.",
      bookmarkFolderForRoot: "{root} 하위 도메인 매핑",
      bookmarkFolderDefault: "fn-knock 하위 도메인 매핑",
    },
    streamMappings: {
      payloadObjectRequired: "스트림 매핑은 객체여야 합니다.",
      listenPortRequiredInteger: "수신 포트는 유효한 정수여야 합니다.",
      listenPortNotInteger: "수신 포트 {port}은 유효한 정수가 아닙니다.",
      listenPortOutOfRange: "수신 포트 {port}이 범위를 벗어났습니다.",
      duplicatePort:
        "{protocol} 수신 포트 {port}이 중복되었습니다. 프로토콜 + 포트를 고유하게 유지하십시오.",
      targetMustBeHostPort:
        "대상 주소 {target}은 호스트:포트 형식이어야 합니다.",
      saveFailed: "프로토콜 매핑을 저장하지 못했습니다.",
      syncFailed:
        "프로토콜 매핑 및 게이트웨이 포트 허용 규칙을 동기화하지 못했습니다.",
      syncFailedRolledBack:
        "프로토콜 매핑 및 게이트웨이 포트 허용 규칙을 동기화하지 못했습니다. 구성이 롤백되었습니다.",
    },
    passkeyRp: {
      parentDomainRequired:
        "상위 도메인 패스키 RP가 활성화된 경우 루트 도메인을 입력하거나 상위 RP ID를 명시적으로 지정합니다.",
      mustMatchAuthHost:
        "상위 도메인 암호 키 RP ID {rpId}은 인증 서비스 {authHost}과 일치하거나 해당 상위 도메인이어야 합니다.",
    },
    subdomainMode: {
      payloadObjectRequired: "하위 도메인 모드 요청 내용은 객체여야 합니다.",
      saveFailed: "하위 도메인 모드 설정을 저장하지 못했습니다.",
      sslAutoSelected:
        "현재 하위 도메인 모드에 더 적합한 인증서로 자동 전환됩니다.",
      sslAutoSelectionSyncFailed:
        "권장 인증서를 찾았지만 게이트웨이와의 동기화에 실패하여 자동으로 전환되지 않았습니다.",
    },
    authMode: {
      loadFailed: "인증 로그인 모드를 불러오지 못했습니다",
      invalidMode: "지원하지 않는 로그인 모드입니다",
      previewFailed: "로그인 모드 전환 미리보기에 실패했습니다",
      switchFailed: "로그인 모드 전환에 실패했습니다",
      blockingIssues: "차단 항목이 남아 있어 로그인 모드를 전환할 수 없습니다",
    },
    authAccounts: {
      loadFailed: "인증 계정을 불러오지 못했습니다",
      notFound: "인증 계정을 찾을 수 없습니다",
      saveFailed: "인증 계정을 저장하지 못했습니다",
      syncFailed: "인증 계정을 TOTP로 동기화하지 못했습니다",
      usernameExists: "사용자 이름이 이미 존재합니다",
      usernameTooShort: "사용자 이름은 비워 둘 수 없습니다",
      usernameTooLong: "사용자 이름은 64자를 초과할 수 없습니다",
      usernameInvalid:
        "사용자 이름은 문자, 숫자, 점, 밑줄, 하이픈만 포함할 수 있으며 공백은 사용할 수 없습니다",
      passwordTooShort: "계정 비밀번호는 비워 둘 수 없습니다",
      passwordTooLong: "계정 비밀번호는 128자를 초과할 수 없습니다",
      passwordWhitespace: "계정 비밀번호에는 공백을 포함할 수 없습니다",
      passwordNeedsLettersAndNumbers:
        "계정 비밀번호에는 문자와 숫자가 모두 포함되어야 합니다",
      passwordSaveFailed: "계정 비밀번호를 저장하지 못했습니다",
      deleteFailed: "인증 계정을 삭제하지 못했습니다",
      deleted: "인증 계정이 삭제되었습니다",
      totpAlreadyBound: "계정에 이미 사용 가능한 TOTP가 바인딩되어 있습니다",
    },
    authCredentialSettings: {
      loadFailed: "인증 자격 증명 설정을 불러오지 못했습니다.",
      loadConfigFailed: "구성을 불러오지 못했습니다.",
      saveFailed: "인증 자격 증명 설정을 저장하지 못했습니다.",
    },
    totp: {
      invalidCode: "인증코드가 올바르지 않습니다. 다시 시도해 보세요.",
      invalidSecretOrCode: "TOTP 비밀 키 또는 인증 코드가 올바르지 않습니다.",
      notFound: "TOTP를 찾을 수 없습니다",
      loadFailed: "TOTP 자격 증명을 불러오지 못했습니다.",
      saveFailed: "TOTP 자격 증명을 저장하지 못했습니다.",
      exportFailed: "TOTP 자격 증명을 내보내지 못했습니다.",
      importFailed: "TOTP 자격 증명을 가져오지 못했습니다.",
      deleteFailed: "TOTP 자격 증명을 삭제하지 못했습니다.",
      updateFailed: "TOTP 자격 증명을 업데이트하지 못했습니다.",
      bound: "TOTP 자격 증명이 연결되었습니다.",
      deleted: "TOTP 자격 증명이 삭제되었습니다.",
      updated: "TOTP 자격 증명이 업데이트되었습니다.",
    },
    totpImport: {
      payloadObject: "TOTP 자격 증명 가져오기 내용은 객체여야 합니다.",
      unsupportedKind: "지원되지 않는 TOTP 자격 증명 가져오기 형식입니다.",
      unsupportedVersion: "지원되지 않는 TOTP 자격 증명 가져오기 버전입니다.",
      credentialsArray: "TOTP 자격 증명 목록은 배열이어야 합니다.",
      accountsArray: "계정 자격 증명 목록은 배열이어야 합니다.",
      passwordArray: "계정 비밀번호 자격 증명 목록은 배열이어야 합니다.",
      countExceeded:
        "한 번에 최대 {max}개의 TOTP 자격 증명을 가져올 수 있습니다.",
      accountCountExceeded:
        "한 번에 최대 {max}개의 계정 자격 증명을 가져올 수 있습니다.",
      passwordCountExceeded:
        "한 번에 최대 {max}개의 계정 비밀번호 자격 증명을 가져올 수 있습니다.",
    },
    passkeys: {
      notFound: "패스키를 찾을 수 없습니다.",
      listFailed: "패스키 목록을 불러오지 못했습니다.",
      deleteFailed: "패스키를 삭제하지 못했습니다.",
      deleted: "패스키가 삭제되었습니다.",
    },
    syncRoutes: {
      partialFailedGatewayLogging:
        "부분 동기화 실패: Gateway_logging={gatewayLogging}",
      partialFailedGatewayLoggingWaf:
        "부분 동기화 실패: Gateway_logging={gatewayLogging}, waf={waf}",
      success:
        "동기화된 {rules} 경로 경로, {hostRules} 호스트 경로, {streamRules} 프로토콜 매핑, 요청 로그 구성 및 현재 실행 모드에 대한 WAF 구성",
    },
    backup: {
      readFnosDirectoryFailed: "FNOS 백업 디렉터리를 읽지 못했습니다.",
      exportFnosSuccess: "FNOS 디렉터리로 백업을 내보냈습니다.",
      exportFnosFailed: "FNOS 디렉터리로 내보내지 못했습니다.",
      importSuccessWithWarnings:
        "백업을 가져왔지만 일부 런타임 동기화 단계가 실패했습니다.",
      importSuccess: "백업 가져오기 및 런타임 동기화 완료",
      importFailed: "백업을 가져오지 못했습니다.",
      importFnosSuccessWithWarnings:
        "FNOS 백업을 가져왔지만 일부 런타임 동기화 단계가 실패했습니다.",
      importFnosSuccess: "FNOS 백업을 가져왔고 런타임 동기화가 완료되었습니다.",
      importFnosFailed: "FNOS에서 백업을 가져오지 못했습니다.",
    },
    sessions: {
      notFound: "세션을 찾을 수 없습니다",
      listFailed: "세션 목록을 불러오지 못했습니다.",
      loadFailed: "세션을 불러오지 못했습니다.",
      updateFailed: "세션을 업데이트하지 못했습니다.",
      deleteFailed: "세션을 삭제하지 못했습니다.",
      mobilityLoadFailed: "세션 이동성 세부 정보를 불러오지 못했습니다.",
      deleted: "세션이 삭제되었습니다.",
    },
  },
  gatewayLogs: {
    configLoadFailed: "요청 로그 설정을 읽지 못했습니다.",
    configSaveFailed: "요청 로그 설정을 저장하지 못했습니다.",
    configSyncFailed:
      "요청 로그 설정이 저장되었지만 게이트웨이와 동기화하지 못했습니다.",
    readDirectoryFailed: "로그 디렉터리를 읽지 못했습니다.",
    readDatesFailed: "로그 날짜를 읽지 못했습니다.",
    readEntriesFailed: "요청 로그를 읽지 못했습니다.",
    deleteEntriesFailed: "요청 로그를 삭제하지 못했습니다.",
    invalidJsonObject: "요청 본문이 유효한 JSON 객체가 아닙니다.",
  },
  backoffRoutes: {
    ipRequired: "IP 매개변수가 누락되었습니다",
    listFailed: "로그인 backoff 목록을 불러오지 못했습니다.",
    statusFailed: "로그인 backoff 상태를 불러오지 못했습니다.",
    resetFailed: "로그인 backoff를 재설정하지 못했습니다.",
  },
  systemInfoRoutes: {
    loadAccessEntryFailed: "접속 진입 정보를 불러오지 못했습니다.",
  },
  securityOverviewRoutes: {
    loadFailed: "보안 개요를 불러오지 못했습니다.",
  },
  ipLocationRoutes: {
    batchLimit: "한 번에 최대 {max} IP를 쿼리합니다.",
    enqueueFailed: "IP 위치 조회 대기열 추가에 실패했습니다.",
  },
  gatewayPortal: {
    syncConfigFailed:
      "포털 디스플레이 구성을 게이트웨이에 동기화하지 못했습니다.",
    syncHostRulesFailed: "호스트 경로를 동기화하지 못했습니다.",
  },
  gatewayVisibility: {
    customCidrInvalid: "맞춤 CIDR 형식이 잘못되었습니다. {cidrs}",
    emptyEnabledConfig:
      "가시성을 활성화한 후 하나 이상의 지역 또는 하나의 사용자 정의 CIDR을 추가하세요.",
    syncFailed: "게이트웨이 공개 상태 구성을 동기화하지 못했습니다.",
  },
  gatewayCrawlerBlocker: {
    syncFailed: "크롤러 차단 구성을 동기화하지 못했습니다.",
  },
  scanner: {
    settingsLoadFailed: "스캐너 설정을 불러오지 못했습니다.",
    settingsUpdateFailed: "스캐너 설정을 업데이트하지 못했습니다.",
    invalidRequestBody: "요청 본문이 올바르지 않습니다.",
    atLeastOneIpRequired: "하나 이상의 IP를 제공하세요.",
    blacklistLoadFailed: "스캐너 차단 목록을 불러오지 못했습니다.",
    recordNotFound: "기록을 찾을 수 없습니다.",
    blacklistRecordLoadFailed: "스캐너 차단 목록 기록을 불러오지 못했습니다.",
    blacklistRecordDeleteFailed: "스캐너 차단 목록 기록을 삭제하지 못했습니다.",
    blacklistRecordsDeleteFailed:
      "스캐너 차단 목록 기록을 삭제하지 못했습니다.",
    cidrExemptionsInvalid: "CIDR 면제 형식이 잘못되었습니다. {cidrs}",
  },
  gatewayLogging: {
    syncConfigFailed: "게이트웨이 요청 로그 구성을 동기화하지 못했습니다.",
  },
  sslGateway: {
    clearFailed: "게이트웨이 인증서를 지우지 못했습니다.",
    syncFailed: "게이트웨이 인증서를 동기화하지 못했습니다.",
  },
  sslRoutes: {
    statusReadFailed: "SSL 상태를 불러오지 못했습니다.",
    gatewayStatusReadFailed: "게이트웨이 SSL 상태를 읽을 수 없습니다.",
    readSharedFileFailed: "공유 디렉터리 파일을 읽지 못했습니다.",
    emptyDomains:
      "도메인 목록이 비어 있습니다. 먼저 도메인이나 IP를 추가하세요.",
    certOrKeyInvalid: "인증서 또는 개인 키가 유효하지 않습니다.",
    hostRequired: "호스트가 필요합니다",
    localCaCertificateLabel: "로컬 CA 인증서",
    rootCaNotInitialized: "루트 CA가 초기화되지 않았습니다.",
    success: "성공함",
    certNotInstalled: "인증서가 설치되지 않았습니다.",
    certReadFailed: "SSL 인증서를 읽지 못했습니다.",
    certZipCreateFailed: "SSL 인증서 zip을 생성하지 못했습니다.",
    manualCertificateLabel: "수동으로 업로드된 인증서",
    certNotFound: "인증서를 찾을 수 없습니다.",
    caInitFailed: "로컬 CA 초기화에 실패했습니다.",
    caHostLoadFailed: "로컬 CA 호스트 목록을 불러오지 못했습니다.",
    caHostSaveFailed: "로컬 CA 호스트 목록을 저장하지 못했습니다.",
    certSaveFailed: "SSL 인증서 저장에 실패했습니다.",
    certActivateFailed: "SSL 인증서 활성화에 실패했습니다.",
    deploymentModeSaveFailed: "SSL 배포 모드 저장에 실패했습니다.",
    certDeleteFailed: "SSL 인증서 삭제에 실패했습니다.",
    certClearFailed: "SSL 인증서 구성 초기화에 실패했습니다.",
  },
  redis: {
    defaultCredential: "기본 자격 증명",
    certificateLabels: {
      acme: "ACME 인증서",
      ca: "자체 서명된 인증서",
      manual: "수동으로 업로드된 인증서",
      current: "현재 인증서",
    },
    ssl: {
      certFormatInvalid: "인증서 형식이 잘못되었습니다: {message}",
      keyFormatInvalid: "개인 키 형식이 잘못되었습니다: {message}",
      certKeyMismatch: "인증서와 개인 키가 일치하지 않습니다.",
      certKeyCheckFailed: "인증서 및 개인 키 확인 실패: {message}",
      certContentRequired: "인증서 내용이 필요합니다.",
      certNotFound: "인증서를 찾을 수 없습니다.",
      certOrKeyInvalid: "인증서 또는 개인 키가 유효하지 않습니다.",
    },
    acme: {
      domainRequired: "도메인이 필요합니다",
      domainsRequired: "도메인 목록은 필수 항목입니다.",
      dnsProviderRequired: "DNS 제공업체가 필요합니다.",
      primaryDomainDuplicated:
        "기본 도메인 {primaryDomain}이 이미 다른 요청 항목에 존재합니다.",
      applicationNotFound: "요청 항목을 찾을 수 없습니다",
      noMatchingIssuedCertificate:
        "이 요청 항목에는 도메인 구성과 일치하는 발급된 인증서가 없습니다.",
      jobDataInvalid: "ACME 작업 데이터가 잘못되었습니다.",
      multipleApplicationsUseNewApi:
        "여러 요청 항목이 이미 존재합니다. 새로운 API를 사용하여 ACME 요청 항목을 관리하세요.",
    },
  },
  acmeService: {
    waiting: "조치를 기다리는 중",
    sendSignalFailed: "{signal}을 {target}으로 보내지 못했습니다: {detail}",
    setDefaultCaFailed:
      "기본 인증 기관을 설정하지 못했습니다(종료 코드: {code}){brief}",
    registerAccountFailed: "ACME 계정 등록 실패(종료 코드: {code}){brief}",
    bundledZipMissing: "번들로 제공되는 acmesh.zip 리소스를 찾을 수 없습니다.",
    extractingBundled: "번들 acme.sh 리소스 추출 중...",
    unzipFailed: "추출 실패, 종료 코드: {code}",
    extractedAcmeMissing: "추출에 성공했지만 acme.sh를 찾을 수 없습니다.",
    writingDataDir: "데이터 디렉터리를 작성하는 중...",
    writtenAcmeMissing: "작성한 후 acme.sh를 찾을 수 없습니다.",
    checkInstallFailed: "설치 상태 확인 실패: {detail}",
    ready: "acme.sh가 준비되었습니다",
    notInstalled: "acme.sh가 설치되지 않았습니다",
    initializingBundled: "번들 acme.sh 초기화 중...",
    registeringAccount: "ACME 계정 등록 중...",
    savingDefaultCa: "기본 인증 기관 저장 중...",
    installSuccess: "설치 성공, 계정 이메일: {email}",
    installFailed: "설치 실패: {detail}",
    installFirst: "acme.sh를 먼저 설치하세요.",
    installingCannotDelete: "acme.sh가 설치 중이므로 삭제할 수 없습니다.",
    deleted: "acme.sh가 삭제되었습니다.",
    deleteFailed: "삭제 실패: {detail}",
    domainsRequired: "도메인 목록은 필수 항목입니다.",
    dnsTypeRequired: "DNS 확인 유형이 누락되었습니다.",
    issueFailed: "증명서 발급 실패(종료코드: {code}){brief}",
  },
  acmeJobRunner: {
    manualStop: "ACME 작업이 사용자에 의해 수동으로 중지되었습니다.",
    lockMessages: {
      manualRequest: "인증서 요청 중",
      autoRenew: "인증서 자동 갱신 중",
    },
    activeTaskRunning:
      "ACME 작업이 이미 실행 중입니다. 나중에 다시 시도하세요.",
    flowFailed: "인증서 요청 흐름 실패: {message}",
    stopSignalSent:
      "중지 신호가 전송되어 {count} acme.sh 프로세스가 종료되었습니다.",
    noRunningProcess: "실행 중인 acme.sh 프로세스를 찾을 수 없습니다.",
    stopProcessError: "프로세스 중지 중 예외 발생: {message}",
    processStillRunning: "acme.sh 프로세스가 아직 실행 중입니다: {pids}",
    lockLost:
      "ACME 런타임 잠금이 손실되었습니다. 작업이 중지되었습니다. 요청을 다시 시작하세요.",
    lockRefreshFailed: "ACME 런타임 잠금 새로 고침 실패: {message}",
    lockLeaseExpired:
      "{message}; 잠금 임대가 만료되었습니다. 작업이 중지되었습니다. 요청을 다시 시작하세요.",
    applicationChangedSkipped:
      "실행 중에 요청 항목 도메인이 변경되었습니다. 이전 인증서 작성을 건너뛰었습니다. 요청을 다시 시작하세요.",
    issuedButApplicationChanged:
      "인증서가 발급되었으나 요청항목 도메인이 변경되어 현재 요청항목에 기록되지 않습니다.",
    issuedButCertReadFailed:
      "인증서가 발급되었으나 인증서 파일 읽기에 실패했습니다. 나중에 다시 시도하거나 acme.sh 디렉터리를 확인하세요.",
    clearedDomainWorkingState:
      "acme.sh 도메인 작업 디렉터리를 지웠습니다. 이제 인증서 목록 및 갱신이 시스템 작업으로 관리됩니다.",
    clearDomainWorkingStateFailed:
      "인증서가 저장되었지만 acme.sh 도메인 상태 지우기에 실패함: {message}",
    linkedLibrarySyncedGateway:
      "연결된 인증서 라이브러리 항목을 동기화하고 게이트웨이 인증서 목록을 새로 고쳤습니다.",
    linkedLibraryUpdated: "연결된 인증서 라이브러리 항목을 업데이트했습니다.",
    addedToLibraryAndSyncedGateway:
      "인증서 발급 후 인증서 라이브러리에 자동으로 추가되었으며, 게이트웨이 인증서 목록이 새로 고쳐졌습니다.",
    addedToLibrary:
      "인증서는 발급 후 인증서 라이브러리에 자동으로 추가되었습니다.",
    addToLibraryFailed:
      "인증서가 발급되고 저장되었으나 인증서 라이브러리에 추가하지 못했습니다: {message}",
    stoppedIgnoredProcessError:
      "작업이 중지되었습니다. 프로세스 종료 오류가 무시되었습니다.",
  },
  acmeRoutes: {
    invalidRequestBody: "요청 본문이 올바르지 않습니다.",
    loadStatusFailed: "ACME 상태를 불러오지 못했습니다.",
    loadClientSettingsFailed: "ACME 클라이언트 설정을 불러오지 못했습니다.",
    saveClientSettingsFailed: "ACME 클라이언트 설정을 저장하지 못했습니다.",
    switchCertificateAuthorityFailed: "ACME 인증 기관을 전환하지 못했습니다.",
    loadOverviewFailed: "ACME 개요를 불러오지 못했습니다.",
    loadApplicationOverviewFailed: "ACME 요청 항목 개요를 불러오지 못했습니다.",
    loadConfigFailed: "ACME 설정을 불러오지 못했습니다.",
    loadSubdomainRecommendationFailed:
      "하위 도메인 인증서 추천을 불러오지 못했습니다.",
    loadApplicationsFailed: "ACME 요청 항목 목록을 불러오지 못했습니다.",
    loadApplicationFailed: "ACME 요청 항목을 불러오지 못했습니다.",
    updateApplicationFailed: "ACME 요청 항목을 업데이트하지 못했습니다.",
    deleteApplicationFailed: "ACME 요청 항목을 삭제하지 못했습니다.",
    syncLibraryFailed:
      "ACME 인증서를 인증서 라이브러리에 동기화하지 못했습니다.",
    deployCertificateFailed: "ACME 인증서를 배포하지 못했습니다.",
    loadJobFailed: "ACME 작업을 불러오지 못했습니다.",
    loadJobLogsFailed: "ACME 작업 로그를 불러오지 못했습니다.",
    loadJobPollFailed: "ACME 작업을 폴링하지 못했습니다.",
    stopJobFailed: "ACME 작업을 중지하지 못했습니다.",
    loadCertificateInfoFailed: "ACME 인증서 정보를 불러오지 못했습니다.",
    deleteCertificateFailed: "ACME 인증서를 삭제하지 못했습니다.",
    uninstallFailed: "ACME 클라이언트를 제거하지 못했습니다.",
    createCertificateZipFailed: "ACME 인증서 zip을 생성하지 못했습니다.",
    loadCertificateFailed: "ACME 인증서를 불러오지 못했습니다.",
    domainsInvalid: "도메인 목록이 비어 있거나 유효하지 않습니다.",
    dnsTypeRequired: "DNS 확인 유형이 누락되었습니다.",
    unsupportedDnsProvider: "지원되지 않는 DNS 공급자",
    missingDnsCredentials:
      "DNS API 자격 증명이 누락되었습니다. 다음 옵션 중 하나를 입력하세요: {requirements}",
    cloudflareInvalidKey:
      "Cloudflare API 키가 잘못되었습니다(잘못된 X-Auth-Key 형식).",
    cloudflareInvalidEmail:
      "Cloudflare 이메일이 잘못되었습니다(잘못된 X-Auth-Email 형식).",
    cloudflareInvalidHeaders:
      "Cloudflare API 요청 헤더가 유효하지 않습니다. 일반적으로 API 키 또는 이메일이 올바르지 않기 때문입니다.",
    acmeFrequencyLimited:
      "요청 빈도는 제한되어 있습니다(Retry-After={seconds} 초, 600초 이상 재시도 중지). 기다렸다가 다시 시도해 보세요.",
    dnsApiRateLimited:
      "DNS API 비율 제한이 트리거되었습니다(429/비율 제한). 나중에 다시 시도하세요.",
    logUnknownFailure:
      "로그에서 오류가 감지되었으나 자동으로 분류할 수 없습니다.",
    installingRetryLater:
      "acme.sh를 설치하는 중입니다. 나중에 다시 시도하세요.",
    installFirst: "acme.sh를 먼저 설치하세요.",
    multipleApplicationsUseNewApi:
      "여러 요청 항목이 이미 존재합니다. 새로운 API를 사용하여 ACME 요청 항목을 관리하세요.",
    applicationNotFound: "요청 항목을 찾을 수 없습니다",
    notFound: "찾을 수 없음",
    installingCannotDelete: "acme.sh가 설치 중이므로 삭제할 수 없습니다.",
    installingCannotSwitchCa:
      "acme.sh를 설치하는 중입니다. 아직 인증 기관을 전환할 수 없습니다.",
    noMatchingIssuedCertificate:
      "이 요청 항목에는 도메인 구성과 일치하는 발급된 인증서가 없습니다.",
    success: "성공함",
    dns01Only: "DNS-01 확인만 지원됩니다.",
    certNotFound: "인증서를 찾을 수 없습니다.",
    certOrKeyInvalid: "인증서 또는 개인 키가 유효하지 않습니다.",
  },
  acmeDnsProviders: {
    groups: {
      common: "공통",
      domestic: "중국",
      international: "국제",
      selfHostedAdvanced: "자체 호스팅/고급",
    },
    credentialSchemes: {
      default: "기본 자격 증명",
    },
    fields: {
      accountEmail: "계정 이메일",
      sshPrivateKeyPath: "SSH 개인 키 파일 경로",
    },
    labels: {
      aliyun: "Alibaba Cloud DNS",
      tencentCloudDnspod: "Tencent Cloud DNSPod(TencentCloud)",
      huaweiCloudDns: "화웨이 클라우드 DNS",
      jdCloudDns: "JD클라우드 DNS",
      westCn: "West.cn",
    },
    cloudflare: {
      globalKeyDescription:
        "Cloudflare의 레거시 글로벌 API 키 방법과 호환됩니다.",
      apiTokenDescription:
        "추천합니다. 토큰만 필요합니다. 영역 ID 또는 계정 ID를 알고 있는 경우 해당 ID도 입력하여 자동 감지를 줄입니다.",
    },
    gcloud: {
      description:
        "gcloud 명령어와 런타임 환경의 승인된 구성에 따라 다릅니다. 비워 두면 기본 gcloud 구성이 사용됩니다.",
    },
    azure: {
      managedIdentityDescription:
        "AZUREDNS_MANAGEDIDENTITY를 true로 설정합니다.",
    },
    descriptions: {
      boolean01: "0 또는 1을 입력합니다.",
      optionalBoolean01: "선택사항. 0 또는 1을 입력합니다.",
    },
    requirements: {
      optionalSuffix: "; 선택적 {keys}",
      orSeparator: "; 또는 ",
    },
  },
  acmePatches: {
    duckdns: {
      scriptMissing: "DuckDNS DNS API 스크립트를 찾을 수 없습니다: {path}",
      proxyApplied: "DuckDNS API를 {from}에서 {to}으로 전환했습니다.",
    },
  },
  reverseProxyTrustedIps: {
    syncFailed: "역방향 프록시 제한 면제 IP를 동기화하지 못했습니다.",
  },
  commonAuthLocations: {
    cidrLookupFailed: "CIDR 조회 실패",
    syncFailed: "공통 위치 면제 구성을 게이트웨이에 동기화하지 못했습니다.",
  },
  generalBlacklist: {
    invalidRequestBody: "요청 본문이 올바르지 않습니다.",
    invalidIp: "IP 주소가 올바르지 않습니다.",
    invalidIpWithValue: "IP 주소가 올바르지 않습니다: {ip}",
    atLeastOneValidIpRequired: "하나 이상의 유효한 IP를 입력하세요.",
    backendRequestFailed: "일반 블랙리스트 백엔드 요청에 실패했습니다.",
    backendResponseMissingData:
      "일반 블랙리스트 백엔드 응답에 데이터가 없습니다.",
  },
  fnosDataShare: {
    invalidPath: "잘못된 공유 파일 경로",
    shareMissing:
      "FNOS 공유 디렉터리를 찾을 수 없습니다. 앱 리소스가 올바르게 구성되었는지 확인하세요.",
    fileOnly: "공유 디렉터리의 파일만 읽을 수 있습니다.",
    fileTooLarge:
      "파일이 너무 큽니다. 여기에는 인증서 또는 개인 키 텍스트 파일만 배치하세요.",
  },
  autoHttps: {
    listenEacces:
      "포트 80을 수신할 수 있는 권한이 없습니다. 이 장치 또는 컨테이너가 프로세스가 낮은 포트를 바인딩하도록 허용하는지 확인하세요.",
    listenEaddrinuse:
      "포트 80은 이미 다른 프로그램에서 사용 중이므로 자동 HTTPS를 시작할 수 없습니다. FNOS 시스템 설정, 보안, 포트 설정, 편집을 시도하고 포트 80 및 443에 대한 리디렉션을 선택 취소하세요.",
    listenFailedWithMessage: "포트 80에서 수신 실패: {message}",
    listenFailed: "포트 80에서 수신하지 못했습니다.",
  },
  wafCollector: {
    drainFailed: "WAF 이벤트를 가져오지 못했습니다.",
  },
  hostMappingBookmarks: {
    defaultFolderTitle: "fn-knock 하위 도메인 매핑",
  },
  whitelist: {
    listFailed: "화이트리스트 레코드를 불러오지 못했습니다.",
    addFailed: "화이트리스트 레코드를 추가하지 못했습니다.",
    updateRecordsFailed: "화이트리스트 레코드를 업데이트하지 못했습니다.",
    deleteFailed: "화이트리스트 레코드를 삭제하지 못했습니다.",
    commentUpdateFailed: "화이트리스트 메모를 업데이트하지 못했습니다.",
    regionListFailed: "지역 화이트리스트를 불러오지 못했습니다.",
    regionAddFailed: "지역 화이트리스트를 추가하지 못했습니다.",
    regionDeleteFailed: "지역 화이트리스트를 삭제하지 못했습니다.",
    regionRequired: "지역을 하나 이상 선택하세요.",
    regionEmpty: "선택한 지역에서 사용할 수 있는 CIDR을 찾지 못했습니다.",
    regionNotFound: "지역 화이트리스트를 찾을 수 없습니다.",
    recordNotFound: "화이트리스트 기록을 찾을 수 없습니다",
    domainResolveFailed: "도메인 확인 실패",
    refreshFailed: "허용 목록 레코드를 새로 고치지 못했습니다.",
  },
  whitelistManager: {
    dnsRecordQueryFailedWithCode: "{label} 레코드 쿼리 실패({code}): {message}",
    dnsRecordQueryFailed: "{label} 레코드 쿼리 실패: {message}",
    targetFormatInvalid: "IP, CIDR 또는 도메인 형식이 잘못되었습니다.",
    autoGrantIpOnly: "자동 로그인 인증은 단일 IP만 지원합니다.",
    cidrInvalid: "CIDR 형식이 잘못되었습니다.",
    domainInvalid: "도메인 형식이 잘못되었습니다.",
    ipInvalid: "IP 형식이 잘못되었습니다.",
    autoOwnerMissing: "자동 허용 목록 소유자 식별자가 누락되었습니다.",
    domainResolveFailed: "도메인 확인 실패",
    resolvedIpCount: "해결된 {count} IP",
    noAaaaRecords: "A/AAAA 레코드가 해결되지 않았습니다.",
    syncAllowedStateFailed:
      "도메인 확인 결과가 업데이트되었지만 동기화 시스템 허용 상태가 실패했습니다.",
  },
  terminal: {
    defaultTitle: "웹 터미널",
    defaultSessionTitlePrefix: "세션-",
    tmuxNotDetectedInstallFirst:
      "tmux가 감지되지 않았습니다. 먼저 tmux 환경을 설치하세요.",
    tmuxReadyWithVersion: "tmux가 준비되었습니다: {version}",
    refreshingApt: "Debian 패키지 소스를 새로 고치는 중...",
    aptUpdateFailed: "apt-get 업데이트 실패",
    installingTmux: "tmux 설치 중...",
    aptInstallTmuxFailed: "apt-get 설치 tmux 실패",
    verifyingTmuxInstall: "tmux 설치 확인 중...",
    tmuxMissingAfterInstall:
      "설치가 완료된 후에도 tmux가 여전히 감지되지 않습니다.",
    tmuxInstallCompleteWithVersion: "tmux 설치 완료: {version}",
    tmuxInstallFailed: "tmux 설치 실패",
    operationFailed: "터미널 작업 실패",
    operationFailedWithMessage: "터미널 작업 실패: {message}",
    cwdUnavailable: "작업 디렉터리가 없거나 액세스할 수 없습니다: {path}",
    webTerminalDisabled: "웹 터미널이 활성화되지 않았습니다.",
    tmuxInstallingWait:
      "tmux를 설치하는 중입니다. 설치가 완료될 때까지 기다리세요.",
    tmuxStatusError: "tmux 상태 오류: {message}",
    tmuxMissingCannotCreate:
      "tmux가 감지되지 않아 재개 가능한 터미널 세션을 생성할 수 없습니다.",
    rootRunRequiresDangerToggle:
      "현재 프로세스는 루트로 실행 중입니다. 터미널을 생성하기 전에 설정에서 명시적인 고위험 실행 스위치를 활성화하세요.",
    requestedShellUnavailable: "요청한 셸을 사용할 수 없습니다: {shell}",
    noShellDetected:
      "사용 가능한 쉘이 감지되지 않았습니다. zsh, bash 또는 sh가 설치되어 있는지 확인합니다.",
    paneMetadataReadFailed: "터미널 창 메타데이터를 읽을 수 없습니다.",
    paneTtyParseFailed: "터미널 창 tty를 구문 분석할 수 없습니다.",
    inputPipeCreateFailed: "터미널 입력 파이프를 생성할 수 없습니다.",
    ioRelayCreateFailed: "터미널 IO 릴레이를 생성할 수 없습니다.",
    sessionLimitReached: "터미널 세션 제한에 도달했습니다({count})",
    tmuxSessionCreateFailed: "tmux 세션을 생성하지 못했습니다.",
    sessionTitleRequired: "세션 이름은 필수 항목입니다.",
    sessionMissingOrExpired: "터미널 세션이 존재하지 않거나 만료되었습니다.",
    tmuxMissingCannotAttach:
      "tmux가 감지되지 않아 터미널 세션을 연결할 수 없습니다.",
    inputPipeNotReady: "터미널 입력 파이프가 아직 준비되지 않았습니다.",
    inputWriteInterrupted: "터미널 입력 쓰기가 중단되었습니다.",
    attachmentExpired: "터미널 연결이 만료되었습니다",
    inputSendFailed: "터미널 입력을 보내지 못했습니다.",
    resizeFailed: "터미널 크기를 조정하지 못했습니다.",
    sessionNotFound: "터미널 세션을 찾을 수 없습니다",
  },
  waf: {
    manifestInvalid: "시스템 규칙 매니페스트 형식이 잘못되었습니다.",
    manifestMissingZipInfo:
      "시스템 규칙 매니페스트에 zip 파일 정보가 누락되었습니다.",
    manifestRequestFailed: "시스템 규칙 매니페스트 요청 실패: HTTP {status}",
    manifestRefreshFailed: "시스템 규칙 매니페스트를 새로 고치지 못했습니다.",
    confOnly: ".conf 규칙 파일만 지원됩니다.",
    ruleFilenameInvalid: "규칙 파일 이름이 잘못되었습니다.",
    fileTooLarge: "{filename}이 1MB를 초과합니다.",
    fileInvalidUtf8: "{filename}은 유효한 UTF-8 텍스트가 아닙니다.",
    filesystemDirectiveBlocked:
      "{filename}에 허용되지 않는 파일 시스템 지시문이 포함되어 있습니다.",
    systemRuleDescription: "시스템 보안 규칙",
    customRuleDescription: "사용자가 업로드한 규칙",
    enableNeedsRule:
      "WAF를 켜기 전에 하나 이상의 WAF 규칙 파일을 활성화하십시오.",
    rulesLoadFailed: "WAF 규칙을 로드하지 못했습니다.",
    configSyncFailed: "WAF 구성을 게이트웨이에 동기화하지 못했습니다.",
    sourceInvalid: "규칙 소스가 잘못되었습니다.",
    ruleFileNotFound: "규칙 파일을 찾을 수 없습니다.",
    zipInvalid: "시스템 규칙 zip 형식이 잘못되었습니다.",
    zipDirectoryInvalid: "시스템 규칙 zip 디렉터리가 잘못되었습니다.",
    zipUnpackedTooLarge: "추출 후 시스템 규칙 패키지가 너무 큽니다.",
    zipHeaderInvalid: "시스템 규칙 zip 파일 헤더가 잘못되었습니다.",
    zipMethodUnsupported: "지원되지 않는 zip 압축 방법 {method}",
    zipSizeInvalid: "시스템 규칙 zip 파일 크기가 잘못되었습니다.",
    zipPathInvalid: "시스템 규칙 zip 파일 경로가 잘못되었습니다: {path}",
    downloadFailed: "시스템 규칙 다운로드 실패: HTTP {status}",
    zipTooLarge: "시스템 규칙 패키지가 너무 큽니다.",
    zipHashMismatch: "시스템 규칙 패키지 해시 확인에 실패했습니다.",
    zipEmpty: "시스템 규칙 패키지가 비어 있습니다.",
    zipDuplicateFile: "시스템 규칙 패키지에 중복된 파일이 있습니다: {path}",
    zipConfRootOnly:
      "시스템 규칙 패키지의 .conf 파일은 루트 디렉터리에 있어야 합니다.",
    zipNoConf: "시스템 규칙 패키지에 .conf 파일이 포함되어 있지 않습니다.",
    systemRulePathInvalid: "시스템 규칙 파일 경로가 잘못되었습니다.",
    manifestEmpty: "시스템 규칙 매니페스트가 비어 있습니다.",
    keepOneEnabledRule:
      "WAF가 켜져 있는 동안 하나 이상의 규칙 파일을 활성화된 상태로 유지하세요.",
    uploadSelectConf: "업로드할 .conf 파일 선택",
    base64Invalid: "규칙 파일 내용이 유효한 Base64가 아닙니다.",
    reloadRulesFailed: "WAF 규칙을 다시 로드하지 못했습니다.",
    detailsLoadFailed: "WAF 세부 정보를 불러오지 못했습니다.",
    statusReadFailed: "WAF 상태를 읽지 못했습니다.",
    invalidRequestBody: "요청 본문이 올바르지 않습니다.",
    dateInvalid: "날짜 형식이 잘못되었습니다. YYYY-MM-DD 형식이어야 합니다.",
    configSaveOrLoadFailed: "WAF 설정을 저장하거나 로드하지 못했습니다.",
    systemRulesSyncFailed: "시스템 규칙을 동기화하지 못했습니다.",
    ruleToggleFailed: "WAF 규칙을 활성화 또는 비활성화하지 못했습니다.",
    ruleReadFailed: "WAF 규칙을 읽지 못했습니다.",
    customRuleUploadFailed: "맞춤 규칙을 업로드하지 못했습니다.",
    customRuleDeleteFailed: "맞춤 규칙을 삭제하지 못했습니다.",
    eventsDrainFailed: "WAF 이벤트를 가져오지 못했습니다.",
    logsQueryFailed: "WAF 로그를 쿼리하지 못했습니다.",
    logNotFound: "WAF 로그를 찾을 수 없습니다",
    logLoadFailed: "WAF 로그를 불러오지 못했습니다.",
    logsDeleteFailed: "WAF 로그를 삭제하지 못했습니다.",
  },
  oidc: {
    callbackStateExpired:
      "로그인 상태가 만료되었습니다. 다시 로그인을 시작하세요.",
    loginFailedRetry: "외부 로그인에 실패했습니다. 다시 로그인을 시작하세요.",
    loginMethodUnavailable:
      "현재 로그인 모드에서는 외부 로그인을 사용할 수 없습니다.",
    reservedExtraAuthParam:
      "extra_auth_params에는 예약된 OIDC 매개변수인 {key}이 포함되어 있습니다.",
    urlInvalid: "{label}은 유효한 URL이어야 합니다.",
    urlMustUseHttps: "{label}은 HTTPS를 사용해야 합니다.",
    providerUnsupported: "지원되지 않는 외부 로그인 공급자",
    providerMissingRequiredConfig:
      "{provider}에 필수 구성이 누락되었습니다: {fields}",
    providerMissingRequiredFields:
      "외부 로그인 제공업체에 필수 구성이 누락되었습니다: {fields}",
    accessTokenMissing: "access_token이 반환되지 않았습니다.",
    idTokenMissing: "id_token이 반환되지 않았습니다.",
    callbackUrlBuildFailed:
      "외부 로그인 콜백 URL을 구축할 수 없습니다. public_auth_base_url을 구성합니다.",
    issuerMissing: "OIDC 발급자가 구성되지 않았습니다.",
    discoveryMissingFields: "OIDC 검색 문서에 필수 입력란이 누락되었습니다.",
    nonceCheckFailed: "OIDC nonce 확인 실패",
    issuerCheckFailed: "OIDC 발급자 확인 실패",
    subjectEmpty: "OIDC 제목이 비어 있습니다.",
    githubUserIdEmpty: "GitHub 사용자 ID가 비어 있습니다.",
    providerNotFound: "외부 로그인 공급자를 찾을 수 없습니다.",
    connectionTestSuccess: "연결 테스트 성공",
    oauthEndpointIncomplete: "OAuth2 엔드포인트 구성이 불완전합니다.",
    connectionTestFailed: "연결 테스트 실패",
    totpMissing: "TOTP 자격 증명을 찾을 수 없습니다.",
    selectProvider: "외부 로그인 공급자 선택",
    providerUnavailable: "외부 로그인 공급자를 사용할 수 없습니다.",
    bindingNotFound: "외부 계정 바인딩을 찾을 수 없습니다.",
    inviteInvalid: "바인딩 초대 링크가 잘못되었습니다.",
    inviteExpired: "바인딩 초대 링크가 만료되었습니다",
    inviteProviderNotAllowed: "이 초대 링크는 이 공급자를 허용하지 않습니다.",
    authorizationEndpointMissing: "인증 끝점이 구성되지 않았습니다.",
    authorizationEndpointInvalid: "인증 엔드포인트 형식이 올바르지 않습니다.",
    bindStateInvalid: "바인딩 초대 상태가 잘못되었습니다.",
    accountNotBoundCannotLogin:
      "이 외부 계정은 연결되어 있지 않아 로그인할 수 없습니다.",
    tokenEndpointMissing: "토큰 엔드포인트가 구성되지 않았습니다.",
    clientIdMissing: "client_id가 구성되지 않았습니다.",
    bindProviderMismatch: "바인딩 초대가 로그인 제공업체와 일치하지 않습니다.",
    inviteTotpMissing:
      "이 바인딩 초대에 연결된 TOTP가 더 이상 존재하지 않습니다.",
    accountAlreadyBoundOtherTotp:
      "이 외부 계정은 이미 다른 TOTP에 연결되어 있습니다.",
    inviteUsed: "바인딩 초대 링크가 이미 사용되었습니다.",
    externalAccountFallback: "외부 계정",
    loginFailedWithDetail: "외부 로그인 실패: {detail}",
    tokenRequestFailed: "외부 로그인 토큰을 가져오지 못했습니다: {detail}",
    readResponseFailed: "외부 로그인 응답을 읽지 못했습니다: {detail}",
    httpResponseFailed: "외부 로그인 요청 실패: HTTP {status}: {detail}",
    jsonResponseInvalid: "외부 로그인 응답이 유효한 JSON이 아닙니다: {detail}",
    jwksUriMissing: "OIDC JWKS URI가 구성되지 않았습니다.",
    jwksFetchFailed: "OIDC JWKS를 가져오지 못했습니다: {detail}",
    jwksInvalid: "OIDC JWKS 응답이 올바르지 않습니다: {detail}",
    tokenHeaderInvalid: "OIDC token header가 올바르지 않습니다: {detail}",
    signingKeyUnavailable: "OIDC 서명 키를 사용할 수 없습니다.",
    signingKeyInvalid: "OIDC 서명 키가 올바르지 않습니다: {detail}",
    idTokenVerificationFailed: "OIDC id_token 검증 실패: {detail}",
    githubProfileRequestFailed: "GitHub 프로필 요청 실패: {detail}",
    providerErrors: {
      accessDenied:
        "외부 로그인 승인을 취소했거나 제공업체가 요청을 거부했습니다.",
      temporarilyUnavailable:
        "외부 로그인 서비스를 일시적으로 사용할 수 없습니다. 나중에 다시 시도하세요.",
      serverError:
        "외부 로그인 공급자가 서비스 오류를 반환했습니다. 나중에 다시 시도하세요.",
      invalidScope:
        "외부 로그인 범위가 잘못 구성되었습니다. 관리자에게 공급자 설정을 확인하도록 요청하세요.",
      rejected:
        "공급자가 외부 로그인 요청을 거부했습니다. 외부 로그인 구성을 확인하고 다시 시도하세요.",
      incomplete:
        "외부 로그인이 완료되지 않았습니다. 다시 로그인을 시작하세요.",
    },
    bindWithProvider: "{provider}으로 바인딩",
    selectProviderTitle: "외부 계정 제공업체 선택",
    bindToTotp: "외부 계정을 {totp}에 바인딩합니다.",
    linkMissingToken: "링크에 토큰이 없습니다.",
    inviteMissingExpiredUsed:
      "이 초대는 존재하지 않거나, 만료되었거나, 이미 사용되었습니다.",
    noProvidersTitle: "사용 가능한 외부 로그인 제공업체가 없습니다.",
    noProvidersBody:
      "이 초대에는 현재 바인딩할 수 있는 외부 계정 제공업체가 없습니다.",
    bindFailedTitle: "외부 계정 바인딩 실패",
    bindStartFailed: "외부 계정 바인딩을 시작할 수 없습니다.",
    startFailed: "외부 로그인을 시작하지 못했습니다.",
    callbackMissingParams:
      "외부 로그인 콜백에 필수 매개변수가 누락되었습니다. 다시 로그인을 시작하세요.",
    loginFailed: "외부 로그인 실패",
    operationAborted:
      "외부 로그인 요청이 중단되었습니다. 다시 로그인을 시작하세요.",
    loginFailedRetryAfter: "{message}. {seconds} 초 후에 다시 시도하세요.",
    createProviderFailed: "외부 로그인 공급자를 생성하지 못했습니다.",
    updateProviderFailed: "외부 로그인 공급자를 업데이트하지 못했습니다.",
    deleteProviderFailed: "외부 로그인 공급자를 삭제하지 못했습니다.",
    testProviderFailed: "외부 로그인 공급자를 테스트하지 못했습니다.",
    deleteBindingFailed: "외부 계정 바인딩을 삭제하지 못했습니다.",
    createInviteFailed: "바인딩 초대를 생성하지 못했습니다.",
    listProvidersFailed: "외부 로그인 제공자 목록을 불러오지 못했습니다.",
    providerPayloadObject: "제공자 페이로드는 객체여야 합니다.",
    loadProviderFailed: "외부 로그인 제공자를 불러오지 못했습니다.",
    listBindingsFailed: "외부 계정 바인딩 목록을 불러오지 못했습니다.",
    invitationPayloadObject: "초대 페이로드는 객체여야 합니다.",
    totpRequired: "TOTP 자격 증명이 필요합니다.",
    loadTotpFailed: "TOTP 자격 증명을 불러오지 못했습니다.",
    loadConfigFailed: "구성을 불러오지 못했습니다.",
    inviteUrlBuildFailed: "외부 계정 초대 URL을 만들지 못했습니다.",
    connectionConfigInvalid:
      "외부 로그인 제공자 연결 구성이 올바르지 않습니다.",
    oauthEndpointIncompleteWithField:
      "OAuth2 엔드포인트 구성이 완전하지 않습니다: {field}",
    discoveryHttpFailed: "OIDC discovery 요청 실패: HTTP {status}: {detail}",
    discoveryInvalid: "OIDC discovery 문서가 올바르지 않습니다.",
    discoveryMissingFieldsWithList:
      "OIDC discovery 문서에 필수 필드가 없습니다: {fields}",
    providerTypeRequired: "외부 로그인 제공자 유형이 필요합니다.",
    storedProviderInvalid: "저장된 외부 로그인 제공자가 올바르지 않습니다.",
    storedProviderTypeInvalid:
      "저장된 외부 로그인 제공자 유형이 올바르지 않습니다.",
    catalog: {
      googleDescription: "Google 계정으로 로그인하세요.",
      microsoftDescription: "Microsoft/Azure AD 계정으로 로그인하세요.",
      githubDescription: "GitHub OAuth로 로그인하세요.",
      customLabel: "맞춤형 OIDC",
      customDescription:
        "표준 OpenID Connect Discovery와 함께 사용자 정의 공급자를 사용합니다.",
    },
  },
  subdomainMode: {
    recommendationMissingBase:
      "루트 도메인이나 인증 서비스가 구성되지 않아 권장 인증서 도메인을 아직 생성할 수 없습니다.",
    recommendationWildcardSummary:
      "권장 도메인: {rootDomain} 및 *.{rootDomain}은 동일한 상위 도메인 아래 루트 도메인, 인증 서비스 및 비즈니스 하위 도메인을 포함합니다.",
    authOutOfRootWarning:
      "현재 인증 서비스 {authHost}은 루트 도메인 {rootDomain}에 속하지 않습니다. 정확한 도메인은 별도로 추가되었습니다. 선택한 DNS 공급자가 이러한 도메인을 관리할 수 있는지 확인하세요.",
    recommendationSingleHostSummary:
      "루트 도메인이 구성되지 않았으므로 인증 서비스 {authHost}에 대한 단일 도메인 인증서만 권장될 수 있습니다.",
    wildcardSuggestion:
      "나중에 여러 비즈니스 하위 도메인을 처리하려면 와일드카드 인증서를 요청하기 전에 루트 도메인을 추가하세요.",
    configureRootOrAuth:
      "하위 도메인 모드에서 루트 도메인을 구성하거나 먼저 호스트 매핑에서 인증 서비스를 지정하세요.",
    authMissingWarning:
      "인증 서비스가 지정되지 않았으므로 권장 사항은 루트 도메인에서만 파생됩니다.",
    uncoveredHostMappingsWarning:
      "{count} 호스트 매핑이 권장 인증서 적용 범위를 벗어났습니다. 공개 노출이 필요한 경우 인증서를 추가하거나 도메인 계획을 조정하세요.",
    coverageNoSsl:
      "SSL 인증서가 활성화되지 않았으므로 인증 서비스 및 비즈니스 하위 도메인은 아직 HTTPS로 보호되지 않습니다.",
    coverageReadyConcrete:
      "배포된 인증서에는 인증 서비스와 구성된 모든 호스트 매핑이 포함됩니다.",
    coverageReadyRecommended:
      "배포된 인증서는 하위 도메인 모드에 대한 현재 권장 범위를 충족합니다.",
    coveragePartialConcrete:
      "현재 인증서는 하위 도메인 모드에 필요한 일부 도메인에만 적용됩니다. 인증 서비스 또는 일부 비즈니스 호스트에는 여전히 인증서 불일치가 있을 수 있습니다.",
    coveragePartialRecommended:
      "현재 인증서는 일부 권장 도메인에만 적용됩니다. 나중에 하위 도메인 모드를 활성화하면 인증서 불일치가 계속 발생할 수 있습니다.",
    coverageMismatchConcrete:
      "배포된 인증서가 하위 도메인 모드와 일치하지 않습니다. 인증 서비스 및 비즈니스 호스트는 아직 올바르게 다루어지지 않습니다.",
    coverageMismatchRecommended:
      "배포된 인증서가 아직 하위 도메인 모드에 권장되는 도메인 범위를 포함하지 않습니다.",
    coverageMissingRequiredWarning:
      "현재 인증서에 {count} 필수 적용 범위 항목이 누락되었습니다. 인증서를 재발급하거나 교체하십시오.",
    coverageMissingRecommendedWarning:
      "현재 인증서에는 {count} 권장 도메인 적용 범위 항목이 없습니다. 나중에 해당 도메인을 사용할 경우 재발급하거나 교체하세요.",
    coverageAuthHostMissingWarning:
      "현재 인증서에는 인증 서비스 {authHost}이 포함되지 않습니다.",
    inventoryEmpty:
      "인증서 인벤토리에는 하위 도메인 모드에 사용할 수 있는 인증서가 아직 포함되어 있지 않습니다.",
    inventoryActiveReady:
      "활성 인증서는 하위 도메인 모드에 필요한 도메인을 완전히 포함합니다.",
    inventoryOneReady:
      "인벤토리의 인증서 하나는 하위 도메인 모드를 완전히 포함하며 직접 활성 모드로 전환할 수 있습니다.",
    inventoryMultipleReady:
      "인벤토리의 {count} 인증서는 각각 현재 하위 도메인 모드를 완전히 포함합니다.",
    inventoryCombinedReady:
      "인증서 인벤토리를 결합하면 전체 적용 범위를 제공할 수 있습니다.",
    inventoryCandidateReady:
      "인증서 인벤토리에는 현재 하위 도메인 모드를 다루는 후보가 이미 있습니다.",
    inventoryCombinedNeedsMultiSni:
      "인증서 인벤토리는 현재 하위 도메인 모드를 조합하여 다룰 수 있지만 게이트웨이는 여전히 단일 활성 인증서 모드이므로 아직 모두 적용될 수는 없습니다.",
    inventoryPartialCandidates:
      "인증서 인벤토리에는 부분적인 후보가 있지만 여전히 인증 서비스와 모든 호스트 매핑을 완전히 포함하지는 않습니다.",
    inventoryNoCertificateCoversRecommendation:
      "현재 하위 도메인 모드에 권장되는 도메인을 다루는 인증서는 없습니다.",
    inventoryMultiCertRequiresSniWarning:
      "인증서 인벤토리에는 결합된 적용 범위를 위해 여러 인증서가 필요하지만 게이트웨이는 여전히 단일 활성 인증서 모드에 있으므로 모든 인증서가 동시에 적용될 수는 없습니다.",
    inventorySwitchRecommendedWarning:
      "활성 인증서가 하위 도메인 모드와 완전히 일치하지 않습니다. 권장 인증서로 전환하세요.",
    inventoryBetterForSniWarning:
      "기존 인증서 인벤토리는 향후 다중 인증서/SNI 배포에 더 적합합니다.",
  },
  cloudflared: {
    configReadFailed: "Cloudflared 설정을 읽지 못했습니다.",
    configWriteFailed: "Cloudflared 설정을 저장하지 못했습니다.",
    missingToken: "Cloudflare 토큰을 먼저 구성하세요",
    startFailedWithDetail: "Cloudflared를 시작하지 못했습니다: {detail}",
    processExited: "cloudflared 프로세스가 종료되었습니다.",
    processExitedWithCode:
      "cloudflared 프로세스가 {code} 코드로 종료되었습니다.",
    processCrashed: "cloudflared 프로세스가 중단되었습니다: {message}",
    resumeOnBoot:
      "이력서: Cloudflared가 지난번에 실행 중이었고 자동으로 복원 중입니다...",
    unknownError: "알 수 없는 오류",
    notInitialized: "Cloudflared가 초기화되지 않았습니다",
    startFailed: "시작하지 못했습니다.",
    stopFailed: "Cloudflared를 중지하지 못했습니다.",
    logsListFailed: "Cloudflared 로그를 불러오지 못했습니다.",
    logsClearFailed: "Cloudflared 로그를 지우지 못했습니다.",
    logsPollFailed: "Cloudflared 로그를 폴링하지 못했습니다.",
  },
  dnsmasq: {
    notDetectedInstallFirst: "dnsmasq가 감지되지 않았습니다. 먼저 설치하세요.",
    dnsPortUnavailable:
      "DNS 포트 53을 사용할 수 없습니다. 포트를 해제하고 다시 시도하세요.",
    dnsPortUnavailableWithDetail:
      "DNS 포트 53을 사용할 수 없습니다. 포트를 해제하고 다시 시도하세요: {detail}",
    detectedWithVersion:
      "dnsmasq가 감지되었습니다: {version}. 초기화 또는 서비스 시작을 기다리는 중입니다.",
    detected:
      "dnsmasq가 감지되었습니다. 초기화 또는 서비스 시작을 기다리는 중입니다.",
    missingServiceAutoComplete:
      "시스템 서비스가 누락되어 초기화 중에 완료됩니다.",
    servicePackageMissing:
      "dnsmasq 실행 파일이 감지되었지만 시스템 서비스가 설치되지 않았습니다. 먼저 dnsmasq 패키지를 설치하세요.",
    completingService: "dnsmasq 시스템 서비스를 완료하는 중...",
    completeServiceFailed: "dnsmasq 시스템 서비스를 완료하지 못했습니다.",
    serviceDefinitionMissingAfterInstall:
      "dnsmasq 서비스 설치가 완료되었지만 사용 가능한 시스템 서비스 정의가 감지되지 않았습니다.",
    executableMissing: "dnsmasq 실행 파일이 감지되지 않았습니다.",
    configTestFailed: "dnsmasq 구성 검증에 실패했습니다.",
    restartFailed: "dnsmasq를 다시 시작하지 못했습니다.",
    serviceDefinitionMissing:
      "dnsmasq 시스템 서비스 정의가 감지되지 않았습니다. 초기화를 완료하여 서비스 환경을 완성합니다.",
    readyWithVersion: "dnsmasq가 준비되었습니다: {version}",
    ready: "dnsmasq가 준비되었습니다",
    refreshingApt: "Debian 패키지 소스를 새로 고치는 중...",
    aptUpdateFailed: "apt-get 업데이트 실패",
    installing: "dnsmasq 설치 중...",
    aptInstallFailed: "apt-get 설치 dnsmasq 실패",
    enablingService: "DNSmasq 서비스를 활성화하는 중...",
    verifyingService: "dnsmasq 서비스 확인 중...",
    installMissingAfterComplete:
      "설치가 완료된 후에도 dnsmasq가 여전히 감지되지 않습니다.",
    installFailed: "dnsmasq 설치 실패",
    checkingEnvironment: "dnsmasq 환경을 확인하는 중...",
    validatingConfig: "dnsmasq 구성 검증 중...",
    startingService: "DNSmasq 서비스 시작 중...",
    initializeFailed: "dnsmasq 초기화 실패",
  },
  firewall: {
    goBackendCallFailed: "Go 백엔드 API 호출 실패: {message}",
    clearLegacyTcpRedirectFailed:
      "레거시 TCP 리디렉션 {listenPort} -> {targetPort}을 지우지 못했습니다.",
    initDefaultRulesFailed: "기본 방화벽 규칙을 초기화하지 못했습니다.",
    syncWhitelistTargetFailed:
      "화이트리스트 대상 {target}을 동기화하지 못했습니다.",
    cleanRulesFailed: "방화벽 규칙을 지우지 못했습니다.",
    syncAuthGatewayConfigFailed:
      "인증 게이트웨이 구성을 동기화하지 못했습니다.",
    syncReverseProxyThrottleFailed:
      "역방향 프록시 제한 구성을 동기화하지 못했습니다.",
    syncGatewayVisibilityConfigFailed:
      "게이트웨이 공개 상태 구성을 동기화하지 못했습니다.",
    syncGatewayProxyHeadersConfigFailed:
      "게이트웨이 프록시 헤더 구성을 동기화하지 못했습니다.",
    syncGatewayHostResponseConfigFailed:
      "게이트웨이 호스트 응답 구성을 동기화하지 못했습니다.",
    syncGatewayCrawlerBlockerConfigFailed:
      "크롤러 차단 구성을 동기화하지 못했습니다.",
    enableProxyProtocolForceFailed:
      "강제 프록시 프로토콜 모드를 활성화하지 못했습니다.",
    disableProxyProtocolForceFailed:
      "강제 프록시 프로토콜 모드를 비활성화하지 못했습니다.",
    disableStreamRulesFailed: "프로토콜 매핑 리스너를 비활성화하지 못했습니다.",
    flushPathRoutesFailed: "경로 경로를 지우지 못했습니다.",
    syncHostRoutesFailed: "호스트 경로를 동기화하지 못했습니다.",
    syncDefaultRouteFailed: "기본 경로를 동기화하지 못했습니다.",
    flushHostRoutesFailed: "호스트 경로를 지우지 못했습니다.",
    syncPathRoutesFailed: "경로 경로를 동기화하지 못했습니다.",
    syncStreamRulesFailed: "프로토콜 매핑을 동기화하지 못했습니다.",
    syncAuthEntryRouteFailed: "인증 항목 경로를 동기화하지 못했습니다.",
    syncAuthDefaultRouteFailed: "인증 기본 경로를 동기화하지 못했습니다.",
  },
  updateManager: {
    manifestFieldInvalid: "업데이트 매니페스트 필드 {field}이 잘못되었습니다.",
    manifestFormatInvalid: "업데이트 매니페스트 형식이 잘못되었습니다.",
    manifestMissingVersion: "업데이트 매니페스트에 버전이 없습니다.",
    manifestMissingUpdateAvailable:
      "업데이트 매니페스트에 update_available이 없습니다.",
    manifestMissingForceUpdate:
      "업데이트 매니페스트에 force_update가 없습니다.",
    manifestMissingDownloadUrl:
      "업데이트 매니페스트에 download_url이 없습니다.",
    manifestArm64FieldsIncomplete:
      "업데이트 매니페스트 ARM64 다운로드 필드가 불완전합니다.",
    architectureUnsupported:
      "이 아키텍처에서는 자동 업데이트가 지원되지 않습니다: {arch}",
    manifestMissingArm64DownloadUrl:
      "업데이트 매니페스트에 ARM64 다운로드 URL이 없습니다.",
    manifestMissingArm64Checksum:
      "업데이트 매니페스트에 ARM64 체크섬이 없습니다.",
    checkHttpFailed: "업데이트 확인 실패: HTTP {status}",
    checkFailed: "업데이트 확인 실패",
    noUpdateInfo: "아직 업데이트 정보를 가져오지 않았습니다.",
    featureDisabled: "현재 업데이트가 비활성화되어 있습니다.",
    alreadyLatest: "이미 최신 버전을 사용 중입니다.",
    downloadHttpFailed: "다운로드 실패: HTTP {status}",
    responseBodyUnreadable: "다운로드 실패: 응답 본문을 읽을 수 없습니다.",
    checksumFailed:
      "체크섬 실패: {expected}이 필요했지만 {actual}을 받았습니다.",
    downloadFailed: "다운로드 실패",
    noInstallableUpdate: "설치 가능한 업데이트가 없습니다.",
    downloadPackageFirst:
      "설치하기 전에 업데이트 패키지를 다운로드하고 확인하세요.",
    packageMissing: "업데이트 패키지가 없습니다. 다시 다운로드하세요.",
    packageChecksumFailed:
      "업데이트 패키지 체크섬에 실패했습니다. 다시 다운로드하세요.",
    installStartFailed: "업데이트 설치를 시작하지 못했습니다.",
  },
  tunnelManagers: {
    cloudflared: {
      macAutoDownloadUnsupported:
        "macOS에서는 자동 앱 다운로드가 지원되지 않습니다. Brew install cloudflared를 사용하여 수동으로 설치하세요.",
      platformUnsupported: "이 플랫폼은 지원되지 않습니다",
      downloadStarted: "Cloudflared 다운로드를 시작했습니다.",
      responseBodyUnreadable: "다운로드 응답 본문을 읽을 수 없습니다.",
      downloadCancelled: "다운로드가 취소되었습니다.",
      unknownError: "알 수 없는 오류",
      deleteSuccess: "Cloudflared가 삭제되었습니다.",
      deleteFailed: "Cloudflared 삭제 실패: {detail}",
      macManualRemove: "macOS에서 수동으로 cloudflared 제거",
      notInstalledBrew:
        "Cloudflared가 설치되어 있지 않습니다. 먼저 brew install cloudflared를 사용하여 설치하세요.",
      notInitialized:
        "Cloudflared가 초기화되지 않았습니다. 먼저 다운로드하세요.",
    },
    frp: {
      platformUnsupported: "이 플랫폼은 지원되지 않습니다",
      packageMissing: "FRP 패키지가 누락되었습니다.",
      extractFailed: "종료 코드 {code}으로 인해 추출에 실패했습니다.",
      downloadStarted: "FRP 다운로드를 시작했습니다.",
      responseBodyUnreadable: "다운로드 응답 본문을 읽을 수 없습니다.",
      connectionFailed: "연결 실패",
      downloadFailed: "다운로드 실패: {detail}",
      unknownError: "알 수 없는 오류",
      downloadCancelled: "다운로드가 취소되었습니다.",
      deleteSuccess: "FRP가 삭제되었습니다.",
      deleteFailed: "FRP 삭제 실패: {detail}",
      notInitialized: "FRP가 초기화되지 않았습니다. 먼저 다운로드하세요.",
    },
  },
  frpc: {
    instanceNotFound: "FRP 인스턴스가 존재하지 않습니다: {id}",
    instanceLimitExceeded: "최대 {limit} 추가 FRP 인스턴스가 지원됩니다.",
    primaryName: "1차 FRP",
    instanceName: "FRP 인스턴스",
    verifyFailedWithDetail: "frpc 확인 실패: {detail}",
    verifyFailedWithCode: "종료 코드 {code}으로 인해 frpc 확인에 실패했습니다.",
    verifyFrpNotInitialized:
      "FRP가 초기화되지 않아 frpc.toml을 확인할 수 없습니다. 먼저 시스템 설정에서 FRP 리소스를 다운로드하세요.",
    pidInvalidForInstance:
      "PID가 더 이상 유효하지 않거나 이 인스턴스에 속하지 않습니다.",
    processExited: "frpc 프로세스가 종료되었습니다.",
    processExitedWithCode: "frpc 프로세스가 {code} 코드로 종료되었습니다.",
    processCrashed: "frpc 프로세스가 중단되었습니다: {message}",
    processStillRunning: "FRP 프로세스가 여전히 실행 중입니다. pid={pid}",
    primaryDeleteDenied: "기본 FRP 인스턴스는 삭제할 수 없습니다.",
    notInitialized: "FRP가 초기화되지 않았습니다.",
    startFailedWithDetail: "frpc를 시작하지 못했습니다: {detail}",
    pidReadFailed: "frpc PID를 읽지 못했습니다.",
    startedWithPid: "frpc가 시작되었습니다. pid={pid}",
    stoppedWithPid: "frpc가 중지되었습니다. pid={pid}",
    alreadyStopped: "frpc가 이미 중지되었습니다.",
    pidCleanedForInstance:
      "PID는 이 인스턴스에 속하지 않습니다. 이 인스턴스 런타임 기록이 삭제되었습니다",
    resumeOnBoot:
      "재개: 이 FRP 인스턴스는 지난번에 실행 중이었으며 자동으로 복원 중입니다...",
    routes: {
      saveConfigFailed: "구성을 저장하지 못했습니다.",
      startFailed: "시작하지 못했습니다.",
      stopFailed: "중지하지 못했습니다.",
      createInstanceFailed: "인스턴스를 생성하지 못했습니다.",
      startInstanceFailed: "인스턴스를 시작하지 못했습니다.",
      stopInstanceFailed: "인스턴스를 중지하지 못했습니다.",
      restartInstanceFailed: "인스턴스를 다시 시작하지 못했습니다.",
      getInstanceLogsFailed: "인스턴스 로그를 가져오지 못했습니다.",
      clearInstanceLogsFailed: "인스턴스 로그를 지우지 못했습니다.",
      pollInstanceFailed: "인스턴스를 폴링하지 못했습니다.",
      getInstanceDetailFailed: "인스턴스 세부정보를 가져오지 못했습니다.",
      updateInstanceFailed: "인스턴스를 업데이트하지 못했습니다.",
      deleteInstanceFailed: "인스턴스를 삭제하지 못했습니다.",
    },
  },
  dockerAdminPanel: {
    passwordTooShort: "관리자 패널 비밀번호는 6자 이상이어야 합니다.",
    passwordTooLong: "관리자 패널 비밀번호는 128자를 초과할 수 없습니다.",
    passwordWhitespace: "관리자 패널 비밀번호에는 공백을 포함할 수 없습니다.",
    passwordNeedsLettersAndNumbers:
      "관리자 패널 비밀번호에는 문자와 숫자가 모두 포함되어야 합니다.",
    passwordAlreadyConfigured: "관리자 패널 비밀번호가 이미 설정되었습니다.",
    passwordNotConfigured: "관리자 패널 비밀번호가 아직 설정되지 않았습니다.",
    newPasswordSameAsCurrent: "새 비밀번호는 현재 비밀번호와 같을 수 없습니다.",
    resetHelp:
      "fn-knock 관리자 패널 비밀번호 재설정 도구\n\n사용법:\n  fn-knock-reset-panel-password\n\n작업:\n  - 관리자 패널 비밀번호 지우기\n  - 모든 관리자 패널 로그인 세션 지우기\n  - 로그인 실패 백오프 상태 지우기\n\n완료 후 다음에 관리자 항목을 방문하면 최초 비밀번호 설정 흐름이 다시 시작됩니다.",
    resetCleared: "[fn-knock] 관리자 패널 비밀번호 상태가 지워졌습니다.",
    resetNextVisit:
      "[fn-knock] 다음에 관리자 항목을 방문할 때 관리자 패널 비밀번호를 다시 설정하세요.",
    resetFailed: "[fn-knock] 관리자 패널 비밀번호를 지우지 못했습니다.",
  },
  passkeyRoutes: {
    notFoundWithRetry:
      "패스키를 찾을 수 없습니다. {seconds} 초 후에 다시 시도하세요.",
    verifyFailedWithRetry:
      "확인에 실패했습니다. {seconds} 초 후에 다시 시도하세요.",
    bindTokenExpired: "바인딩 자격 증명이 만료되었습니다.",
    loginMethodUnavailable:
      "현재 로그인 모드에서는 패스키 로그인을 사용할 수 없습니다.",
    loadStatusFailed: "패스키 상태를 불러오지 못했습니다.",
    createOptionsFailed: "패스키 옵션을 만들지 못했습니다.",
    loadPasskeysFailed: "패스키 목록을 불러오지 못했습니다.",
    noPasskeyAvailable: "사용 가능한 패스키가 없습니다.",
    noValidPasskeyAvailable: "유효한 패스키가 없습니다.",
    invalidRpConfig: "패스키 RP 구성이 올바르지 않습니다.",
    invalidResponse: "패스키 응답이 올바르지 않습니다.",
    challengeExpired: "패스키 챌린지가 만료되었습니다.",
    verifyFailed: "패스키를 확인하지 못했습니다.",
    notFound: "패스키를 찾을 수 없습니다.",
    createSessionFailed: "인증 세션을 만들지 못했습니다.",
    loginSuccessful: "로그인했습니다.",
    unauthorizedOrMissingTotp: "권한이 없거나 TOTP ID가 없습니다.",
    createBindTokenFailed: "패스키 바인딩 토큰을 만들지 못했습니다.",
    createRegistrationOptionsFailed: "패스키 등록 옵션을 만들지 못했습니다.",
    registerFailed: "패스키를 등록하지 못했습니다.",
    registrationFailed: "패스키 등록에 실패했습니다.",
    alreadyRegistered: "이미 등록된 패스키입니다.",
    unknownDevice: "알 수 없는 장치",
  },
  authRoutes: {
    pathNotFound: "인증 API 경로를 찾을 수 없습니다.",
    loadBootstrapFailed: "인증 부트스트랩을 불러오지 못했습니다.",
    authenticationRequired: "인증이 필요합니다.",
    loadSessionFailed: "인증 세션을 불러오지 못했습니다.",
    loadCaptchaConfigFailed: "Captcha 구성을 불러오지 못했습니다.",
    createCaptchaChallengeFailed: "Captcha 챌린지를 만들지 못했습니다.",
    loadOidcProvidersFailed: "OIDC 제공자를 불러오지 못했습니다.",
    loadOidcInviteFailed: "OIDC 초대를 불러오지 못했습니다.",
    inspectOidcInviteFailed: "OIDC 초대를 확인하지 못했습니다.",
    loadAuthConfigFailed: "인증 구성을 불러오지 못했습니다.",
    loadLoginCredentialsFailed: "로그인 자격 증명을 불러오지 못했습니다.",
    createSessionFailed: "인증 세션을 만들지 못했습니다.",
    loginSuccessful: "로그인했습니다.",
    loginMethodUnavailable: "현재 로그인 방법을 사용할 수 없습니다.",
    verifyFailed: "인증 상태를 확인하지 못했습니다.",
    localNetworkAccessAllowed: "로컬 네트워크 접근이 허용되었습니다.",
    authenticated: "인증되었습니다.",
    invalidCaptchaProof: "Captcha proof가 올바르지 않습니다.",
    invalidCaptchaAlgorithm: "Captcha 알고리즘이 올바르지 않습니다.",
    invalidCaptchaChallenge: "Captcha 챌린지가 올바르지 않습니다.",
    invalidCaptchaSignature: "Captcha 서명이 올바르지 않습니다.",
    captchaChallengeExpired: "Captcha 챌린지가 만료되었습니다.",
    captchaChallengeAlreadyUsed: "Captcha 챌린지가 이미 사용되었습니다.",
    captchaVerifyFailed: "Captcha를 확인하지 못했습니다.",
    turnstileResponseInvalid: "Turnstile 응답이 올바르지 않습니다.",
    unknownTotp: "알 수 없는 TOTP",
  },
  maintenanceClear: {
    confirmPhrase: "모든 데이터 지우기",
    confirmationMismatch: "확인 문구가 일치하지 않습니다",
    clearFailed: "모든 데이터를 지우지 못했습니다",
  },
  maintenanceBackup: {
    commandMissing: "시스템 명령이 누락되었습니다: {command}",
    commandFailed: "명령 실행 실패: {command}",
    commandCheckFailed: "명령 확인 실패: {command}",
    commandsMissingNoApt:
      "시스템 명령이 누락되었습니다: {commands}. Debian apt-get을 찾을 수 없으므로 자동으로 설치할 수 없습니다.",
    commandsMissingNoPackageManager:
      "시스템 명령이 누락되었습니다: {commands}. opkg 또는 Debian apt-get을 찾을 수 없으므로 자동으로 설치할 수 없습니다.",
    opkgUpdateFailed: "opkg 업데이트 실패",
    aptUpdateFailed: "apt-get 업데이트 실패",
    packageInstallFailed: "{packages}을 설치하지 못했습니다.",
    commandsStillMissingAfterInstall:
      "자동 설치 후에도 명령이 여전히 누락됨: {commands}",
    commandErrorWithDetail: "{message}(종료 코드: {code}): {detail}",
    commandError: "{message}(종료 코드: {code})",
    shareDirectoryMissing:
      "FNOS 공유 디렉터리를 찾을 수 없습니다. 앱 리소스가 올바르게 구성되었는지 확인하세요.",
    invalidBackupPath: "잘못된 백업 파일 경로",
    invalidRedisStreamData: "잘못된 Redis 스트림 데이터 형식: {key}({id})",
    unsupportedRedisExportType:
      "내보내기에 지원되지 않는 Redis 데이터 유형: {type}({key})",
    createArchiveFailed: "백업 아카이브를 생성하지 못했습니다.",
    buildResponseFailed: "백업 다운로드 응답을 만들지 못했습니다.",
    invalidBackupExtension: "백업 파일 확장자는 {extension}이어야 합니다.",
    stringArrayRequired: "{label}은 문자열 배열이어야 합니다.",
    stringArrayOnlyStrings: "{label}은 문자열만 포함할 수 있습니다.",
    objectRequired: "{label}은 객체여야 합니다.",
    fieldStringRequired: "{label}.{field}은 문자열이어야 합니다.",
    arrayRequired: "{label}은 배열이어야 합니다.",
    zsetMemberRequired: "{label}[{index}]에는 문자열 멤버가 포함되어야 합니다.",
    zsetScoreRequired:
      "{label}[{index}]에는 유효한 숫자 점수가 포함되어야 합니다.",
    streamIdRequired: "{label}[{index}]에는 문자열 ID가 포함되어야 합니다.",
    streamFieldsInvalid:
      "{label}[{index}].fields는 길이가 짝수인 비어 있지 않은 문자열 배열이어야 합니다.",
    entryObjectRequired: "항목[{index}]은 객체여야 합니다.",
    entryKeyPrefixRequired: "항목[{index}].key는 {prefix}으로 시작해야 합니다.",
    entryTypeUnsupported: "항목[{index}].type은 지원되지 않습니다.",
    entryTtlInvalid:
      "항목[{index}].ttl_ms는 양의 정수이거나 null이어야 합니다.",
    entryValueStringRequired: "항목[{index}].값은 문자열이어야 합니다.",
    jsonParseFailed: "백업 파일 JSON을 구문 분석할 수 없습니다.",
    payloadObjectInvalid: "백업 파일 콘텐츠가 유효한 개체가 아닙니다.",
    unsupportedSchemaVersion: "버전={version}인 백업 파일만 지원됩니다.",
    unsupportedPrefix: "접두사가 {prefix}인 백업 파일만 지원됩니다.",
    missingAppVersion: "백업 파일에 app_version이 없습니다.",
    appVersionUnsupported:
      "현재 버전 {currentVersion}은 {range}에서 내보낸 백업만 가져올 수 있습니다. {appVersion}을 받았습니다",
    missingExportedAt: "백업 파일이 누락되었습니다.",
    missingEntries: "백업 파일에 항목 배열이 없습니다.",
    duplicateRedisKey: "백업 파일에 중복된 Redis 키가 포함되어 있습니다.",
    archiveMissingPayload: "백업 아카이브가 누락되었습니다. {filename}",
    archivePasswordInvalid: "백업 아카이브 비밀번호 확인에 실패했습니다.",
    readArchiveFailed: ".knock 백업 아카이브를 읽지 못했습니다.",
    payloadUtf8Invalid: "백업 파일 콘텐츠가 유효한 UTF-8 텍스트가 아닙니다.",
    writeRedisFailed: "Redis 백업 데이터를 쓰지 못했습니다.",
    unknownError: "알 수 없는 오류",
    syncSteps: {
      runModeGatewayRoutes: "실행 모드 및 게이트웨이 경로",
      directModeWhitelist: "직접 모드 허용 목록",
      gatewayLogging: "로그 구성 요청",
      wafRuntime: "WAF 구성 및 실행 상태",
      sslDeployment: "SSL 인증서 배포",
      legacyAuthLogCleanup: "기존 인증 로그 정리",
      systemResourceMonitorReset: "시스템 리소스 모니터 상태 재설정",
    },
    archiveEmpty: "백업 아카이브 콘텐츠가 비어 있습니다.",
    archiveTooLarge: "백업 아카이브가 너무 커서 가져올 수 없습니다.",
    directoryImportFileNotFound: "가져올 백업 파일을 찾을 수 없습니다.",
    directoryImportFileUnreadable: "가져올 백업 파일을 읽을 수 없습니다.",
    directoryImportFileOnly: "백업 디렉터리의 파일만 가져올 수 있습니다.",
    directoryImportExtensionOnly: "{extension} 백업 파일만 가져올 수 있습니다.",
    directoryImportTooLarge:
      "백업 파일이 너무 커서 FNOS 디렉터리에서 가져올 수 없습니다.",
    archiveContentMissing: "백업 아카이브 콘텐츠가 누락되었습니다.",
    archiveBase64Invalid: "백업 아카이브가 유효한 Base64 데이터가 아닙니다.",
  },
  captcha: {
    powServerNotConfigured: "PoW 보안 문자가 서버에 구성되어 있지 않습니다.",
    providerMismatch: "보안 문자 유형이 일치하지 않습니다",
    turnstileNotConfigured:
      "Turnstile가 구성되지 않았습니다. 설정을 완료하려면 관리자에게 문의하세요.",
    turnstileSecretMissing:
      "Cloudflare Turnstile secret_key가 구성되지 않았습니다.",
    turnstileTokenRequired: "Turnstile 토큰이 필요합니다",
    turnstileServiceUnavailable:
      "Turnstile 확인 서비스를 일시적으로 이용할 수 없습니다.",
    turnstileVerifyFailedWithReason: "회전식 문 확인 실패: {reason}",
    turnstileVerifyFailed: "Turnstile 확인 실패",
    providerUnavailable: "사용 가능한 보안문자 제공업체를 찾을 수 없습니다.",
    powNotEnabled: "PoW 보안 문자가 활성화되지 않았습니다.",
    powUnavailable: "PoW 보안 문자를 사용할 수 없습니다",
    providerConfigMismatch: "보안 문자 제공자가 현재 구성과 일치하지 않습니다",
  },
  hmac: {
    missingTimestamp: "HMAC 타임스탬프가 없습니다.",
    missingNonce: "HMAC nonce가 없습니다.",
    missingSignature: "HMAC 서명이 없습니다.",
    timestampExpired: "HMAC 타임스탬프가 만료되었습니다.",
    invalidKey: "HMAC 키가 올바르지 않습니다.",
    invalidSignature: "HMAC 서명이 올바르지 않습니다.",
    nonceReused: "HMAC nonce가 이미 사용되었습니다.",
    nonceVerifyFailed: "HMAC nonce 확인에 실패했습니다.",
  },
  cidr: {
    serviceError: "CIDR 서비스 오류",
    emptyResponse: "<빈 응답>",
    upstreamUrl: "업스트림 URL: {url}",
    status: "상태: {status}{statusText}",
    contentType: "콘텐츠 유형: {contentType}",
    upstreamCode: "업스트림 코드: {code}",
    upstreamMessage: "업스트림 메시지: {message}",
    requestId: "요청 ID: {requestId}",
    responsePreview: "응답 미리보기: {preview}",
    provinceRequired: "시/도는 필수 항목입니다.",
    invalidApiUrl: "CIDR API URL이 잘못되었습니다: {error}",
    upstreamTimeout: "CIDR 업스트림 요청 시간이 초과되었습니다.",
    upstreamRequestFailedGeneric: "CIDR 업스트림 요청 실패: {error}",
    upstreamRequestFailed: "CIDR 업스트림 요청 실패({status})",
    invalidJson: "CIDR 업스트림이 잘못된 JSON을 반환했습니다.",
    upstreamUnexpected: "CIDR 업스트림이 예상치 못한 응답을 반환했습니다.",
    provinceWideLabel: "모든 {province}",
    provinceWideUnsupported:
      "저장성과 광둥성은 성 전체 CIDR 선택을 지원하지 않습니다. 도시를 선택하세요.",
    operatorInvalid: "통신사는 Telecom, Unicom 또는 Mobile만 지원됩니다.",
    operatorUnsupported:
      "현재 CIDR 서비스는 통신사 필터링을 지원하지 않습니다. CIDR 컨테이너를 0.1.3 이상으로 업그레이드하세요.",
  },
  dashboard: {
    inbound: "인바운드",
    outbound: "아웃바운드",
    upstreamUnavailable: "업스트림 서비스를 사용할 수 없습니다.",
    hostRequired: "호스트가 필요합니다",
    statsLoadFailed: "대시보드 통계를 불러오지 못했습니다.",
    configLoadFailed: "대시보드 구성을 불러오지 못했습니다.",
    displayConfigSaveFailed: "대시보드 표시 구성을 저장하지 못했습니다.",
  },
  acme: {
    alreadyInstalled: "acme.sh가 이미 설치되어 있습니다.",
    installInProgress: "설치 작업이 이미 진행 중입니다.",
    installSubmitted: "설치 작업이 제출되었습니다.",
    issueSucceeded: "인증서가 성공적으로 발급되었습니다.",
  },
  ddns: {
    ipv6OnlyUnavailable:
      "업데이트 범위는 IPv6 전용이지만 사용 가능한 IPv6 주소가 감지되지 않았습니다.",
    ipv4OnlyUnavailable:
      "업데이트 범위는 IPv4뿐이지만 사용 가능한 IPv4 주소가 감지되지 않았습니다.",
    dualStackUnavailable:
      "업데이트 범위 내에서 사용 가능한 IPv4 또는 IPv6 주소가 감지되지 않았습니다.",
    domainConfigIncomplete: "도메인 구성이 불완전합니다.",
    domainNotInZone: "도메인 {fqdn}은 루트 영역 {zone}에 속하지 않습니다.",
    invalidJsonResponse: "응답이 유효한 JSON이 아닙니다: {text}",
    aRecordFailed: "레코드 처리에 실패했습니다.",
    aaaaRecordFailed: "AAAA 레코드 처리 실패",
    providerDnsUpdateSuccess: "{provider} DNS 업데이트 성공",
    aliyunParamKeyMissing: "Aliyun 요청 매개변수에 키 이름이 누락되었습니다.",
    requestFailed: "요청 실패",
    tencentMissingResponse:
      "HTTP {status}: Tencent Cloud API 응답이 누락되었습니다. 응답",
    invalidHeaderFormat: "잘못된 헤더 형식: {header}",
    publicCheckSourceEmpty: "{family} 공개 탐지 주소는 비워 둘 수 없습니다.",
    publicCheckSourceInvalidUrl: "잘못된 {family} 공개 탐지 주소: {source}",
    publicCheckSourceUnsupportedProtocol:
      "{family} 공개 탐지 주소는 HTTP/HTTPS만 지원합니다: {source}",
    publicCheckSourceListEmpty:
      "{family} 공개 탐지 주소가 설정되지 않았습니다.",
    publicCheckSourceRequestFailed: "탐지 소스 {url} 요청 실패: HTTP {status}",
    publicCheckSourceInvalidPayload:
      "탐지 소스 {url}이(가) 유효한 {family} 주소를 반환하지 않았습니다.",
    publicCheckTestFailed: "공개 탐지 주소 테스트 실패",
    publicDnsResolveFailed:
      "공용 DNS로 {host}의 {family} 주소를 확인하지 못했습니다: {detail}",
    publicDnsNoAddress:
      "공용 DNS가 {host}의 {family} 주소를 반환하지 않았습니다.",
    publicDnsNoUsableServer:
      "선택한 인터페이스에서 공용 DNS 서버에 연결할 수 없습니다.",
    publicCheckTimeout: "공개 탐지 요청 시간이 초과되었습니다.",
    publicCheckTooManyRedirects:
      "공개 탐지 요청의 리디렉션 횟수가 너무 많습니다.",
    interfaceSourceLabel: "인터페이스 {name}",
    selectedInterfaceSourceLabel: "선택된 인터페이스",
    publicSourceLabel: "공용 네트워크",
    staticSourceLabel: "고정 IP",
    domainSourceLabel: "도메인 {domain}",
    domainSourceLabelEmpty: "원본 도메인",
    staticIpv4Invalid: "잘못된 고정 IPv4 주소: {value}",
    staticIpv6Invalid: "잘못된 고정 IPv6 주소: {value}",
    sourceDomainRequired: "확인할 원본 도메인을 입력하세요.",
    sourceDomainInvalid: "원본 도메인 형식이 올바르지 않습니다: {domain}",
    sourceDomainResolveFailed: "원본 도메인 {domain} 확인 실패: {error}",
    singleAddressProviderUnsupported:
      "{provider}은(는) 한 번에 하나의 주소만 업데이트할 수 있습니다. 업데이트 범위를 IPv4 전용 또는 IPv6 전용으로 설정하세요.",
    interfaceIpv6Unavailable:
      "IP 소스가 직접 인터페이스이지만 선택한 인터페이스에 사용 가능한 IPv6 주소가 없습니다.",
    interfaceIpv4Unavailable:
      "IP 소스가 직접 인터페이스이지만 선택한 인터페이스에 사용 가능한 IPv4 주소가 없습니다.",
    interfaceDualStackUnavailable:
      "IP 소스가 직접 인터페이스이지만 선택한 인터페이스에 사용 가능한 IPv4 또는 IPv6 주소가 없습니다.",
    publicIpv6Unavailable:
      "IP 소스가 공용 네트워크이지만 사용 가능한 IPv6 주소를 얻지 못했습니다.",
    publicIpv4Unavailable:
      "IP 소스가 공용 네트워크이지만 사용 가능한 IPv4 주소를 얻지 못했습니다.",
    publicDualStackUnavailable:
      "IP 소스가 공용 네트워크이지만 사용 가능한 IPv4 또는 IPv6 주소를 얻지 못했습니다.",
    staticIpv6Unavailable:
      "IP 소스가 고정 IP이지만 사용 가능한 IPv6 주소가 입력되지 않았습니다.",
    staticIpv4Unavailable:
      "IP 소스가 고정 IP이지만 사용 가능한 IPv4 주소가 입력되지 않았습니다.",
    staticDualStackUnavailable:
      "IP 소스가 고정 IP이지만 사용 가능한 IPv4 또는 IPv6 주소가 입력되지 않았습니다.",
    domainIpv6Unavailable:
      "IP 소스가 도메인 확인이지만 사용 가능한 IPv6 주소를 확인하지 못했습니다.",
    domainIpv4Unavailable:
      "IP 소스가 도메인 확인이지만 사용 가능한 IPv4 주소를 확인하지 못했습니다.",
    domainDualStackUnavailable:
      "IP 소스가 도메인 확인이지만 사용 가능한 IPv4 또는 IPv6 주소를 확인하지 못했습니다.",
    selectInterfaceAddress:
      "직접 인터페이스 모드를 사용하기 전에 {family} 주소를 선택하세요.",
    selectedInterfaceAddressUnavailable:
      "선택한 인터페이스의 {index} {family} 주소는 더 이상 사용할 수 없습니다. 다시 선택하세요.",
    ipv4FailedContinueIpv6: "IPv4 감지에 실패했습니다. IPv6({error}) 계속",
    ipv4Failed: "IPv4 감지 실패({error})",
    ipv6FailedContinueIpv4: "IPv6 감지에 실패했습니다. IPv4({error})로 계속",
    ipv6Failed: "IPv6 감지 실패({error})",
    publicIpv6NotSelectable:
      "공개 감지에서 IPv6({ip})을 반환했지만 이 시스템이나 Docker 호스트에서 선택할 수 있는 인터페이스 주소에 포함되어 있지 않습니다. 외부에서 연결할 수 없는 경우 직접 인터페이스 모드를 사용하고 호스트 공용 IPv6을 선택합니다.",
    interfaceRequired:
      "직접 인터페이스 모드를 사용하기 전에 아웃바운드 인터페이스를 선택하세요.",
    interfaceNotFound: "사용 가능한 인터페이스를 찾을 수 없습니다: {name}",
    dockerHostInterfaceLabel: "호스트 {name}({summary})",
    curlStatusLineParseFailed:
      "컬 응답 상태 줄을 구문 분석할 수 없습니다: {line}",
    curlNoHeaders: "컬이 응답 헤더를 반환하지 않았습니다.",
    requestCanceled: "요청이 취소되었습니다.",
    curlRequestFailed: "컬 요청 실패: {detail}",
    nodeTransportInterfaceAddressUnavailable:
      "내장 HTTP 요청을 인터페이스 {name}에 바인딩할 수 없습니다: 사용 가능한 {family} 로컬 주소가 없습니다.",
    nodeTransportInterfaceNoAddress:
      "내장 HTTP 요청을 인터페이스 {name}에 바인딩할 수 없습니다: 사용 가능한 로컬 주소가 없습니다.",
    nodeTransportUnsupportedProtocol:
      "내장 HTTP 요청은 이 프로토콜을 지원하지 않습니다: {protocol}",
    nodeTransportRedirectLimitExceeded:
      "내장 HTTP 요청 리디렉션 횟수가 한도 {max}회를 초과했습니다.",
    triggerCron: "예정된 점검",
    triggerEnable: "자동 업데이트 활성화 후 즉시 확인",
    triggerStartup: "시작 후 확인",
    triggerMessage: "{trigger}: {message}",
    notConfigured: "구성되지 않음",
    skippedNoProvider: "DDNS 공급자가 선택되지 않았습니다. 건너뛰었습니다",
    skippedIncompleteConfig: "현재 구성이 불완전합니다. 건너뛰었습니다",
    skippedPublicIpUnavailable: "공개 IP를 얻을 수 없습니다. 건너뛰었습니다",
    skippedReason: "{reason}; 건너뛰었습니다",
    targetIpNoChange:
      "대상 IP는 변경되지 않았습니다. 업데이트가 필요하지 않습니다",
    none: "없음",
    ipChange: "{family}: {before} -> {after}",
    targetIpChanged: "감지된 대상 IP 변경: {changes}",
    dnsUpdateSuccess: "DNS 업데이트 성공 [{provider}]: {message}",
    dnsUpdateFailed: "DNS 업데이트 실패 [{provider}]: {message}",
    taskError: "작업 오류: {message}",
    intervalOutOfRange:
      "자동 동기화 간격은 {min}에서 {max}분 사이의 정수여야 합니다.",
    primaryDomainName: "기본 도메인",
    noProviderSelected: "선택한 제공업체가 없습니다.",
    duplicateTarget:
      "공급자 및 도메인 요약이 동일한 DDNS 항목이 이미 존재합니다.",
    domainTargets: {
      invalidDomain: "전체 도메인 형식이 올바르지 않습니다: {domain}",
      tooMany: "전체 도메인은 최대 두 개까지 설정할 수 있습니다",
      invalidPair:
        "전체 도메인 두 개는 와일드카드와 대응하는 기준 도메인 조합이어야 합니다",
      mismatchedPair: "와일드카드 도메인과 기준 도메인이 일치하지 않습니다",
      pairUnsupported:
        "{provider}는 와일드카드와 기준 도메인의 동시 업데이트를 지원하지 않습니다",
      rootMissing:
        "와일드카드와 기준 도메인 조합을 사용하기 전에 {field}을(를) 설정하세요",
      rootMismatch:
        "조합의 기준 도메인이 {field} 관리 범위를 벗어났습니다(Zone {expected}, 조합 {actual})",
      allSucceeded: "도메인 {count}개",
      itemSucceeded: "{domain}: 성공",
      itemFailed: "{domain}: 실패({detail})",
    },
    primaryInitFailed: "기본 DDNS 항목을 초기화하지 못했습니다.",
    primaryDomainScope: "기본 도메인",
    additionalDomainScope: "추가 도메인",
    targetNotFound: "DDNS 항목을 찾을 수 없습니다",
    unknownProvider: "알 수 없는 DDNS 제공업체: {provider}",
    primaryDeleteForbidden: "기본 도메인 항목은 삭제할 수 없습니다.",
    primaryDisableForbidden:
      "기본 도메인 항목은 단독으로 비활성화할 수 없습니다.",
    unknownProviderShort: "알 수 없는 제공자: {provider}",
    selectProviderFirst: "먼저 DDNS 제공업체를 선택하세요.",
    primaryConfigIncomplete:
      "현재 기본 도메인 구성이 불완전합니다. 필수 입력란을 모두 작성하세요.",
    targetConfigIncomplete:
      "현재 항목 구성이 불완전합니다. 필수 입력란을 모두 작성하세요.",
    manualTestStart:
      "수동 테스트가 시작되었습니다. 현재 대상 IP를 확인하는 중...",
    manualTestPrefix: "수동 테스트",
    currentTargetIp: "현재 대상 IP({source}) — IPv4: {ipv4}, IPv6: {ipv6}",
    testAborted: "{message}; 테스트가 중단되었습니다",
    updateSuccess: "업데이트 성공: {message}",
    updateFailed: "업데이트 실패: {message}",
    testError: "테스트 오류: {message}",
    statusLoadFailed: "DDNS 상태를 불러오지 못했습니다.",
    toggleFailed: "DDNS 활성화 상태를 업데이트하지 못했습니다.",
    settingsLoadFailed: "DDNS 자동 동기화 설정을 불러오지 못했습니다.",
    settingsSaveFailed: "DDNS 자동 동기화 설정을 저장하지 못했습니다.",
    logsLoadFailed: "DDNS 로그를 불러오지 못했습니다.",
    logsClearFailed: "DDNS 로그를 지우지 못했습니다.",
    pollFailed: "DDNS 로그와 상태를 폴링하지 못했습니다.",
    providerSetFailed: "공급자를 설정하지 못했습니다.",
    configSaveFailed: "DDNS 구성을 저장하지 못했습니다.",
    createTargetFailed: "DDNS 항목을 생성하지 못했습니다.",
    updateTargetFailed: "DDNS 항목을 업데이트하지 못했습니다.",
    deleteTargetFailed: "DDNS 항목을 삭제하지 못했습니다.",
    updateTargetEnabledFailed:
      "DDNS 항목 활성화 상태를 업데이트하지 못했습니다.",
    providers: {
      common: {
        fields: {
          root_domain: {
            label: "루트 도메인",
            description: "example.com과 같은 영역을 결정하는 데 사용됩니다.",
          },
          domain: {
            label: "전체 도메인",
            shortLabel: "도메인",
            description: "업데이트할 전체 도메인 이름",
            hostDescription: "업데이트할 전체 호스트 이름",
          },
          ttl: {
            description: "기본 {seconds} 초",
          },
          access_key_id: {
            label: "액세스 키 ID",
            description:
              "DNS 레코드 읽기/쓰기 권한이 있는 클라우드 공급자 액세스 키 ID",
          },
          access_key_secret: {
            label: "액세스 키 Secret",
            description: "액세스 키 ID와 함께 사용하는 Secret",
          },
          secret_access_key: {
            label: "액세스 키 Secret",
            description: "액세스 키 ID와 함께 사용하는 Secret",
          },
          secret_id: {
            label: "SecretId",
            description:
              "선택한 DNS 서비스 권한이 있는 Tencent Cloud API SecretId",
          },
          secret_key: {
            label: "SecretKey",
            description: "SecretId와 함께 사용하는 Tencent Cloud API SecretKey",
          },
          api_key: {
            label: "API 키",
            description: "공급자 콘솔에서 생성한 API Key",
          },
          api_secret: {
            label: "API Secret",
            description: "API Key와 함께 사용하는 API Secret",
          },
          secret_api_key: {
            label: "Secret API Key",
            description: "Porkbun 콘솔에서 생성한 Secret API Key",
          },
          api_token: {
            label: "API 토큰",
            description: "공급자 콘솔에서 생성한 API Token",
          },
          token_id: {
            label: "Token ID",
            description: "DNSPod 콘솔에서 생성한 API Token ID",
          },
          token_key: {
            label: "Token Key",
            description: "DNSPod 콘솔에서 생성한 API Token Key",
          },
          zone_id: {
            label: "Zone ID",
            description: "공급자 콘솔의 Zone 또는 사이트 ID",
          },
        },
      },
      dynv6: {
        fields: {
          token: {
            description: "dynv6.com 계정에서 생성됨",
          },
          zone: {
            label: "구역 이름",
            description: "dynv6 영역 도메인",
          },
          ipv6prefix: {
            description: "선택사항. dynv6 API로 전달됨",
          },
        },
        configIncomplete: "dynv6 구성이 불완전합니다",
        empty: "(비어 있음)",
        success: "dynv6: {detail}(전송: {params})",
        updateFailed: "dynv6 업데이트 실패 [{status}]: {detail}",
        requestError: "dynv6 요청 오류: {detail}",
      },
      duckdns: {
        fields: {
          domains: {
            label: "하위 도메인",
            description:
              ".duckdns.org 접미사 없이 DuckDNS 하위 도메인만 입력하세요. 쉼표로 구분된 값이 지원됩니다.",
          },
          token: {
            description: "DuckDNS 콘솔 홈 페이지에 표시된 계정 토큰",
          },
        },
        configIncomplete: "DuckDNS 구성이 불완전합니다.",
        noIpAvailable:
          "DuckDNS 업데이트 실패: 사용 가능한 IPv4 또는 IPv6 주소가 없습니다.",
        updateFailedWithStatus: "DuckDNS 업데이트 실패 [{status}]: {detail}",
        requestFailed: "요청 실패",
        updateFailed: "DuckDNS 업데이트 실패: {detail}",
        nonOkResponse: "OK가 아닌 응답을 반환함",
        success: "DuckDNS 업데이트 성공{detail}",
        requestError: "DuckDNS 요청 오류: {detail}",
      },
      dnspod: {
        fields: {
          record_line: {
            label: "라인",
            description: "기본적으로 기본 줄을 사용합니다.",
          },
        },
        defaultLine: "기본값",
        configIncomplete: "DNSPod 구성이 불완전합니다.",
        queryRecordFailed: "레코드를 쿼리하지 못했습니다.",
        updateRecordFailed: "기록을 업데이트하지 못했습니다.",
        createRecordFailed: "레코드를 생성하지 못했습니다.",
      },
      cloudflare: {
        fields: {
          api_token: {
            label: "API 토큰",
            description: "Zone.DNS 편집 권한이 필요합니다.",
          },
          zone_id: {
            description:
              "Cloudflare 도메인 페이지에서 세 개의 점을 클릭하고 영역 ID 복사를 선택하세요.",
          },
          proxied: {
            label: "Cloudflare 프록시",
            description: "Cloudflare 프록시 활성화 여부(주황색 구름)",
            options: {
              dnsOnly: "DNS만",
              orangeCloud: "주황색 구름",
            },
          },
        },
        configIncomplete: "Cloudflare 구성이 불완전합니다",
        zoneLookupFailed: "Cloudflare Zone을 조회하지 못했습니다: {detail}",
        zoneMismatch:
          "조합의 기준 도메인이 Cloudflare Zone을 벗어났습니다(Zone {expected}, 조합 {actual})",
        searchRecordFailed: "{type} 레코드 쿼리 실패: {detail}",
        updateRecordFailed: "{type} 레코드 업데이트 실패: {detail}",
        createRecordFailed: "{type} 레코드 생성 실패: {detail}",
        recordOperationError: "{type} 레코드 작업 오류: {detail}",
        success: "Cloudflare DNS 업데이트 성공",
      },
      godaddy: {
        configIncomplete: "GoDaddy 구성이 불완전합니다",
        updateFailed: "업데이트 실패",
        updateFailedWithStatus: "[{status}] {detail}",
      },
      porkbun: {
        configIncomplete: "돼지고기 구성이 불완전합니다",
        queryRecordFailed: "레코드를 쿼리하지 못했습니다.",
        updateRecordFailed: "기록을 업데이트하지 못했습니다.",
        createRecordFailed: "레코드를 생성하지 못했습니다.",
      },
      alidns: {
        label: "알리윤 DNS",
        fields: {
          access_key_secret: {
            placeholder: "Aliyun AccessKey Secret",
          },
          line: {
            label: "라인",
            description: '기본적으로 Aliyun "기본" 라인을 사용합니다.',
          },
        },
        configIncomplete: "Aliyun DNS 구성이 불완전합니다",
        requestFailed: "요청 실패",
        updateFailed: "업데이트 실패",
        createFailed: "생성 실패",
        recordIdMissing: "Aliyun DNS가 RecordId 없는 레코드를 반환했습니다.",
      },
      baidu: {
        label: "바이두 클라우드 DNS",
        fields: {
          access_key_id: {
            placeholder: "Baidu AI 클라우드 액세스 키",
          },
          secret_access_key: {
            placeholder: "Baidu AI 클라우드 비밀 키",
          },
        },
        configIncomplete: "Baidu Cloud DNS 구성이 불완전합니다.",
        queryFailed: "쿼리 실패",
        updateFailed: "업데이트 실패",
        createFailed: "생성 실패",
      },
      huawei: {
        label: "화웨이 클라우드 DNS",
        fields: {
          access_key_id: {
            placeholder: "화웨이 클라우드 AK",
          },
          secret_access_key: {
            placeholder: "화웨이 클라우드 SK",
          },
        },
        webCryptoUnsupported:
          "현재 런타임은 Web Crypto를 지원하지 않으므로 Huawei Cloud AK/SK 서명을 생성할 수 없습니다.",
        configIncomplete: "Huawei Cloud DNS 구성이 불완전합니다",
        requestFailed:
          "Huawei Cloud DNS 요청 실패: HTTP {status} {statusText}, {detail}",
        zoneNotFound: "화웨이 클라우드 존을 찾을 수 없습니다: {zone}",
        recordsetIdMissing:
          "Huawei Cloud DNS가 ID 없는 레코드셋을 반환했습니다.",
      },
      tencentcloud: {
        label: "Tencent Cloud DNS",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          record_line: {
            label: "라인",
            description: "기본적으로 기본 줄을 사용합니다.",
          },
          record_line_id: {
            label: "라인 ID",
            description: "선택사항. 설정된 경우 Line ID가 우선순위를 갖습니다.",
          },
        },
        defaultLine: "기본값",
        configIncomplete: "Tencent Cloud DNS 구성이 불완전합니다.",
        missingUpdatedRecordId:
          "Tencent Cloud가 업데이트된 RecordId를 반환하지 않았습니다.",
        missingCreatedRecordId:
          "Tencent Cloud가 생성된 RecordId를 반환하지 않았습니다.",
      },
      noip: {
        fields: {
          hostname: {
            description:
              "전체 호스트 이름을 입력하세요. 여러 호스트 이름은 쉼표로 구분할 수 있습니다.",
          },
          username: {
            label: "사용자 이름",
            description:
              "NO-IP 콘솔에서 생성된 DDNS 키 사용자 이름을 사용하세요.",
          },
          password: {
            label: "비밀번호",
            description:
              "기본 계정 비밀번호가 아닌 DDNS Key와 페어링된 비밀번호를 사용하세요.",
          },
        },
        statusMessages: {
          "911":
            "NO-IP에는 일시적인 서버 측 오류가 있습니다. 공식적인 지침은 최소 30분 후에 다시 시도하는 것입니다.",
          nohost:
            "지정된 호스트 이름이 존재하지 않거나 현재 DDNS 키에 속하지 않습니다.",
          badauth: "사용자 이름 또는 비밀번호가 올바르지 않습니다.",
          badagent:
            "NO-IP로 인해 클라이언트가 비활성화되었습니다. 사용자 에이전트 또는 클라이언트 상태를 확인하세요.",
          "!donator": "현재 계정은 이 요청의 향상된 기능을 지원하지 않습니다.",
          abuse: "이 DDNS 키는 남용으로 인해 NO-IP에 의해 차단되었습니다.",
        },
        unknownStatus: "반환된 알 수 없는 상태: {code}",
        updateFailed: "NO-IP 업데이트 실패: {detail}",
        updateSuccess: "NO-IP 업데이트 성공{detail}",
        ipUnchanged: "NO-IP IP가 변경되지 않았습니다{detail}",
        configIncomplete: "NO-IP 구성이 불완전합니다.",
        noIpAvailable:
          "NO-IP 업데이트 실패: 사용 가능한 IPv4 또는 IPv6 주소가 없습니다.",
        updateFailedWithStatus: "NO-IP 업데이트 실패 [{status}]: {detail}",
        requestFailed: "요청 실패",
        emptyResponse: "NO-IP 업데이트 실패: 빈 응답을 반환했습니다.",
        requestError: "NO-IP 요청 오류: {detail}",
      },
      esa: {
        label: "알리윤 ESA DNS",
        fields: {
          access_key_secret: {
            placeholder: "Aliyun AccessKey Secret",
          },
          site_name: {
            label: "사이트 이름",
            description:
              "ESA 사이트 이름은 일반적으로 루트 도메인입니다. 사이트 ID가 설정된 경우 이는 대체 조회로만 사용됩니다.",
          },
          site_id: {
            description:
              "선택사항. 설정 시 사이트 목록을 먼저 조회하지 않고 해당 사이트가 직접 운영됩니다.",
          },
          proxied: {
            label: "ESA 프록시",
            description:
              "기본적으로 DNS 전용입니다. 프록시가 활성화되면 비즈니스 유형이 자동으로 전송됩니다.",
            options: {
              dnsOnly: "DNS만",
              enabled: "프록시 활성화",
            },
          },
          biz_name: {
            label: "사업 유형",
            description:
              "ESA 프록시가 활성화된 경우에만 적용됩니다. 기본값은 웹입니다.",
            options: {
              web: "웹",
              api: "API",
              imageVideo: "오디오/비디오",
            },
          },
        },
        configIncomplete: "Aliyun ESA DNS 구성이 불완전합니다.",
        siteNameMissing: "Aliyun ESA DNS 사이트 이름이 없습니다.",
        siteLookupFailed: "Aliyun ESA 사이트를 조회하지 못했습니다: {detail}",
        siteMismatch:
          "설정된 Site ID가 사이트 조회 결과와 일치하지 않습니다(설정 {expected}, 조회 {actual})",
        siteNotFound: "ESA 사이트를 찾을 수 없습니다: {site}",
        noIpAvailable: "Aliyun ESA DNS에 업데이트할 IP 주소가 없습니다.",
        createRecordFailed: "CreateFailed: 레코드를 생성하지 못했습니다.",
        success: "Aliyun ESA DNS 업데이트 성공",
        recordIdMissing: "업데이트 실패: 레코드에 RecordId가 없습니다.",
      },
      dynu: {
        fields: {
          api_key: {
            description: "Dynu API 자격 증명에서 생성된 API 키",
          },
          domain: {
            description:
              "업데이트할 전체 Dynu 호스트 이름입니다. 와일드카드/기준 도메인 조합의 기준 도메인은 다른 Service의 일반 하위 도메인이 아니라 Dynu에 독립 DDNS Service로 등록되어 있어야 합니다. 업데이트 시 별도의 기준 도메인 레코드를 만들지 않고 IP를 설정하고 Wildcard Alias를 활성화합니다.",
          },
          group: {
            description: "선택사항. Dynu DNS 레코드에 기록된 그룹입니다.",
          },
        },
        actionFailed: "{action} 실패",
        actions: {
          resolveRoot: "Dynu 루트 도메인 확인",
          readDnsService: "Dynu DNS 서비스 읽기",
          updateWildcardAlias: "Dynu 와일드카드 별칭 업데이트",
          queryRecord: "Dynu {type} 레코드 쿼리",
          updateRecord: "Dynu {type} 레코드 업데이트",
          createRecord: "Dynu {type} 레코드 생성",
        },
        invalidRootInfo:
          "Dynu가 유효한 루트 도메인 정보를 반환하지 않았습니다.",
        wildcardUnsupported:
          "Dynu REST는 *.{domain}을 DNS 레코드 nodeName으로 사용하는 것을 지원하지 않습니다. Dynu DDNS 서비스에서 {domain}을 독립 서비스로 추가하고 와일드카드 별칭을 활성화하거나 DDNS 구성을 {domain}으로 변경하세요.",
        wildcardUnchanged: "Dynu Wildcard 별칭 IP가 변경되지 않았습니다.",
        wildcardSuccess: "Dynu Wildcard 별칭 업데이트 성공",
        configIncomplete: "Dynu 구성이 불완전합니다",
        noIpAvailable:
          "Dynu 업데이트 실패: 사용 가능한 IPv4 또는 IPv6 주소가 없습니다.",
        recordIdMissing: "Dynu DNS 레코드에 RecordId가 없습니다.",
        requestError: "Dynu 요청 오류: {detail}",
      },
      edgeone: {
        label: "Tencent Cloud EdgeOne",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          zone_id: {
            description: "호스팅된 영역을 찾는 데 사용되는 EdgeOne 사이트 ID",
          },
          domain: {
            description:
              "업데이트할 전체 호스트 이름입니다. 먼저 국제화된 도메인 이름을 퓨니코드로 변환하세요.",
          },
          location: {
            label: "라인",
            placeholder: "기본값 또는 CN.BJ",
            description: "선택사항. 기본 글로벌 라인을 사용하려면 비워 두세요.",
          },
          ttl: {
            description:
              "기본값은 300초입니다. EdgeOne은 60-86400을 허용합니다.",
          },
          overseas_access: {
            label: "해외 접근 통제",
            description:
              "활성화되면 EdgeOne 보안 정책 API가 해외 IP 액세스를 차단합니다. 홍콩, 마카오, 대만은 해외로 간주되지 않습니다. 이는 구성이 변경될 때 한 번 동기화되며 모든 DDNS 업데이트에서 반복되지 않습니다.",
            options: {
              off: "끄기",
              blockOverseas: "해외 IP 차단",
            },
          },
          endpoint: {
            description:
              "기본값은 본토 엔드포인트입니다. https://teo.intl.tencentcloudapi.com 또는 지역 엔드포인트를 사용할 수 있습니다.",
          },
          region: {
            placeholder: "비어 있음",
            description:
              "선택사항. 대부분의 시나리오에서는 이 항목을 비워 둘 수 있습니다.",
          },
        },
        configIncomplete: "Tencent Cloud EdgeOne 구성이 불완전합니다.",
        zoneLookupFailed: "EdgeOne 사이트를 조회하지 못했습니다: {detail}",
        zoneMismatch:
          "조합의 기준 도메인이 EdgeOne Zone을 벗어났습니다(Zone {expected}, 조합 {actual})",
        configTargetIncomplete:
          "Tencent Cloud EdgeOne 구성이 불완전합니다: 영역 ID 또는 도메인이 누락되었습니다.",
        missingRecordId: "EdgeOne이 RecordId 없이 레코드를 반환했습니다.",
        missingCreatedRecordId:
          "EdgeOne이 생성된 RecordId를 반환하지 않았습니다.",
        overseasAccess: {
          describeRulesFailed:
            "EdgeOne 해외 액세스 제어가 기존 사용자 정의 규칙을 읽지 못했습니다(provider_target={target}, zone_id={zoneId}, endpoint_host={endpointHost}, region={region}, entity={entity}, scope={scope}): {message}",
          syncFailedWithAttempt:
            "EdgeOne 해외 액세스 제어 동기화 실패({attempt}, submit_rule_count={count}): {message}",
          syncAllScopesFailed:
            "EdgeOne 해외 액세스 제어 동기화 실패: 모든 규칙 범위 실패",
          cleanupAllScopesFailed:
            "EdgeOne 해외 액세스 제어 정리 실패: 모든 규칙 범위 실패",
          syncSuccess:
            "EdgeOne 해외 IP 차단 정책이 동기화되었습니다. 중국 본토, 홍콩, 마카오, 대만만 허용됩니다.",
          cleanupSuccess: "EdgeOne 해외 IP 차단 정책이 해제되었습니다.",
        },
      },
      edgeone_cname: {
        label: "Tencent Cloud EdgeOne(CNAME 액세스)",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          zone_id: {
            description:
              "가속 도메인 사이트를 찾는 데 사용되는 EdgeOne 사이트 ID",
          },
          domain: {
            label: "가속 영역",
            description:
              "EdgeOne에는 이미 가속 도메인이 생성되어 있습니다. IP_DOMAIN 원본만 지원되며 한 번에 하나의 원본 주소만 업데이트할 수 있습니다.",
          },
          overseas_access: {
            label: "해외 접근 통제",
            description:
              "활성화되면 EdgeOne 보안 정책 API가 해외 IP 액세스를 차단합니다. 홍콩, 마카오, 대만은 해외로 간주되지 않습니다. 이는 구성이 변경될 때 한 번 동기화되며 모든 DDNS 업데이트에서 반복되지 않습니다.",
            options: {
              off: "끄기",
              blockOverseas: "해외 IP 차단",
            },
          },
          endpoint: {
            description:
              "기본값은 본토 엔드포인트입니다. https://teo.intl.tencentcloudapi.com 또는 지역 엔드포인트를 사용할 수 있습니다.",
          },
          region: {
            placeholder: "비어 있음",
            description:
              "선택사항. 대부분의 시나리오에서는 이 항목을 비워 둘 수 있습니다.",
          },
        },
        configIncomplete:
          "Tencent Cloud EdgeOne(CNAME 액세스) 구성이 불완전합니다.",
        singleAddressOnly:
          'Tencent Cloud EdgeOne(CNAME 액세스)은 한 번에 하나의 원본 주소만 업데이트할 수 있습니다. DDNS 업데이트 범위를 "IPv4 전용" 또는 "IPv6 전용"으로 설정하세요.',
        noIpAvailable:
          "Tencent Cloud EdgeOne(CNAME 액세스)에 업데이트할 IP 주소가 없습니다.",
        domainNotFound: "EdgeOne 가속 도메인을 찾을 수 없음: {domain}",
        unsupportedOriginType:
          "현재 가속 도메인 원점 유형은 {originType}입니다. IP_DOMAIN 가속 도메인만 DDNS로 업데이트할 수 있습니다.",
        originUnchanged:
          "Tencent Cloud EdgeOne(CNAME 액세스) 원본이 이미 최신 상태입니다.",
        successWithInvalidHostHeaderIgnored:
          "Tencent Cloud EdgeOne(CNAME 액세스) 원본이 성공적으로 업데이트되었습니다(잘못된 호스트 헤더 무시).",
        success:
          "Tencent Cloud EdgeOne(CNAME 액세스) 원본이 성공적으로 업데이트되었습니다.",
      },
    },
  },
  smartConnect: {
    runTypes: {
      direct: "직접 모드",
      reverseProxy: "역방향 프록시 모드",
      subdomain: "하위 도메인 모드",
    },
    currentMode: "현재 모드",
    unavailableReason:
      "하위 도메인 모드만 사용할 수 있습니다. 현재 모드: {mode}.",
    selectLocalIp: "로컬 LAN IP를 선택하세요",
    selectValidLocalIpv4: "유효한 로컬 LAN IPv4 주소를 선택하세요.",
    dnsmasqNotInstalled: "dnsmasq가 감지되지 않았습니다. 먼저 설치하세요.",
    dnsmasqNotInitialized:
      "dnsmasq가 초기화를 완료하지 않았습니다. 먼저 환경 초기화를 완료하세요.",
    syncFailed: "스마트 연결 동기화 실패",
  },
  scanDiscovery: {
    localIpv4CidrOnly: "스캔 범위는 로컬 IPv4 CIDR: {cidrs}만 지원합니다.",
    maxCidrsExceeded: "한 번에 최대 {max} 스캔 범위를 선택하세요.",
    maxHostsExceededWithCurrent:
      "한 번에 최대 {max} 호스트를 스캔합니다. 현재 선택에는 {current} 호스트가 있습니다",
    maxHostsExceeded: "한 번에 최대 {max} 호스트를 스캔하세요.",
    selectAtLeastOneCidr: "로컬 IPv4 스캔 범위를 하나 이상 선택하세요.",
    scanJobNotFound: "스캔 작업을 찾을 수 없거나 만료되었습니다.",
    loadTargetsFailed: "스캔 대상을 불러오지 못했습니다.",
    loadConfigFailed: "설정을 불러오지 못했습니다.",
    saveTargetsFailed: "스캔 대상을 저장하지 못했습니다.",
    loadSettingsFailed: "검색 설정을 불러오지 못했습니다.",
    saveSettingsFailed: "검색 설정을 저장하지 못했습니다.",
    invalidIntensityMode: "잘못된 검색 강도 모드입니다.",
    invalidIntensityLevel: "잘못된 검색 강도 단계입니다.",
    targetLabels: {
      docker: "{cidr}(Docker 호스트 LAN)",
      loopback: "{cidr}(로컬 루프백)",
      interface: "{cidr} ({name})",
      mapping: "{cidr} (기존 매핑 대상)",
      custom: "{cidr}(사용자 정의)",
      saved: "{cidr}(저장됨)",
    },
    serviceLabels: {
      lottery: "복권 도우미",
      dlymusic: "다오리위 뮤직 매니저",
      kuake: "쿼크 자동 전송",
      xunlei: "순레이",
      nowen: "성운 포털",
      fnos: "FNOS",
      fnys: "FNOS 비디오",
      xiaoyaAlist: "샤오야 알리리스트",
    },
  },
  gatewayProxyHeaders: {
    runTypes: {
      direct: "직접 모드",
      reverseProxy: "역방향 프록시 모드",
      subdomain: "하위 도메인 모드",
    },
    unavailableReason:
      "하위 도메인 모드만 사용할 수 있습니다. 현재 모드: {mode}.",
    syncFailed: "게이트웨이 프록시 헤더 구성을 동기화하지 못했습니다.",
  },
  sshSecurity: {
    logSourceUnavailable:
      "이 시스템에서 Journalctl 또는 /var/log/auth.log를 찾을 수 없습니다.",
    openWrtUnsupported: "OpenWrt 빌드에서는 아직 SSH 보안이 지원되지 않습니다.",
    enableUnavailable: "이 환경에서는 SSH 보안을 활성화할 수 없습니다.",
    syncFirewallUnavailable: "이 환경에서는 SSH 방화벽을 동기화할 수 없습니다.",
    clearFirewallUnavailable: "이 환경에서는 SSH 방화벽을 지울 수 없습니다.",
    logSourceUnavailableShort: "SSH 로그 소스를 사용할 수 없습니다.",
    customCidrInvalid: "맞춤 CIDR 형식이 잘못되었습니다. {cidrs}",
    customCidrsMustBeArray: "custom_cidrs는 배열이어야 합니다.",
    syncSshPolicyFailed: "SSH 전용 방화벽 규칙을 동기화하지 못했습니다.",
    clearSshPolicyFailed: "SSH 전용 방화벽 규칙을 지우지 못했습니다.",
    blockRecordInvalid: "블록 레코드 형식이 잘못되었습니다.",
    routes: {
      loadConfigFailed: "SSH 보안 설정을 불러오지 못했습니다.",
      updateConfigFailed: "SSH 보안 구성을 업데이트하지 못했습니다.",
      syncFirewallSuccess:
        "동기화된 {allowedCidrs}은 CIDR을 허용하고 {synced} SSH는 {ports} 포트에 대한 IP를 차단했습니다.",
      syncFirewallFailed: "SSH 방화벽을 동기화하지 못했습니다.",
      clearFirewallSuccess: "SSH 전용 방화벽 규칙을 지웠습니다.",
      clearFirewallFailed: "SSH 방화벽을 지우지 못했습니다.",
      readLoginLogsFailed: "SSH 로그인 로그를 읽지 못했습니다.",
      listBlocksFailed: "SSH 차단 목록을 불러오지 못했습니다.",
      blockNotFound: "블록 기록을 찾을 수 없습니다",
      loadBlockFailed: "SSH 차단 기록을 불러오지 못했습니다.",
      removeBlockFailed: "블록을 제거하지 못했습니다.",
      selectIps: "차단을 해제할 IP를 선택하세요.",
      removeBlocksFailed: "블록을 제거하지 못했습니다.",
    },
  },
  systemEvents: {
    routes: {
      unsupportedSystemEventType: "지원하지 않는 시스템 이벤트 유형입니다",
      unsupportedSystemEventSource: "지원하지 않는 시스템 이벤트 소스입니다",
      unsupportedSystemEventLevel: "지원하지 않는 시스템 이벤트 레벨입니다",
      unsupportedSubjectKind: "지원하지 않는 이벤트 주체 유형입니다",
      unsupportedEventType: "지원하지 않는 이벤트 유형입니다",
      unsupportedEventLevel: "지원하지 않는 이벤트 레벨입니다",
      unsupportedEventSource: "지원하지 않는 이벤트 소스입니다",
      loadConfigFailed: "시스템 이벤트 설정을 불러오지 못했습니다",
      writeEventFailed: "시스템 이벤트를 기록하지 못했습니다",
      listEventsFailed: "시스템 이벤트 목록을 불러오지 못했습니다",
      deleteEventsFailed: "시스템 이벤트를 삭제하지 못했습니다",
      clearEventsFailed: "시스템 이벤트를 비우지 못했습니다",
    },
  },
  notifications: {
    brand: {
      prefix: "Knock ",
      defaultTitle: "Knock 알림",
    },
    templates: {
      events: {
        authLoginSuccess: "로그인 성공",
        authLogout: "로그아웃됨",
        authLoginFailure: "로그인 실패",
        authSessionIpDrift: "세션 IP 드리프트",
        securityScannerBlocked: "스캐너가 차단되었습니다.",
        ddnsUpdateCompleted: "DDNS가 업데이트되었습니다.",
        gatewayThrottleBlocked: "게이트웨이 조절이 차단됨",
        wafBlocked: "WAF가 차단됨",
        sshLoginSuccess: "SSH 로그인 성공",
        sshLoginFailure: "SSH 로그인 실패",
        sshIpBlocked: "SSH IP가 차단됨",
        appUpdateAvailable: "애플리케이션 업데이트 가능",
        cpuAlert: "CPU 경고",
        cpuRecovered: "CPU가 복구되었습니다.",
        memoryAlert: "메모리 경고",
        memoryRecovered: "메모리 복구됨",
        frpConnected: "FRP 연결됨",
        frpDisconnected: "FRP 연결이 끊어졌습니다.",
        cloudflaredConnected: "Cloudflared가 연결됨",
        cloudflaredDisconnected: "Cloudflared 연결이 끊어졌습니다.",
      },
      ruleName: "{event} 알림",
      levels: {
        info: "정보",
        warn: "경고",
        error: "오류",
        critical: "심각",
      },
      sources: {
        serverAdmin: "관리 백엔드",
        goReauthProxy: "인증 프록시",
        systemMonitor: "시스템 모니터",
      },
      authMethods: {
        oidc: "외부 계정",
      },
      grantTypes: {
        browserSession: "브라우저 세션",
        loginIpGrant: "로그인 IP 부여",
      },
      wafModes: {
        detection: "탐지",
        blocking: "차단",
        off: "끄기",
      },
      wafActions: {
        block: "블록",
        deny: "거부",
        detect: "감지",
        log: "로그",
        pass: "패스",
      },
      logoutSources: {
        userLogout: "사용자가 로그아웃했습니다.",
        adminSessionDelete: "관리자가 세션을 종료했습니다.",
      },
      driftSources: {
        proxySession: "프록시 세션",
        fnosToken: "FNOS 토큰",
        sessionRefresh: "세션 새로 고침",
        browserSession: "브라우저 세션",
      },
      ddnsTriggers: {
        cron: "예약된 작업",
        enable: "활성화 후 첫 번째 실행",
        startup: "시작 후 확인",
        manualTest: "수동 테스트",
      },
      ddnsUpdateScopes: {
        ipv4Only: "IPv4 전용",
        ipv6Only: "IPv6 전용",
      },
      ddnsIpSources: {
        public: "공개 탐지",
        interface: "인터페이스 읽기",
        static: "고정 IP",
        domain: "도메인 확인",
      },
      updateCheckReasons: {
        cron: "예정된 점검",
        manual: "수동 점검",
        manualCheckAndDownload: "수동 확인 및 다운로드",
        downloadBootstrap: "사전 다운로드 확인",
      },
      credential: "자격 증명",
      unknownCredential: "알 수 없는 자격증명",
      credentialLinkedTotp:
        '{authMethod} "{credential}"이 TOTP "{totp}"에 연결됨',
      credentialName: '자격 증명 "{credential}"',
      sessionCommentCompact: "참고: {comment}",
      appendSessionComment: "{text}(참고: {comment})",
      yes: "예",
      no: "아니요",
      wafOutcomeBlocked: "차단됨",
      wafOutcomeLogged: "기록됨",
      sections: {
        overview: "이벤트 개요",
        aggregation: "집계",
        advice: "권장 조치",
      },
      aggregationText:
        "이 알림은 {seconds}초 기간 내에 {count} 유사한 이벤트를 집계했습니다.",
      details: {
        units: {
          seconds: "{count} 초",
          minutes: "{count} 분",
          times: "{count} 회",
          ratePerSecond: "{count}/초",
        },
        listSeparator: ", ",
        unknown: "알 수 없음",
        unknownIp: "알 수 없는 IP",
        unknownMethod: "알 수 없는 방법",
        unknownProvider: "알 수 없는 제공업체",
        unknownUser: "알 수 없는 사용자",
        unknownHost: "알 수 없는 호스트",
        currentSession: "현재 세션",
        memoryMetric: "메모리",
        connected: "연결됨",
        disconnected: "연결이 끊김",
        parenthesized: " ({value})",
        sessionCommentSentence: '현재 세션 메모: "{comment}".',
        aggregationStatsValue: "{count} 이벤트 / {seconds}-두 번째 창",
        facts: {
          credentialName: "자격 증명 이름",
          linkedTotp: "연결된 TOTP",
          sessionComment: "세션 노트",
          loginIp: "로그인 IP",
          ipLocation: "IP 위치",
          authMethod: "인증 방법",
          loginProvider: "로그인 공급자",
          grantType: "보조금 유형",
          rememberLogin: "로그인 기억하기",
          sessionExpiresAt: "세션 만료 시간:",
          sessionId: "세션 ID",
          logoutSource: "로그아웃 소스",
          loginTime: "로그인 시간",
          sourceIp: "소스 IP",
          failureAttempts: "실패한 시도",
          retryWait: "재시도 대기",
          limitUntil: "까지 제한",
          originalIp: "원래 IP",
          originalLocation: "원래 위치",
          currentIp: "현재 IP",
          currentLocation: "현재 위치",
          driftSource: "드리프트 소스",
          hitCount: "적중 횟수",
          observationWindow: "관찰 창",
          triggerThreshold: "트리거 임계값",
          blockedAt: "차단된 시간",
          recentPaths: "최근 경로",
          target: "대상",
          provider: "공급자",
          targetType: "대상 유형",
          trigger: "트리거",
          updateScope: "업데이트 범위",
          ipSource: "IP 소스",
          ipv4Change: "IPv4 변경",
          ipv6Change: "IPv6 변경",
          result: "결과",
          blockDuration: "차단 기간",
          blockedUntil: "차단 기간:",
          rateLimit: "비율 제한",
          burstCapacity: "버스트 용량",
          targetHost: "대상 호스트",
          requestPath: "요청 경로",
          routeType: "경로 유형",
          authRoute: "인증 경로",
          traceId: "추적 ID",
          requestAddress: "요청 주소",
          outcome: "결과",
          wafAction: "WAF 작업",
          wafMode: "WAF 모드",
          ruleIds: "규칙 ID",
          ruleBundle: "규칙 번들",
          statusCode: "상태 코드",
          user: "사용자",
          port: "항구",
          logTime: "로그 시간",
          invalidUser: "잘못된 사용자",
          threshold: "임계값",
          window: "창",
          blockedReason: "차단 이유",
          relatedUser: "관련 사용자",
          currentVersion: "현재 버전",
          latestVersion: "최신 버전",
          checkReason: "이유 확인",
          forceUpdate: "강제 업데이트",
          releaseNotes: "릴리스 노트",
          hostname: "호스트 이름",
          currentUsage: "현재 사용량",
          alertThreshold: "경고 기준",
          recoverThreshold: "복구 임계값",
          sampleInterval: "샘플 간격",
          sustainDuration: "지속 시간",
          tunnelType: "터널 유형",
          connectionStatus: "연결 상태",
          processPid: "프로세스 PID",
          runtimeFeedback: "런타임 피드백",
          eventType: "이벤트 유형",
          riskLevel: "위험 수준",
          eventSource: "이벤트 소스",
          happenedAt: "발생 시간",
          aggregationStats: "집계",
        },
        authLoginSuccess: {
          loginViaProvider: "{provider}을 통해 로그인됨",
          loginWithMethod: "{method} 사용",
          authViaProvider: "{provider}을 통해",
          authWithMethod: "{method} 사용",
          summaryOidc:
            "{credential} {method}은 IP {ip}{totpPart}에서 성공했습니다.",
          linkedTotpPart: ', 연결된 TOTP "{totp}"',
          summaryTotp:
            '{method} {ip}에서 로그인된 TOTP "{totp}"이 연결된 "{credential}"',
          summaryCredential: '{ip}에서 로그인된 자격 증명 "{credential}"',
          overview:
            "이 로그인으로 인증이 완료되었습니다. {auth}; 부여 유형: {grantType}{locationPart}. {commentPart}",
          locationPart: ", 위치: {location}",
          advice:
            "이 로그인이 귀하의 것이 아닌 경우 즉시 세션을 취소하고 액세스 정책을 검토하십시오.",
        },
        authLogout: {
          summaryTotp:
            '연결된 TOTP "{totp}"이 있는 {method} "{credential}"이 로그아웃되었습니다.',
          summaryCredential: '자격 증명 "{credential}"이 로그아웃되었습니다.',
          overview:
            "이 세션은 {ip}{locationPart}에서 로그아웃되었습니다. 로그아웃 소스: {source}. {commentPart}",
          advice:
            "예상치 못한 로그아웃인 경우 관리자가 세션을 종료했는지, 아니면 비정상적인 정리가 발생했는지 확인하세요.",
        },
        authLoginFailure: {
          summary: "{ip}의 로그인 실패로 인해 {attempts} 시도에 도달했습니다.",
          overview:
            "반복적인 로그인 인증 실패가 감지되었습니다. 현재 소스 IP: {ip}{retryPart}{blockedPart}.",
          retryPart: "; {seconds} 초 후에 다시 시도",
          blockedPart: "; 제한은 {time}까지 지속됩니다.",
          advice:
            "본인이 아닌 경우 즉시 자격 증명 보안을 확인하고 원본 IP를 차단하거나 로그인 보호 수준을 높이는 것을 고려하세요.",
        },
        authSessionIpDrift: {
          summary: "{session} IP가 {fromIp}에서 {toIp}으로 변경되었습니다.",
          overview:
            "{session}의 액세스 소스 IP가 변경되었습니다. 소스 분류: {source}. {commentPart} 이는 일반적으로 네트워크 전환, 프록시 변경 또는 세션 이상과 관련이 있습니다.",
          advice:
            "이 IP 변경이 예상치 못한 것이라면 가능한 한 빨리 현재 세션에서 탈취 위험을 확인하십시오.",
        },
        securityScannerBlocked: {
          summary: "{ip}이 스캔 동작으로 인해 차단되었습니다.",
          overview:
            "이 소스는 {minutes} 분 이내에 {threshold}{pathsPart} 임계값을 초과하여 {hits} 스캔 적중을 트리거했습니다.",
          pathsPart: "; 최근 일치된 경로에는 {paths}이 포함됩니다.",
          advice:
            "게이트웨이 로그를 검토하여 이것이 악의적인 조사인지 확인하세요. 거짓 긍정인 경우 검색 임계값을 조정합니다.",
        },
        ddnsUpdateCompleted: {
          defaultTarget: "DDNS 대상",
          summarySuccess: "{target} DDNS 업데이트 성공",
          summaryFailure: "{target} DDNS 업데이트 실패",
          currentTask: "이 작업",
          overview:
            "{trigger}이 DDNS 업데이트를 실행했습니다. 범위: {scope}; IP 소스: {ipSource}. {resultPart}",
          resultPart: "결과: {message}",
          adviceSuccess:
            "DNS 확인이 적용되지 않은 경우 DNS 캐시가 새로 고쳐질 때까지 기다린 후 외부 액세스를 다시 확인하세요.",
          adviceFailure:
            "공급자 자격 증명, DNS 레코드 구성, 공용 IP 감지 상태를 확인하세요.",
          primaryDomain: "기본 도메인",
          additionalDomain: "추가 도메인",
        },
        gatewayThrottleBlocked: {
          summary: "빠른 요청으로 인해 {ip}이 {seconds}초 동안 차단되었습니다.",
          overview:
            "이 소스는 게이트웨이 제한을 트리거했습니다. 속도 제한: {rate}/s; 버스트 용량: {burst}{targetPart}.",
          targetPart: "; 대상 요청: {target}",
          advice:
            "액세스 로그를 검토하여 버스트 트래픽, 거짓 긍정 또는 악성 트래픽인지 확인한 다음 필요에 따라 제한을 조정합니다.",
        },
        wafBlocked: {
          summary: "{ip}의 요청은 WAF의 {outcome}이었습니다.",
          overview:
            "WAF {outcome} 소스 {ip}{hostPart}{pathPart}{actionPart}{modePart}. {rulesPart}",
          hostPart: " {host}에 액세스하는 중",
          pathPart: " {path}",
          actionPart: "; 액션: {action}",
          modePart: "; 현재 모드: {mode}",
          rulesPart: "일치하는 규칙: {rules}.",
          adviceBlocked:
            "WAF 로그의 추적 ID를 사용하여 적중 세부 정보를 검사합니다. 이것이 거짓 긍정인 경우 프로젝트 유지 관리 담당자에게 문제를 보고하십시오.",
          adviceLogged:
            "WAF 로그의 추적 ID를 사용하여 적중 세부 정보를 검사한 다음 규칙 및 요청 컨텍스트를 기반으로 정책 변경이 필요한지 여부를 결정합니다.",
        },
        sshLoginSuccess: {
          summary: '{ip}에서 로그인한 SSH 사용자 "{username}"',
          overview:
            "{ip}{locationPart}{authPart}에서 SSH 로그인이 성공했습니다.",
          authPart: "; 인증 방법: {authMethod}",
          advice:
            "예상치 못한 로그인인 경우 SSH 계정, 키, 소스 액세스 정책을 확인하세요.",
        },
        sshLoginFailure: {
          summary: 'SSH 사용자 "{username}"이 {ip}에서 로그인하지 못했습니다.',
          overview:
            "이 소스에서는 {minutes}분 기간{locationPart} 내에 {attempts}/{threshold} SSH 로그인 실패가 누적되었습니다.",
          locationPart: "; 위치: {location}",
          advice:
            "오류가 차단 임계값에 근접하는지 확인하세요. 필요한 경우 SSH 노출을 강화하거나 자격 증명을 조정하세요.",
        },
        sshIpBlocked: {
          reasonCidrNotAllowed: "허용된 지역 범위를 벗어났습니다.",
          reasonFailedThreshold: "실패한 시도가 임계값에 도달했습니다.",
          summary: "{ip}이 SSH 보안에 의해 차단되었습니다.",
          overview: "SSH 보안 차단 소스 {ip}{locationPart}; 이유: {reason}.",
          advice:
            "이 소스를 신뢰할 수 있는지 확인하세요. 실수로 차단된 경우 SSH 보안 차단 목록에서 차단을 해제하세요.",
        },
        appUpdateAvailable: {
          currentVersionUnknown: "현재 버전을 알 수 없음",
          targetVersionUnknown: "대상 버전을 알 수 없음",
          summary: "새 버전 {version} 사용 가능",
          currentCheck: "이 수표",
          overview:
            "{reason}은 fn-knock이 {localVersion}에서 {latestVersion}{forcePart}으로 업그레이드할 수 있음을 발견했습니다.",
          forcePart: "; 곧 업데이트 예약해 줘",
          releaseNotesAdvice: "릴리스 노트: {releaseNotes}",
          advice:
            "적절한 유지 관리 기간에 업데이트를 완료하고 설치 전에 현재 구성 및 서비스 상태를 확인하세요.",
        },
        systemMetric: {
          recoveredSummary:
            "{hostname} {metric} 사용량이 {usage}%로 복구되었습니다.",
          alertSummary: "{hostname} {metric} 사용량이 {usage}%로 증가했습니다.",
          recoveredOverview:
            "{hostname} {metric} 사용량이 {usage}%로 감소했습니다. 복구 라인: {recover}%; 이전 경고 기준: {threshold}%.",
          alertOverview:
            "{hostname} {metric} 사용량은 이제 {usage}%이며, 경고 임계값 {threshold}%를 초과합니다. 복구 라인은 {recover}%입니다.",
          recoveredAdvice:
            "자원이 더 안전한 범위로 돌아왔습니다. 반복되는 변동을 계속 관찰하세요.",
          alertAdvice:
            "지속적인 리소스 포화를 방지하려면 로드가 높은 프로세스, 백그라운드 작업 또는 외부 트래픽 변경 사항을 즉시 확인하세요.",
        },
        tunnel: {
          connectedSummary: "{tunnel}이 연결되었습니다.",
          disconnectedSummary: "{tunnel} 연결이 끊어졌습니다.",
          connectedOverview:
            "{tunnel} 터널 연결이 {messagePart}을 복구했습니다.",
          connectedMessagePart: "; 런타임 피드백: {message}",
          disconnectedOverview:
            "{tunnel} 터널 연결이 끊어졌습니다.{messagePart}.",
          disconnectedMessagePart: "; 현재 피드백: {message}",
          connectedAdvice:
            "액세스 문제를 해결하고 있었다면 지금 외부 항목을 다시 확인하세요.",
          disconnectedAdvice:
            "터널 구성, 업스트림 네트워크 상태, 원격 서비스 연결 가능성을 확인하세요.",
        },
        short: {
          loginFailureAttempts: "{count} 실패",
          scanHits: "{count} 스캔 적중",
          scanBlocked: "스캐너가 차단되었습니다.",
          success: "성공",
          failure: "실패",
          blockSeconds: "차단된 {seconds}",
          blockTriggered: "차단이 실행됨",
          rules: "규칙 {rules}",
          sshLoginSuccess: "SSH 로그인 성공",
          sshLoginFailure: "SSH 로그인 실패",
          regionNotAllowed: "허용되지 않는 지역",
          failureThreshold: "실패 임계값",
          currentVersion: "현재 {version}",
        },
        titles: {
          ddnsUpdateSuccess: "{target} 업데이트 성공",
          ddnsUpdateFailure: "{target} 업데이트 실패",
          credentialIpDrift: '자격 증명 "{credential}" IP 드리프트',
          appUpdateAvailable: "새 버전 {version} 사용 가능",
        },
      },
    },
    providers: {
      catalog: {
        email: {
          label: "이메일",
          description:
            "통합 사서함 연결 관리를 위해 선택적 IMAP 설정을 저장하여 SMTP를 통해 이메일 알림을 보냅니다.",
          fields: {
            smtp_host: {
              label: "SMTP 호스트",
              description: "smtp.example.com과 같은 메일 전송 서버 주소입니다.",
            },
            smtp_port: {
              label: "SMTP 포트",
              description: "공통 포트는 465(SSL/TLS) 및 587(STARTTLS)입니다.",
            },
            smtp_security: {
              label: "SMTP 암호화",
              options: {
                none: "암호화 없음",
              },
            },
            smtp_auth_mode: {
              label: "SMTP 인증 모드",
              description:
                "자동으로 AUTH PLAIN을 선호하고 필요한 경우 AUTH LOGIN으로 대체됩니다.",
              options: {
                auto: "자동 협상",
                none: "인증 없음",
              },
            },
            smtp_username: {
              label: "SMTP 사용자 이름",
            },
            smtp_password: {
              label: "SMTP 비밀번호",
            },
            from_address: {
              label: "발신지 주소",
              description: "MAIL FROM 주소와 From 헤더로 사용됩니다.",
            },
            from_name: {
              label: "보낸 사람 이름",
            },
            to_addresses: {
              label: "기본 수신자",
              description:
                "여러 이메일 주소를 쉼표나 새 줄로 구분하세요. 테스트 전송은 이러한 수신자를 사용하며 규칙은 대상에서 이를 재정의할 수 있습니다.",
              targetLabel: "수신자 재정의",
              targetDescription:
                "선택사항. 공급자 기본 수신자를 사용하려면 비워 두세요.",
              addressLabel: "수신자",
            },
            cc_addresses: {
              label: "기본 CC",
              targetLabel: "참조 무시",
              addressLabel: "CC",
            },
            bcc_addresses: {
              label: "기본 숨은 참조",
              targetLabel: "숨은 참조 무시",
              addressLabel: "숨은참조",
            },
            reply_to: {
              label: "기본 답장",
              targetLabel: "답장 재정의",
              addressLabel: "회신 대상",
            },
            allow_invalid_tls: {
              label: "유효하지 않은 인증서 허용",
              description:
                "자체 호스팅 메일 서버 또는 자체 서명 인증서 디버깅에만 권장됩니다. 생산 중에는 이를 중단하세요.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            imap_host: {
              label: "IMAP 호스트",
              description:
                "선택 사항이며 인바운드 사서함 구성을 위해 저장됩니다. 현재 알림 흐름은 SMTP만 사용하고 IMAP을 읽지 않습니다.",
            },
            imap_port: {
              label: "IMAP 포트",
            },
            imap_security: {
              label: "IMAP 암호화",
              options: {
                none: "암호화 없음",
              },
            },
            imap_username: {
              label: "IMAP 사용자 이름",
            },
            imap_password: {
              label: "IMAP 비밀번호",
            },
            imap_mailbox: {
              label: "IMAP 사서함",
            },
            subject_prefix: {
              label: "제목 접두어",
              description: "선택사항(예: [프로덕션]).",
              placeholder: "[생산]",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
            details: "세부정보:",
            actionLinks: "작업 링크:",
            severity: "심각도: {value}",
            eventId: "이벤트 ID: {value}",
            occurredAt: "발생 장소: {value}",
          },
          errors: {
            invalidEmailAddress:
              "{field}에 잘못된 이메일 주소가 포함되어 있습니다: {value}",
            smtpConnectionClosed: "SMTP 연결이 종료되었습니다",
            smtpReaderDisposed: "SMTP 리더가 폐기되었습니다.",
            invalidSmtpResponse: "SMTP 응답을 구문 분석할 수 없습니다: {line}",
            smtpConnectionTimeout: "SMTP 연결 시간이 초과되었습니다.",
            smtpTlsHandshakeTimeout:
              "SMTP TLS 핸드셰이크 시간이 초과되었습니다.",
            smtpCommandFailed: "{message}: {code} {response}",
            unknownResponse: "알 수 없는 응답",
            authPlainUnsupported: "SMTP 서버가 AUTH PLAIN을 지원하지 않습니다.",
            authLoginUnsupported: "SMTP 서버는 AUTH LOGIN을 지원하지 않습니다.",
            unsupportedAuthMechanisms:
              "지원되지 않는 SMTP 인증 메커니즘: {mechanisms}",
            authFailed: "SMTP 인증 실패",
            usernameAuthFailed: "SMTP 사용자 이름 인증에 실패했습니다.",
            passwordAuthFailed: "SMTP 비밀번호 인증 실패",
            dataStartFailed: "SMTP DATA 단계를 시작하지 못했습니다.",
            submitFailed: "SMTP 메시지를 제출하지 못했습니다.",
            invalidFromAddress: "보낸사람 주소 형식이 잘못되었습니다.",
            recipientRequired: "수신자 이메일 주소가 하나 이상 필요합니다.",
            handshakeFailed: "SMTP 서버 인사말 실패",
            ehloFailed: "SMTP EHLO 실패",
            startTlsUnsupported:
              "SMTP 서버가 STARTTLS 지원을 알리지 않았습니다.",
            startTlsFailed: "SMTP STARTTLS 실패",
            ehloAfterTlsFailed: "TLS 업그레이드 실패 후 SMTP EHLO",
            credentialsRequired:
              "SMTP 사용자 이름과 비밀번호는 비워둘 수 없습니다.",
            noAuthMechanism:
              "SMTP 서버가 사용 가능한 인증 메커니즘을 제공하지 않았습니다.",
            mailFromFailed: "SMTP 발신자를 설정하지 못했습니다.",
            recipientSetFailed:
              "SMTP 수신자 {recipient}을 설정하지 못했습니다.",
            quitFailed: "SMTP 종료 실패",
            missingSmtpHost: "누락된 SMTP 호스트",
            deliveryFailed: "이메일 전송 실패",
          },
        },
        pushplus: {
          label: "PushPlus",
          description:
            "WeChat 공식 계정, 앱, 이메일 등 규칙별 채널 선택을 통해 PushPlus 표준 API를 통해 알림을 보냅니다.",
          fields: {
            server_url: {
              label: "서비스 URL",
              description: "필요한 경우가 아니면 공식 API URL을 유지하세요.",
            },
            token: {
              label: "토큰",
              description:
                "PushPlus 사용자 토큰 또는 메시지 토큰. 비밀로 유지하세요.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            topic: {
              label: "주제 코드",
              description:
                "선택사항. 지정된 주제로 메시지를 보냅니다. 토큰 소유자에게 보내려면 비워 두세요.",
            },
            template: {
              label: "메시지 템플릿",
              description:
                "마크다운은 기본적으로 사용됩니다. 일반 텍스트나 HTML이 채널에 더 적합한 경우 대상별로 전환합니다.",
              options: {
                markdown: "Markdown",
                html: "HTML",
                txt: "일반 텍스트",
                json: "JSON",
              },
            },
            channel: {
              label: "채널 보내기",
              description:
                "기본값은 WeChat 공식 계정입니다. PushPlus에서 다른 채널을 구성한 경우 여기로 전환하세요.",
              options: {
                wechat: "위챗 공식 계정",
                webhook: "타사 웹훅",
                cp: "위컴 앱",
                mail: "이메일",
                sms: "SMS",
                voice: "음성",
                extension: "플러그인/데스크탑 앱",
                app: "앱",
                clawbot: "위챗 ClawBot",
              },
            },
            option: {
              label: "채널 옵션",
              description:
                "선택사항. cp, webhook, mail과 같은 채널은 일반적으로 PushPlus 계정 센터에서 구성된 채널 코드가 필요합니다.",
            },
            to: {
              label: "친구 토큰/사용자 ID",
              description:
                "선택사항. WeChat 공식 계정 채널의 경우 친구 토큰을 사용하거나 WeCom 앱의 사용자 ID를 사용하세요. 여러 수신자가 PushPlus 형식을 따를 수 있습니다.",
              placeholder: "friend_token 또는 user1,user2",
            },
            callback_url: {
              label: "콜백 URL",
              description:
                "선택사항. PushPlus는 비동기 전달이 완료된 후 이 URL을 호출합니다.",
            },
            pre: {
              label: "전처리 코드",
              description:
                "선택사항. PushPlus 계정에 해당 전처리 로직이 구성되어 있는 경우에만 이 항목을 입력하세요.",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingToken: "PushPlus 토큰 누락",
            requestFailed: "PushPlus 요청이 실패했습니다.",
          },
        },
        wxpusher: {
          label: "WxPusher",
          description:
            "WxPusher 표준 API를 통해 지정된 UID 또는 주제에 알림을 보냅니다. 빈 규칙 대상은 공급자 기본값을 상속합니다.",
          fields: {
            server_url: {
              label: "서비스 URL",
              description: "필요한 경우가 아니면 공식 서비스 URL을 유지하세요.",
            },
            app_token: {
              label: "AppToken",
              description: "WxPusher 백엔드 앱용 AppToken. 비밀로 유지하세요.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            uids: {
              label: "기본 UID 목록",
              targetLabel: "UID 목록",
              description:
                "선택사항. 테스트 전송은 이러한 UID를 선호하며 규칙 대상은 비어 있는 경우 이를 상속합니다.",
              targetDescription:
                "선택사항. 공급자 기본 UID 목록을 재정의합니다. 기본값을 상속하려면 비워 두세요.",
            },
            topic_ids: {
              label: "기본 주제",
              description:
                "선택사항. 테스트 전송에서는 이 주제를 선호합니다. 채널을 직접 확인하려면 하나 이상의 기본 UID 또는 주제를 구성하세요.",
              targetDescription:
                "선택사항. 공급자 기본 주제를 재정의합니다. 기본값을 상속하려면 비워 두세요.",
            },
            url: {
              label: "기본 메시지 URL",
              targetLabel: "메시지 URL",
              description:
                "선택사항. 규칙 대상은 비어 있는 경우 이 점프 URL을 상속하고 테스트 전송에서도 이를 사용합니다.",
              targetDescription:
                "선택사항. 공급자 기본 점프 URL을 재정의합니다. 기본값을 상속하려면 비워 두세요.",
            },
            verify_pay_type: {
              label: "기본 구독 확인",
              targetLabel: "구독 확인",
              description:
                "선택사항. 규칙 대상은 비어 있으면 이 구독 확인 정책을 상속합니다.",
              targetDescription:
                "선택사항. 공급자 기본 구독 확인 정책을 재정의합니다. 별도의 재정의를 방지하려면 상속을 선택하세요.",
              options: {
                "0": "확인하지 않음",
                "1": "유료 가입자만",
                "2": "구독 취소 또는 만료된 사용자만 해당",
                __inherit__: "공급자 기본값 상속",
              },
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingAppToken: "WxPusher AppToken 누락",
            invalidTopicIds: "잘못된 주제 ID 형식: {values}",
            recipientRequired:
              "WxPusher에는 하나 이상의 UID 또는 주제 ID가 필요합니다. 공급자 기본값에서 구성하거나 규칙 대상에서 재정의합니다.",
            targetsFailed: "{failed}/{total} WxPusher 대상 실패",
            requestFailed: "WxPusher 요청이 실패했습니다.",
          },
        },
        harmonyosmeow: {
          label: "HarmonyOSMeoW",
          description:
            "MeoW Push API를 통해 HarmonyOS 기기에 Markdown 알림을 보냅니다.",
          fields: {
            server_url: {
              label: "서비스 URL",
              description: "필요한 경우가 아니면 공식 API URL을 유지하세요.",
            },
            nickname: {
              label: "수신자 닉네임",
              description:
                "MeoW 앱에 설정된 사용자 닉네임입니다. 비공개 수신자 식별자로 취급하세요.",
            },
            timeout_seconds: {
              label: "시간 제한(초)",
            },
          },
          errors: {
            missingNickname: "MeoW 수신자 닉네임 누락",
            invalidNickname: "MeoW 수신자 닉네임에는 슬래시를 사용할 수 없습니다.",
            invalidServerUrl: "잘못된 MeoW 서비스 URL",
            requestFailed: "MeoW 요청 실패",
          },
        },
        bark: {
          label: "Bark",
          description:
            "공식 Bark 서비스 또는 자체 호스팅 Bark 서버를 통해 APN 푸시 알림을 iPhone으로 보냅니다.",
          fields: {
            server_url: {
              label: "서비스 URL",
              description:
                "자체 호스팅 Bark 서버를 사용하지 않는 한 공식 온라인 서비스 URL을 유지하세요.",
            },
            device_key: {
              label: "장치 Key",
              description:
                "Bark 앱에서 복사된 장치 키. 여러 키는 쉼표로 구분할 수 있습니다.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            level: {
              label: "알림 수준",
              description:
                "active는 기본 즉시 알림이고, timeSensitive는 Focus를 우회할 수 있으며, important는 중요한 알림입니다.",
              options: {
                active: "즉시 알림",
                timeSensitive: "시간 민감 알림",
                passive: "조용한 알림",
                critical: "중요 알림",
              },
            },
            group: {
              label: "메시지 그룹",
              description:
                "선택사항. 동일한 그룹 메시지는 Bark 클라이언트에 그룹화됩니다.",
            },
            sound: {
              label: "소리",
              description:
                "선택사항. Bark에서 지원하는 시스템 또는 사용자 정의 사운드 이름을 입력하세요.",
            },
            url: {
              label: "URL을 탭하세요",
              description:
                "선택사항. 알림을 탭한 후 이 링크를 엽니다. 비어 있으면 첫 번째 메시지 작업 링크가 사용됩니다.",
            },
            icon: {
              label: "아이콘 URL",
              description:
                "선택사항. iOS 15 이상에서는 맞춤 아이콘을 표시할 수 있습니다.",
            },
            badge: {
              label: "배지 번호",
              description: "선택사항. Bark 앱 아이콘 배지에 표시된 숫자입니다.",
            },
            call: {
              label: "반복되는 벨소리",
              description:
                "활성화되면 Bark의 벨소리가 약 30초 동안 계속 울립니다.",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingDeviceKey: "Bark 장치 키 누락",
            requestFailed: "짖기 요청이 실패했습니다.",
            pushFailed: "짖는 소리 푸시 실패",
            targetsFailed: "{failed}/{total} 짖는 대상이 실패했습니다.",
          },
        },
        serverchan: {
          label: "서버찬",
          description:
            "ServerChan Turbo를 통해 마크다운 알림을 보내고 웹사이트에 구성된 기본 수신 채널을 재사용합니다.",
          fields: {
            server_url: {
              label: "서비스 URL",
              description: "필요한 경우가 아니면 공식 API URL을 유지하세요.",
            },
            sendkey: {
              label: "SendKey",
              description:
                "ServerChan Turbo에서 제공하는 SendKey입니다. 비밀로 유지하세요.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            channel: {
              label: "메시지 채널",
              description:
                "선택사항. 이 푸시에 대해 9|66과 같이 |로 구분된 최대 2개의 채널을 동적으로 선택합니다.",
            },
            openid: {
              label: "OpenID / UID",
              description:
                "선택사항. 테스트 계정은 openid를 사용하고 WeCom 앱 메시지는 수신자 UID를 사용합니다. 값이 여러 개인 경우 ServerChan 설명서 형식을 따르세요.",
              placeholder: "openid1,openid2 또는 uid1|uid2",
            },
            short: {
              label: "카드 요약",
              description:
                "선택사항. 메시지 카드의 간략한 요약(최대 64자) ServerChan이 본문에서 파생되도록 비워 두세요.",
              placeholder: "로그인 이상 현상, 곧 처리하겠습니다",
            },
            noip: {
              label: "발신자 IP 숨기기",
              description:
                "활성화되면 이 푸시는 발신자 소스 IP를 표시하지 않습니다.",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingSendKey: "ServerChan SendKey 누락",
            requestReturned: "ServerChan이 HTTP {status}을 반환했습니다.",
            requestFailed: "ServerChan 요청이 실패했습니다.",
          },
        },
        dingtalk: {
          label: "딩톡봇",
          description:
            "선택적 서명 확인과 함께 DingTalk 봇 Webhook을 통해 그룹 채팅에 Markdown 알림을 보냅니다.",
          fields: {
            webhook_url: {
              label: "Webhook URL",
              description: "DingTalk 봇이 생성한 전체 웹훅 URL입니다.",
            },
            secret: {
              label: "서명 비밀",
              description:
                "선택사항. 봇이 서명을 활성화한 경우 보안 설정 페이지에 표시된 SEC 접두어가 붙은 비밀을 입력하세요.",
            },
            keyword_prefix: {
              label: "키워드 접두사",
              description:
                "선택사항. 봇이 사용자 정의 키워드 확인을 활성화한 경우 여기에서 고정 키워드를 설정하세요. 제목 앞에 자동으로 추가됩니다.",
              placeholder: "모니터링 경고",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            at_mobiles: {
              label: "@ 휴대폰 번호",
              description:
                "선택사항. 여러 값을 쉼표나 새 줄로 구분하세요. 값은 그룹의 회원 휴대폰 번호여야 합니다.",
            },
            at_user_ids: {
              label: "@ 사용자 ID",
              description:
                "선택사항. 여러 값을 쉼표나 새 줄로 구분하세요. @userId 토큰은 본문에 자동으로 추가됩니다.",
            },
            is_at_all: {
              label: "@ 여러분",
              description:
                "활성화되면 요청에 isAtAll이 포함되고 @everyone이 본문에 추가됩니다.",
            },
          },
          mentionAll: "@모두",
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingWebhookUrl: "DingTalk 웹훅 URL 누락",
            requestReturned: "DingTalk가 HTTP {status}을 반환했습니다.",
            requestFailed: "딩톡 요청 실패",
          },
        },
        feishu: {
          label: "페이슈 봇",
          description:
            "선택적 서명 확인과 함께 Feishu 봇 웹후크를 통해 그룹 채팅에 풍부한 게시물 알림을 보냅니다.",
          fields: {
            webhook_url: {
              label: "Webhook URL",
              description: "Feishu 봇이 생성한 전체 웹훅 URL입니다.",
            },
            secret: {
              label: "서명 비밀",
              description:
                "선택사항. 봇이 서명 확인을 활성화한 경우 보안 설정에서 복사한 비밀을 입력하세요.",
            },
            keyword_prefix: {
              label: "키워드 접두사",
              description:
                "선택사항. 봇이 사용자 정의 키워드 확인을 활성화한 경우 여기에서 고정 키워드를 설정하세요. 제목 앞에 자동으로 추가됩니다.",
              placeholder: "앱 알림",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            mention_user_ids: {
              label: "@ 사용자 ID",
              description:
                "선택사항. 여러 값을 쉼표나 새 줄로 구분하세요. 모두 지원합니다. 외부 그룹에서 단일 사용자를 언급하는 것은 Open ID만 지원합니다.",
            },
          },
          mentionAll: "모두",
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingWebhookUrl: "Feishu 웹훅 URL 누락",
            requestReturned: "Feishu가 HTTP {status}을 반환했습니다.",
            requestFailed: "Feishu 요청이 실패했습니다.",
          },
        },
        webhook: {
          label: "Webhook",
          description:
            "HTTP JSON을 지원하는 모든 엔드포인트에 표준 알림 JSON을 보냅니다.",
          fields: {
            url: {
              label: "Webhook URL",
              description: "표준 알림 JSON을 수신하는 대상 주소입니다.",
            },
            method: {
              label: "요청 방법",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            shared_secret: {
              label: "공유 비밀",
              description:
                "선택사항. 설정되면 X-Fn-Knock-Signature 요청 헤더를 통해 전송됩니다.",
            },
            endpoint_path: {
              label: "추가 경로",
              description:
                "선택사항. 전송하기 전에 기본 Webhook URL에 추가됩니다.",
            },
            extra_headers_json: {
              label: "추가 헤더 JSON",
              description: '선택 사항입니다(예: {"X-Env":"prod"}).',
            },
            extra_body_json: {
              label: "추가 본문 JSON",
              description: "선택사항. payload.extra_body에 첨부됩니다.",
            },
          },
          errors: {
            missingUrl: "웹훅 URL 누락",
            requestReturned: "웹훅이 HTTP {status}을 반환했습니다.",
            requestFailed: "웹훅 요청이 실패했습니다.",
          },
        },
        magicpush: {
          label: "매직푸시",
          description:
            "표준 푸시 및 MagicPush 인바운드 모드를 지원하는 자체 호스팅 MagicPush 서비스를 통해 구성된 채널에 알림을 푸시합니다.",
          fields: {
            server_url: {
              label: "기본 API URL",
              description:
                "MagicPush 서비스 루트를 입력하세요. 예를 들어 이미 /api/push 또는 /api/inbound를 포함하는 http://192.168.31.98:3000. URL도 허용됩니다.",
            },
            delivery_mode: {
              label: "배달 모드",
              description:
                "표준 푸시는 /api/push로 전송됩니다. 인바운드 모드는 /api/inbound/:token으로 전송하고 MagicPush 인바운드 규칙이 필드를 매핑하도록 합니다.",
              options: {
                push: "표준 푸시",
                inbound: "인바운드 구성",
              },
            },
            token: {
              label: "토큰",
              description:
                "MagicPush API 토큰. 표준 푸시는 이를 Authorization: Bearer로 보냅니다. 인바운드 모드에서는 이를 /api/inbound/:token에 추가합니다.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingBaseUrl: "MagicPush 기본 API URL 누락",
            missingToken: "MagicPush 토큰 누락",
            invalidBaseUrl: "잘못된 MagicPush 기본 API URL",
            requestReturned: "MagicPush가 HTTP {status}을 반환했습니다.",
            requestFailed: "MagicPush 요청이 실패했습니다.",
          },
        },
        telegram: {
          label: "Telegram",
          description:
            "인라인 작업 버튼을 사용하여 Telegram Bot API를 통해 지정된 채팅이나 채널에 문자 알림을 보냅니다.",
          fields: {
            server_url: {
              label: "봇 API URL",
              description:
                "기본적으로 공식 Bot API를 유지합니다. 공식 엔드포인트에 대한 네트워크 액세스가 불가능한 경우 https://tgapi.fnknock.cn을 릴레이로 사용하세요. 자체 호스팅 Local Bot API 서버를 실행하는 경우 해당 루트 URL을 입력합니다.",
            },
            bot_token: {
              label: "봇 토큰",
              description:
                "@BotFather를 통해 봇을 생성한 후 획득한 봇 토큰입니다.",
            },
            chat_id: {
              label: "채팅 ID",
              description:
                "@channelusername과 같은 대상 채팅 ID 또는 채널 사용자 이름입니다. 먼저 @UserIdzhBot에 메시지를 보내 채팅 ID를 받을 수 있습니다. 테스트 전송도 이 대상을 사용합니다.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            message_thread_id: {
              label: "주제 ID",
              description:
                "선택사항. 그룹 주제로 보내기 위한 주제 ID(message_thread_id)입니다.",
            },
            disable_notification: {
              label: "자동 보내기",
              description:
                "활성화되면 텔레그램은 알림음 없이 자동으로 전달됩니다.",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingBotToken: "텔레그램 봇 토큰 누락",
            missingChatId: "텔레그램 채팅 ID 누락",
            requestReturned: "텔레그램에서 HTTP {status}을 반환했습니다.",
            requestFailed: "텔레그램 요청 실패",
          },
        },
        wecom: {
          label: "WeCom 그룹 봇",
          description:
            "WeCom 그룹 Webhook을 통해 지정된 그룹 채팅에 텍스트 또는 마크다운 알림을 보냅니다.",
          fields: {
            webhook_url: {
              label: "Webhook URL",
              description:
                "WeCom 메시지 푸시 페이지에 생성된 전체 웹훅 URL입니다. 비밀로 유지하세요.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
            mentioned_list: {
              label: "회원 UserID 언급",
              description:
                "선택사항. 여러 값을 쉼표나 새 줄로 구분하세요. @all을 지원합니다.",
            },
            mentioned_mobile_list: {
              label: "휴대폰 번호 언급",
              description:
                "선택사항. 여러 값을 쉼표나 새 줄로 구분하세요. @all을 지원합니다.",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingWebhookUrl: "WeCom 웹훅 URL 누락",
            requestReturned: "WeCom이 HTTP {status}을 반환했습니다.",
            requestFailed: "WeCom 요청이 실패했습니다.",
          },
        },
        pushdeer: {
          label: "PushDeer",
          description:
            "PushDeer 공식 온라인 서비스 또는 자체 호스팅 서비스를 통해 바인딩된 장치에 Markdown 알림을 보냅니다.",
          fields: {
            server_url: {
              label: "서비스 URL",
              description:
                "자체 호스팅 PushDeer 서비스를 사용하지 않는 한 공식 온라인 서비스 URL을 유지하세요.",
            },
            pushkey: {
              label: "PushKey",
              description:
                "PushDeer 클라이언트에서 생성된 PushKey입니다. 여러 키는 쉼표로 구분할 수 있습니다.",
            },
            timeout_seconds: {
              label: "시간 초과(초)",
            },
          },
          message: {
            fallbackTitle: "fn-knock 알림",
          },
          errors: {
            missingPushKey: "PushDeer 푸시키 누락",
            requestReturned: "PushDeer가 HTTP {status}을 반환했습니다.",
            apiReturnedCode: "PushDeer API가 {code} 코드를 반환했습니다.",
            requestFailed: "PushDeer 요청이 실패했습니다.",
          },
        },
      },
    },
    routes: {
      createProviderFailed: "알림 제공자를 생성하지 못했습니다.",
      testProviderFailed: "알림 제공자를 테스트하지 못했습니다.",
      getProviderFailed: "알림 공급자를 가져오지 못했습니다.",
      updateProviderFailed: "알림 제공자를 업데이트하지 못했습니다.",
      deleteProviderFailed: "알림 제공자를 삭제하지 못했습니다.",
      createRuleFailed: "알림 규칙을 생성하지 못했습니다.",
      updateRuleFailed: "알림 규칙을 업데이트하지 못했습니다.",
      deleteRuleFailed: "알림 규칙을 삭제하지 못했습니다.",
      unsupportedDeliveryStatus: "지원되지 않는 배송 상태",
      clearDeliveriesFailed: "배송 기록을 삭제하지 못했습니다.",
    },
    service: {
      unnamed: "이름 없음",
      invalidJsonBody: "요청 본문은 유효한 JSON이어야 합니다.",
      invalidJson: "{field}은 유효한 JSON이어야 합니다.",
      invalidSelectValue: "{field}에 잘못된 값이 있습니다.",
      fieldRequired: "{field}은 비워둘 수 없습니다.",
      testMessage: {
        title: "테스트 알림",
        summary:
          "알림 채널이 올바르게 구성되었으며 테스트 메시지가 성공적으로 트리거되었습니다.",
        bodyText:
          "이는 공급자 연결, 구조적 복사 및 디스플레이 동작을 확인하기 위해 fn-knock에서 보낸 테스트 알림입니다.",
        bodyMarkdown:
          "**연결 확인을 통과했습니다.**\n\n이는 공급자 연결, 구조적 복사 및 디스플레이 동작을 확인하기 위해 fn-knock에서 보낸 테스트 알림입니다.",
        sendType: "전송 유형",
        providerTest: "공급자 테스트",
        sentAt: "보낸 시간",
      },
      providerNotFound: "알림 제공자가 존재하지 않습니다.",
      unsupportedProviderType: "지원되지 않는 알림 공급자 유형",
      providerDefinitionMissing: "알림 공급자 정의가 존재하지 않습니다.",
      providerReferencedByRule:
        '이 공급자는 여전히 "{rule}" 규칙에 의해 참조됩니다.',
      testSendFailed: "테스트 전송 실패",
      testSendSuccess: "테스트 전송 성공",
      providerRequestReturnedStatus:
        "{provider} 요청이 상태 {status}를 반환했습니다.",
      barkPartialFailed: "Bark 대상 {failed}/{total}개 전송 실패",
      providerTypeMismatch: "공급자 유형이 기존 구성과 일치하지 않습니다.",
      providerTestName: "{provider} 테스트",
      invalidProviderRecord: "알림 공급자 레코드가 유효하지 않습니다.",
      ruleProviderMissing: "규칙이 존재하지 않는 알림 공급자를 참조합니다.",
      invalidTemplateOverrideMode: "잘못된 대상 템플릿 재정의 모드",
      unsupportedEventType: "지원되지 않는 시스템 이벤트 유형",
      invalidGroupBy: "잘못된 집계 차원",
      invalidMessageTemplateMode: "잘못된 메시지 템플릿 모드",
      invalidEventLevelFilter: "잘못된 이벤트 수준 필터",
      invalidEventSourceFilter: "잘못된 이벤트 소스 필터",
      targetRequired: "알림 대상이 하나 이상 필요합니다.",
      duplicateEventRule:
        "이 이벤트에 대한 알림 규칙이 이미 존재합니다. 먼저 원래 규칙을 삭제하세요.",
      ruleNotFound: "알림 규칙이 존재하지 않습니다.",
      invalidRuleRecord: "알림 규칙 레코드가 유효하지 않습니다.",
      deletedProvider: "삭제된 제공업체",
      storageUnavailable: "알림 저장소를 일시적으로 사용할 수 없습니다.",
    },
  },
};
