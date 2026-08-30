export const jaJPServer = {
  success: "成功",
  notFound: "見つかりません",
  apiPathNotFound: "API パスが見つかりません",
  invalidLocale: "サポートされていないロケールです",
  dockerAdminDenied:
    "管理パネルには、プライベートネットワークまたは信頼済みプロキシからのみアクセスできます",
  dockerAdminDeniedTitle: "アクセスが拒否されました",
  dockerAdminDeniedDescription:
    "管理パネルは、デフォルトでは現在のデバイス、LAN、VPN、または設定済みの信頼できるリバースプロキシからのみアクセスできます。インターネットからの直接アクセスは拒否されます。",
  dockerAdminCurrentIp: "検出されたアクセス元 IP: {ip}",
  dockerAdminProxyRequired:
    "ポート {port} の管理用エントリーポイントから管理 API へアクセスしてください",
  dockerAdminLoginRequired: "先に管理パネルへログインしてください",
  captchaUnavailable: "CAPTCHA サービスは一時的に利用できません",
  tooManyAttempts:
    "試行回数が多すぎます。しばらくしてから、もう一度お試しください。",
  tooManyAttemptsWithRetry:
    "試行回数が多すぎます。{seconds}秒後にもう一度お試しください。",
  loginCredentialMissing: "サーバーにはログイン認証情報が設定されていません",
  invalidOtpWithRetry:
    "認証コードが正しくありません。{seconds}秒後にもう一度お試しください。",
  invalidPasswordWithRetry:
    "ユーザー名またはパスワードが正しくありません。{seconds}秒後にもう一度お試しください。",
  runtimeProfile: {
    capabilities: {
      default: "現在の実行環境ではこの機能を利用できません",
      direct_mode_available: {
        docker:
          "Docker 環境ではホストの直接接続用ファイアウォールを管理できません",
        platform:
          "現在の実行環境ではホストの直接接続用ファイアウォールを管理できません",
        permission:
          "現在のプロセスにはホストの直接接続用ファイアウォールを管理する権限がありません",
      },
      host_firewall_available: {
        docker: "Docker 環境ではホストのファイアウォールを管理できません",
        platform: "現在の実行環境ではホストのファイアウォールを管理できません",
        permission:
          "現在のプロセスにはホストのファイアウォールを管理する権限がありません",
      },
      smart_connect_available: {
        docker:
          "Docker 環境ではスマート接続を利用できません。ホストの dnsmasq とポート53が必要です",
        platform: "現在の実行環境ではスマート接続を利用できません",
        permission:
          "現在のプロセスにはスマート接続に必要なホスト管理権限がありません",
      },
      fnos_certificate_sync_available: {
        docker: "Docker 環境では FNOS SSL 証明書同期を利用できません",
        platform: "FNOS SSL 証明書同期は FPK 環境でのみ利用できます",
        permission:
          "現在のプロセスには FNOS SSL 証明書同期に必要な root 権限がありません",
      },
      system_clock_sync_available: {
        docker: "Docker 環境ではホストのシステム時刻を同期できません",
        platform: "現在の実行環境ではシステム時刻を同期できません",
        permission:
          "現在のプロセスにはシステム時刻の同期に必要なホスト権限がありません",
      },
      self_update_available: {
        lite: "Knock Lite はアプリ内更新に対応していません。公式サイトから完全版をダウンロードしてください",
        docker:
          "Docker 環境ではアプリ内 FPK アップデートを利用できません。新しいイメージを取得してアップグレードしてください",
        openwrt:
          "OpenWrt 環境ではアプリ内 FPK アップデートを利用できません。デバイスのアーキテクチャに合う IPK を opkg でインストールしてください",
        deployment:
          "現在のデプロイ形式ではアプリ内アップデートを利用できません",
      },
      auto_https_available: {
        lite: "Knock Lite は Root 権限が必要なポート 80 を使用できません",
        platform: "現在の実行環境では自動 HTTPS を利用できません",
        permission:
          "現在のプロセスにはポート 80 の待ち受けに必要な権限がありません",
      },
      fnos_network_tuning_available: {
        lite: "Knock Lite は Root 権限が必要な FNOS ネットワーク最適化を提供しません",
        platform: "現在の実行環境では FNOS ネットワーク最適化を利用できません",
        permission:
          "現在のプロセスにはシステムのネットワーク設定を変更する Root 権限がありません",
      },
      shared_root_available: {
        missing:
          "現在の実行環境には利用可能な共有ディレクトリのマウントがありません",
      },
    },
  },
  systemClock: {
    unknown: "不明",
    actionSeparator: "、",
    listSeparator: "、",
    duration: {
      seconds: "{seconds} 秒",
      minutes: "{minutes} 分",
      minutesSeconds: "{minutes} 分 {seconds} 秒",
    },
    networkCheckFailed: "ネットワーク経由でシステム時刻を確認できませんでした",
    issues: {
      timezone: {
        title: "システムのタイムゾーンが中国標準時ではありません",
        message:
          "現在のシステムタイムゾーンは {timezone} です。{expected} に設定してください。",
      },
      timeMismatch: {
        title: "システム時刻がネットワーク上の時刻と一致しません",
        message:
          "システム時刻はネットワーク上の時刻と約 {drift} ずれています。",
      },
    },
    statusRefreshed: "システム時刻の状態を更新しました",
    syncFailed: "システム時刻の同期に失敗しました",
    networkTimeUnavailable: "ネットワークから標準時刻を取得できませんでした",
    sourceFetchFailed: "{source} から時刻を取得できませんでした",
    missingDateHeader: "{source} から有効な Date ヘッダーが返されませんでした",
    invalidDateHeader: "{source} から解析できない時刻が返されました",
    commandFailed: "{command} の実行に失敗しました",
    timezoneSet: "システムタイムゾーンを {timezone} に設定しました",
    missingZoneinfoFile: "システムにタイムゾーンファイルがありません: {path}",
    timezoneWritten: "システムタイムゾーン {timezone} を書き込みました",
    clockAdjusted: "システム時刻を修正しました",
    ntpEnabled: "NTP による自動時刻同期を有効にしました",
    serviceRestarted: "{service} を再起動しました",
  },
  updateRoutes: {
    downloadStarted: "アップデートパッケージのダウンロードを開始しました",
    downloadStartFailed: "ダウンロードの開始に失敗しました",
    installStarted: "アップデートのインストールを開始しました",
    installStartFailed: "インストールの開始に失敗しました",
    checkAndDownloadStarted:
      "アップデートの確認を開始し、ダウンロードを待機キューへ追加しました",
    startFailed: "開始に失敗しました",
    loadStatusFailed: "アップデート状況の読み込みに失敗しました",
    loadConfirmationFailed: "アップデート確認情報の読み込みに失敗しました",
  },
  gatewayHostResponse: {
    runTypes: {
      direct: "直接接続モード",
      reverseProxy: "リバースプロキシモード",
      subdomain: "サブドメインモード",
    },
    unavailableReason:
      "サブドメインモードでのみ利用できます。現在のモード: {mode}",
    editSubdomainOnly:
      "Host ヘッダー設定はサブドメインマッピングモードでのみ編集できます",
    syncFailed: "ゲートウェイの Host ヘッダー設定を同期できませんでした",
    hostRoutesSyncFailed: "Host ルートの同期に失敗しました",
    updateFailed: "ゲートウェイの Host ヘッダー設定を更新できませんでした",
    updateFailedRolledBack:
      "ゲートウェイの Host ヘッダー設定を更新できなかったため、設定を元に戻しました",
    updateFailedRollbackFailed:
      "{error}、設定の復元にも失敗しました: {rollbackError}",
    restoreConfigFailed: "Host ヘッダー設定を復元できませんでした",
    restoreRuntimeFailed: "Host ヘッダーの実行状態を復元できませんでした",
    restoreGatewayRuntimeFailed:
      "ゲートウェイの Host ヘッダーの実行状態を復元できませんでした",
  },
  admin: {
    runTypes: {
      direct: "ダイレクト接続モード",
      reverseProxy: "リバースプロキシモード",
      subdomain: "サブドメインモード",
    },
    validation: {
      required: "{label} は必須です",
      httpUrlRequired: "{label} は http:// または https:// で始めてください",
      proxyTargetUrlRequired:
        "{label} は http://、https://、ws://、wss:// のいずれかで始め、ホスト名を含めてください",
      invalidFormat: "{label} の形式が正しくありません",
    },
    rollback: {
      failed: "{message}。ロールバックにも失敗しました: {rollbackError}",
      restoreConfigFailed: "以前の設定を復元できませんでした",
      restoreSmartConnectFailed:
        "以前のスマート接続の実行状態を復元できませんでした",
      restoreRuntimeFailed: "以前の実行状態の復元に失敗しました",
      restoreProtocolConfigFailed:
        "プロトコルマッピング設定の復元に失敗しました",
      restoreProtocolFeatureFailed:
        "プロトコルマッピング機能スイッチの復元に失敗しました",
      restoreProtocolRuntimeFailed:
        "プロトコルマッピングの実行状態を復元できませんでした",
      restoreVisibilityConfigFailed: "公開範囲を元の設定に復元できませんでした",
      restoreVisibilityRuntimeFailed:
        "実行中の公開範囲 CIDR を復元できませんでした",
      restoreGatewayVisibilityFailed:
        "ゲートウェイの公開範囲を実行状態に復元できませんでした",
      restoreProxyHeadersConfigFailed:
        "以前のプロキシヘッダー設定を復元できませんでした",
      restoreProxyHeadersRuntimeFailed:
        "プロキシヘッダーの実行状態を復元できませんでした",
      restoreGatewayProxyHeadersRuntimeFailed:
        "ゲートウェイのプロキシヘッダー実行状態を復元できませんでした",
      restorePortalFailed: "ポータル表示を実行状態に復元できませんでした",
    },
    dockerPanel: {
      passwordNotNeeded: "現在のデプロイでは管理パネルのパスワードは不要です",
      setPasswordFailed: "管理パネルのパスワードの設定に失敗しました",
      passwordChangeUnsupported:
        "現在のデプロイでは、管理パネルのパスワードを変更できません",
      changePasswordFailed: "管理パネルのパスワードの変更に失敗しました",
      tooManyAttemptsWithRetry:
        "試行回数が多すぎます。{seconds} 秒後にもう一度お試しください",
      tooManyAttempts: "試行回数が多すぎます。後でもう一度お試しください。",
      passwordSetupRequired:
        "管理パネルのパスワードが設定されていません。最初の設定を完了してください。",
      passwordIncorrectWithRetry:
        "管理パネルのパスワードが正しくありません。{seconds} 秒後にもう一度お試しください。",
    },
    adminPanelRoutes: {
      signInRequired: "先に管理パネルへログインしてください",
      verifySessionFailed: "管理パネルセッションの検証に失敗しました",
      loadStateFailed: "管理パネル状態の読み込みに失敗しました",
      loadConfigFailed: "設定の読み込みに失敗しました",
      loadLocaleFailed: "言語設定の読み込みに失敗しました",
      loadAppearanceFailed: "外観設定の読み込みに失敗しました",
      saveLocaleFailed: "言語設定の保存に失敗しました",
      saveAppearanceFailed: "外観設定の保存に失敗しました",
      loadPasswordFailed: "管理パネルパスワードの読み込みに失敗しました",
      createSessionFailed: "管理パネルセッションの作成に失敗しました",
      verifyPasswordFailed: "管理パネルパスワードの検証に失敗しました",
      checkLoginRateLimitFailed: "ログイン頻度制限の確認に失敗しました",
    },
    runType: {
      switchFailed: "動作モードの切り替えに失敗しました",
      switchFailedRolledBack:
        "動作モードの切り替えに失敗しました。設定はロールバックされました",
      smartConnectDisabled:
        "動作モードは切り替わりましたが、Smart Connect の同期に失敗したため自動的に無効化しました。ローカル IP と dnsmasq の設定を確認してから、再度有効にしてください。",
    },
    firewall: {
      whitelistSynced: "、ホワイトリストの IP {count} 件を同期",
      exemptPorts: "、エントリーポイントのポート {ports} を許可",
      resetSuccess:
        "{runType}用にファイアウォールをリセットしました{whitelistMessage}{exemptPortsMessage}",
      resetFailed: "ファイアウォールのリセットに失敗しました",
      clearSuccess:
        "ファイアウォールルールを消去し、ポート {port} に関連する以前のリダイレクトを削除しました",
      clearFailed: "ファイアウォールのクリアに失敗しました",
    },
    firewallAdditionalPorts: {
      loadFailed: "追加許可ポート設定の読み込みに失敗しました",
      saveFailed: "追加許可ポート設定の保存に失敗しました",
      updateFailedRolledBack:
        "追加許可ポートを適用できなかったため、以前の設定とファイアウォールを復元しました：{message}",
      updateFailedRollback:
        "追加許可ポートを適用できませんでした：{message}；ロールバック失敗：{rollbackError}",
      errors: {
        portsArrayRequired: "ports はポート番号の配列である必要があります",
        portIntegerRequired: "追加許可ポートは整数である必要があります",
        portOutOfRange:
          "追加許可ポートは 1 から 65535 の範囲で指定してください",
        tooManyPorts: "追加許可ポートは 128 件を超えて設定できません",
      },
    },
    protocolMapping: {
      subdomainOnly:
        "プロトコルマッピングはサブドメインモードでのみ有効にできます",
      availabilityInvalid:
        "プロトコルマッピングのスケジュールが無効です。HH:mm 形式を使用し、有効化時刻と無効化時刻を別にしてください",
      updateFeatureFailed:
        "プロトコルマッピング機能スイッチの更新に失敗しました",
      updateFeatureFailedRolledBack:
        "プロトコルマッピング機能スイッチの更新に失敗し、設定がロールバックされました",
    },
    smartConnect: {
      subdomainOnly: "Smart Connect はサブドメインモードでのみ有効にできます",
      updateFailed: "スマート接続の更新に失敗しました",
      updateFailedRolledBack:
        "スマート接続の更新に失敗し、設定がロールバックされました",
    },
    fnosPortIcon: {
      syncFailed:
        "FNOS ポートアイコンの置換設定をゲートウェイへ同期できませんでした",
    },
    fnosNetworkTuning: {
      unavailable:
        "現在のランタイムは FNOS FPK のネットワーク最適化に対応していません",
      updateFailed: "FNOS FPK ネットワーク最適化の更新に失敗しました",
      errors: {
        bbrNotSupported: "ホストカーネルが tcp_bbr を提供していません",
        bbrEnableVerificationFailed:
          "BBR の有効化を要求しましたが、現在のカーネル状態が bbr/fq ではありません",
        bbrRollbackCongestionFailed:
          "BBR のロールバックで以前の輻輳制御を復元できませんでした",
        bbrRollbackQdiscFailed:
          "BBR のロールバックで以前の既定 qdisc を復元できませんでした",
        bbrRollbackStillBbrFailed:
          "BBR のロールバック後も輻輳制御が bbr のままです",
        mtuEnableVerificationFailed:
          "MTU probing の有効化を要求しましたが、tcp_mtu_probing が 1 ではありません",
        mtuRollbackFailed:
          "MTU probing のロールバックで期待値を復元できませんでした",
        emptyPatch:
          "FNOS FPK ネットワーク最適化オプションを少なくとも 1 つ変更してください",
        setSysctlFailed: "{key} の設定に失敗しました",
        rollbackFailed: "{message}; ロールバック失敗: {error}",
      },
      blocked: {
        lite: "Knock Lite は Root 権限が必要な FNOS ネットワーク最適化を提供しません",
        deployment:
          "FNOS FPK ネットワーク最適化は FPK デプロイでのみ利用できます",
        platform: "FNOS FPK ネットワーク最適化には Linux ホストが必要です",
        permission: "FNOS FPK ネットワーク最適化には root 権限が必要です",
      },
    },
    gateway: {
      syncAuthCacheFailed:
        "認証キャッシュ設定をゲートウェイに同期できませんでした",
      syncThrottleFailed:
        "ゲートウェイ スロットリング設定をゲートウェイに同期できませんでした",
      syncCrawlerBlockerFailed:
        "クローラーブロック設定をゲートウェイに同期できませんでした",
      updateFailed: "ゲートウェイ設定の更新に失敗しました",
      updateFailedRolledBack:
        "ゲートウェイ設定の更新に失敗しました。設定はロールバックされました。",
    },
    proxyMappings: {
      payloadObjectRequired:
        "パスプロキシマッピングはオブジェクトである必要があります",
      targetInvalid:
        "パスプロキシの転送先は http://、https://、ws://、wss:// のいずれかで始まり、ホストを含む必要があります",
      syncRulesFailed: "パスプロキシルートの同期に失敗しました",
      restoreRulesFailed: "パスプロキシルートの復元に失敗しました",
      updateFailed: "パスプロキシマッピングの更新に失敗しました",
      updateFailedRolledBack:
        "パスプロキシマッピングの更新に失敗しました。設定はロールバックされました",
    },
    gatewayVisibility: {
      updateFailed: "ゲートウェイの公開範囲を更新できませんでした",
      updateFailedRolledBack:
        "ゲートウェイの公開範囲を更新できませんでした。設定はロールバックされました。",
    },
    gatewayProxyHeaders: {
      subdomainOnly:
        "プロキシヘッダーはサブドメインマッピングモードでのみ編集できます",
      updateFailed: "ゲートウェイのプロキシヘッダー更新に失敗しました",
      updateFailedRolledBack:
        "ゲートウェイのプロキシヘッダー更新に失敗したため、設定をロールバックしました",
    },
    gatewaySettingsRoutes: {
      loadGatewaySettingsFailed: "ゲートウェイ設定の読み込みに失敗しました",
      payloadObjectRequired:
        "ゲートウェイリクエスト本文はオブジェクトである必要があります",
      loadConfigFailed: "設定の読み込みに失敗しました",
      saveGatewaySettingsFailed: "ゲートウェイ設定の保存に失敗しました",
      syncGatewaySettingsFailed:
        "ゲートウェイ設定の同期に失敗しました: {message}",
      responseReloadFailed:
        "ゲートウェイ設定は保存されましたが、応答の再読み込みに失敗しました",
      loadGatewayVisibilityFailed:
        "ゲートウェイの公開範囲を読み込めませんでした",
      loadRuntimeFailed: "実行状態の読み込みに失敗しました",
      loadGatewayProxyHeadersFailed:
        "ゲートウェイのプロキシヘッダー読み込みに失敗しました",
      loadGatewayHostResponseFailed:
        "ゲートウェイの Host ヘッダー設定の読み込みに失敗しました",
      loadGatewayProxyProtocolFailed:
        "ゲートウェイの PROXY Protocol 設定を読み込めませんでした",
    },
    runtimeConfigRoutes: {
      loadCaptchaFailed: "CAPTCHA 設定の読み込みに失敗しました",
      saveCaptchaFailed: "CAPTCHA 設定の保存に失敗しました",
      loadWolFeatureFailed: "Wake-on-LAN 機能設定の読み込みに失敗しました",
      saveWolFeatureFailed: "Wake-on-LAN 機能設定の保存に失敗しました",
      syncWolFeatureFailed:
        "Wake-on-LAN 機能をゲートウェイへ同期できませんでした",
      invalidWolFeature: "Wake-on-LAN 機能設定が無効です",
      invalidRunType: "run_type が不正です",
      loadProtocolMappingFeatureFailed:
        "プロトコルマッピング機能設定の読み込みに失敗しました",
      loadSmartConnectDetailsFailed:
        "Smart Connect 詳細の読み込みに失敗しました",
      loadFnosShareBypassFailed:
        "FNOS 共有バイパス設定の読み込みに失敗しました",
      saveFnosShareBypassFailed: "FNOS 共有バイパス設定の保存に失敗しました",
      loadFnosPortIconHijackFailed:
        "FNOS ポートアイコン引き継ぎ設定の読み込みに失敗しました",
      loadAutoHttpsFailed: "自動 HTTPS 設定の読み込みに失敗しました",
      saveAutoHttpsFailed: "自動 HTTPS 設定の保存に失敗しました",
      saveAutoManageFirewallFailed:
        "ファイアウォール自動管理設定の保存に失敗しました",
      loadConfigFailed: "設定の読み込みに失敗しました",
      loadDefaultRouteFailed: "既定ルートの読み込みに失敗しました",
      saveDefaultRouteFailed: "既定ルートの保存に失敗しました",
      unsupportedTunnelType: "サポートされていないトンネルタイプです",
      saveDefaultTunnelFailed: "既定トンネルの保存に失敗しました",
      upstreamUnavailable: "アップストリームサービスを利用できません",
      proxyProtocolForceBooleanRequired:
        "proxy_protocol_force は boolean である必要があります",
      loadRunModePromptPreferencesFailed:
        "動作モード案内設定の読み込みに失敗しました",
      saveRunModePromptPreferencesFailed:
        "動作モード案内設定の保存に失敗しました",
    },
    captcha: {
      turnstileKeysRequired:
        "Cloudflare Turnstile を有効にするには site_key と secret_key の両方が必要です",
      powDifficultyInvalid:
        "PoW 難易度は 10000〜1000000 の範囲で 10000 単位である必要があります",
      powEnabledBooleanRequired:
        "通常と異なる場所の難易度スイッチは真偽値である必要があります",
      powUncommonDifficultyTooLow:
        "通常と異なる場所の難易度は基本難易度未満にできません",
    },
    ipLocation: {
      ipLookupUrlLabel: "IP 位置情報データベースの URL",
      cidrUrlLabel: "CIDR データベースの URL",
      loadSettingsFailed: "IP ロケーション API 設定の読み込みに失敗しました",
      saveSettingsFailed: "IP ロケーション API 設定の保存に失敗しました",
      modeInvalid: "モードは online または custom である必要があります",
    },
    connectionTest: {
      httpStatus: "サービスから HTTP ステータス {status} が返されました",
      invalidData: "サービスから不正なデータが返されました",
      success: "接続に成功しました",
      timeout: "接続タイムアウト",
      failed: "接続に失敗しました",
    },
    autoHttps: {
      dockerUnsupported: "Docker 版は自動 HTTPS に対応していません",
      openWrtUnsupported: "OpenWrt 版は自動 HTTPS に対応していません",
      startFailed: "自動 HTTPS 起動に失敗しました",
    },
    hostMappings: {
      ungrouped: "未分類",
      payloadObjectRequired:
        "Host マッピングはオブジェクトである必要があります",
      hostRequired: "Host マッピングにはドメインが必要です",
      hostWildcardForbidden:
        "Host マッピング {host} にワイルドカード * は使用できません。完全なホスト名を入力してください",
      duplicateHost: "Host マッピングのドメイン {host} が重複しています",
      protocolModeInvalid:
        "Host マッピング {host} の HTTPS プロトコルは auto、http1、http2 のいずれかである必要があります",
      backendProtocolUnsupported:
        "ゲートウェイバックエンドが {host} の HTTPS プロトコル {mode} を適用できませんでした。ゲートウェイバックエンドを更新してください",
      targetPathModeInvalid:
        "Host マッピング {host} の転送先パスモードは entry または prefix である必要があります",
      backendTargetPathModeUnsupported:
        "ゲートウェイバックエンドが {host} の転送先パスモード {mode} を適用できませんでした。ゲートウェイバックエンドを更新してください",
      visibilityInvalid:
        "Host マッピング {host} の公開範囲設定が無効です: {message}",
      backendVisibilityUnsupported:
        "ゲートウェイバックエンドが {host} の公開範囲ルールを適用できませんでした。ゲートウェイバックエンドを更新してください",
      revisionConflict:
        "別のページで Host マッピングが更新されました。更新してから再試行してください",
      renamePreviousHostInvalid:
        "Host マッピング {host} の変更前のホスト名が無効です",
      renameDestinationExists:
        "Host マッピング {host} は既に存在するため、{previousHost} から名前を変更できません",
      renamePreviousHostStillPresent:
        "変更前の Host マッピング {previousHost} が一覧に残っているため、名前変更元として使用できません",
      renamePreviousHostMissing:
        "変更前の Host マッピング {previousHost} は存在しません",
      renamePreviousHostClaimed:
        "変更前の Host マッピング {previousHost} が複数のマッピングから重複して指定されています",
      targetInvalid:
        "Host マッピング {host} の転送先は http://、https://、ws://、wss:// のいずれかで始め、ホスト名を含めてください",
      singleAuthPortMapping:
        "認証サービスとして AUTH_PORT を参照できる Host マッピングは1件だけです",
      authMappingMustBePublic:
        "認証サービス {host} は公開状態を維持する必要があります。自身への認証や厳格なホワイトリストを有効にすると、ログイン画面へ到達できなくなります。",
      authMappingBasicAuthForbidden:
        "認証サービス {host} では認証情報の自動送信を有効にできません",
      basicAuthInvalid:
        "Host マッピング {host} で認証情報を自動送信するには、ユーザー名とパスワードが必要です。ユーザー名にコロンは使用できません",
      customIconInvalid:
        "Host マッピング {host} のカスタムアイコンが無効か、形式がサポートされていません",
      locationPathRequired:
        "Host マッピング {host} のパスルールにはパスが必要です",
      locationPathMustStartSlash:
        "Host マッピング {host} のパスルール {path} は / で始めてください",
      locationRootForbidden:
        "Host マッピング {host} ではルートパス / をパスルールに指定できません",
      locationReservedPath:
        "ホストマッピング {host} のパスルール {path} は予約されたパスを使用します",
      locationDuplicate:
        "Host マッピング {host} でパスルール {path} が重複しています",
      locationTargetRequired:
        "Host マッピング {host} のパスルール {path} には転送先が必要です",
      locationTargetInvalid:
        "Host マッピング {host} のパスルール {path} の転送先は http://、https://、ws://、wss:// のいずれかで始め、ホスト名を含めてください",
      locationStatusInvalid:
        "Host マッピング {host} のパスルール {path} のレスポンスステータスは 100～599 で指定してください",
      locationHeaderInvalid:
        "ホスト マッピング {host} のパス ルール {path} に不正な応答ヘッダー {header} が含まれています",
      locationHeaderForbidden:
        "Host マッピング {host} のパスルール {path} ではレスポンスヘッダー {header} を変更できません",
      syncHostRulesFailed: "Host ルートの同期に失敗しました",
      syncAuthConfigFailed: "認証ゲートウェイ設定の同期に失敗しました",
      updateFailed: "Host マッピングの更新に失敗しました",
      updateFailedRolledBack:
        "ホスト マッピングの更新に失敗し、設定はロールバックされました",
      metadataFailed: "転送先タイトルの更新に失敗しました",
      onlyHttpTargetsSupported: "http／https の転送先だけに対応しています",
      metadataUpstreamStatus:
        "アップストリームからステータス {status} が返されました",
      bookmarkFolderForRoot: "{root} サブドメインマッピング",
      bookmarkFolderDefault: "fn-knock サブドメインマッピング",
    },
    streamMappings: {
      payloadObjectRequired:
        "ストリームマッピングはオブジェクトである必要があります",
      listenPortRequiredInteger: "待受ポートには整数を指定してください",
      listenPortNotInteger: "待受ポート {port} は整数ではありません",
      listenPortOutOfRange: "待受ポート {port} は範囲外です",
      duplicatePort:
        "{protocol} の待受ポート {port} が重複しています。プロトコルとポートの組み合わせは一意にしてください",
      targetMustBeHostPort:
        "転送先 {target} は host:port 形式で指定してください",
      localTargetLoop:
        "{protocol} の待受ポート {port} をこのホストの同じポート（{target}）へ転送するとループが発生します。外部ポートまたは転送先ポートを変更してください",
      localPortLoop:
        "待受ポート {port} をこのホストの同じポートへ転送するとループが発生します。プロトコルマッピングを開き、外部ポートまたは転送先ポートを変更してください",
      saveFailed: "プロトコルマッピングの保存に失敗しました",
      disableBeforeLegacyRepair:
        "無効な古いプロトコルマッピングが残っています。削除を続ける前にプロトコルマッピングを無効にしてください。",
      syncFailed:
        "プロトコルマッピングとゲートウェイのポート許可ルールの同期に失敗しました",
      syncFailedRolledBack:
        "プロトコルマッピングとゲートウェイのポート許可ルールの同期に失敗したため、設定をロールバックしました",
    },
    passkeyRp: {
      parentDomainRequired:
        "親ドメインのパスキー RP を有効にするには、ルートドメインを入力するか、親ドメインの RP ID を明示的に指定してください。",
      mustMatchAuthHost:
        "親ドメインのパスキー RP ID {rpId} は、認証サービス {authHost} またはその親ドメインと一致させてください。",
    },
    subdomainMode: {
      payloadObjectRequired:
        "サブドメインモードのリクエスト本文はオブジェクトである必要があります",
      rootDomainWildcardForbidden:
        "ルートドメインにワイルドカード * は使用できません。*.example.com ではなく example.com を入力してください。",
      saveFailed: "サブドメインモード設定の保存に失敗しました",
      sslAutoSelected:
        "現在のサブドメインモードに適した証明書へ自動的に切り替えました。",
      sslAutoSelectionSyncFailed:
        "推奨証明書は見つかりましたが、ゲートウェイとの同期に失敗し、自動切り替えが行われませんでした。",
    },
    authMode: {
      loadFailed: "認証ログインモードの読み込みに失敗しました",
      invalidMode: "サポートされていないログインモードです",
      previewFailed: "ログインモード切り替えのプレビューに失敗しました",
      switchFailed: "ログインモードの切り替えに失敗しました",
      blockingIssues:
        "切り替えを妨げる問題が残っているため、ログインモードを変更できません",
    },
    authAccounts: {
      loadFailed: "認証アカウントの読み込みに失敗しました",
      notFound: "認証アカウントが見つかりません",
      saveFailed: "認証アカウントの保存に失敗しました",
      syncFailed: "認証アカウントを TOTP に同期できませんでした",
      usernameExists: "ユーザー名は既に存在します",
      usernameTooShort: "ユーザー名を入力してください",
      usernameTooLong: "ユーザー名は 64 文字以内にしてください",
      usernameInvalid:
        "ユーザー名には英数字、ドット、アンダースコア、ハイフンのみ使用でき、空白は使用できません",
      passwordTooShort: "アカウントパスワードを入力してください",
      passwordTooLong: "アカウントパスワードは 128 文字以内にしてください",
      passwordWhitespace: "アカウントパスワードに空白文字は使用できません",
      passwordNeedsLettersAndNumbers:
        "アカウントパスワードには英字と数字の両方を含めてください",
      passwordSaveFailed: "アカウントパスワードの保存に失敗しました",
      deleteFailed: "認証アカウントの削除に失敗しました",
      deleted: "認証アカウントを削除しました",
      totpAlreadyBound:
        "このアカウントには使用可能な TOTP がすでに登録されています",
    },
    authCredentialSettings: {
      loadFailed: "認証情報設定の読み込みに失敗しました",
      loadConfigFailed: "設定の読み込みに失敗しました",
      saveFailed: "認証情報設定の保存に失敗しました",
    },
    totp: {
      invalidCode: "認証コードが間違っています。もう一度お試しください。",
      invalidSecretOrCode:
        "TOTP シークレットまたは認証コードが正しくありません",
      notFound: "TOTP が見つかりません",
      loadFailed: "TOTP 認証情報の読み込みに失敗しました",
      saveFailed: "TOTP 認証情報の保存に失敗しました",
      exportFailed: "TOTP 認証情報のエクスポートに失敗しました",
      importFailed: "TOTP 認証情報のインポートに失敗しました",
      deleteFailed: "TOTP 認証情報の削除に失敗しました",
      updateFailed: "TOTP 認証情報の更新に失敗しました",
      bound: "TOTP 認証情報を登録しました",
      deleted: "TOTP 認証情報を削除しました",
      updated: "TOTP 認証情報を更新しました",
    },
    totpImport: {
      payloadObject:
        "TOTP 認証情報のインポート内容はオブジェクトである必要があります",
      unsupportedKind: "サポートされていない TOTP 認証情報インポート形式です",
      unsupportedVersion:
        "サポートされていない TOTP 認証情報インポートバージョンです",
      credentialsArray: "TOTP 認証情報リストは配列である必要があります",
      accountsArray: "アカウント認証情報リストは配列である必要があります",
      passwordArray:
        "アカウントパスワード認証情報リストは配列である必要があります",
      countExceeded: "一度にインポートできる TOTP 認証情報は最大 {max} 件です",
      accountCountExceeded:
        "一度にインポートできるアカウント認証情報は最大 {max} 件です",
      passwordCountExceeded:
        "一度にインポートできるアカウントパスワード認証情報は最大 {max} 件です",
    },
    passkeys: {
      notFound: "パスキーが見つかりません",
      listFailed: "パスキー一覧の読み込みに失敗しました",
      deleteFailed: "パスキーの削除に失敗しました",
      deleted: "パスキーを削除しました",
    },
    syncRoutes: {
      partialFailedGatewayLogging:
        "一部の同期に失敗しました: gateway_logging={gatewayLogging}",
      partialFailedGatewayLoggingWaf:
        "一部の同期に失敗しました: gateway_logging={gatewayLogging}、waf={waf}",
      success:
        "現在の動作モード用に、パスルート {rules} 件、Host ルート {hostRules} 件、プロトコルマッピング {streamRules} 件、リクエストログ設定、WAF 設定を同期しました。",
    },
    backup: {
      readFnosDirectoryFailed:
        "FNOS バックアップ ディレクトリの読み取りに失敗しました",
      exportFnosSuccess:
        "バックアップが FNOS ディレクトリにエクスポートされました",
      exportFnosFailed: "FNOS ディレクトリへのエクスポートに失敗しました",
      importSuccessWithWarnings:
        "バックアップはインポートされましたが、実行状態の同期の一部が失敗しました。",
      importSuccess: "バックアップをインポートし、実行状態を同期しました",
      importFailed: "バックアップのインポートに失敗しました",
      importFnosSuccessWithWarnings:
        "FNOS バックアップはインポートされましたが、実行状態の同期の一部が失敗しました",
      importFnosSuccess:
        "FNOS バックアップをインポートし、実行状態を同期しました",
      importFnosFailed: "FNOS からのバックアップのインポートに失敗しました",
    },
    sessions: {
      notFound: "セッションが見つかりません",
      listFailed: "セッション一覧の読み込みに失敗しました",
      loadFailed: "セッションの読み込みに失敗しました",
      updateFailed: "セッションの更新に失敗しました",
      deleteFailed: "セッションの削除に失敗しました",
      mobilityLoadFailed: "セッションの IP 変更履歴の読み込みに失敗しました",
      deleted: "セッションを削除しました",
    },
  },
  gatewayLogs: {
    configLoadFailed: "リクエストログ設定の読み取りに失敗しました",
    configSaveFailed: "リクエストログ設定の保存に失敗しました",
    configSyncFailed:
      "リクエストログ設定は保存しましたが、ゲートウェイとの同期に失敗しました",
    readDirectoryFailed: "ログディレクトリの読み取りに失敗しました",
    readDatesFailed: "ログ日付の読み取りに失敗しました",
    readEntriesFailed: "リクエストログの読み取りに失敗しました",
    geoRefreshActive: "IP 所在地の検索キューはすでに実行中です",
    geoRefreshFailed: "IP 所在地の検索キューを開始できませんでした",
    deleteEntriesFailed: "リクエストログの削除に失敗しました",
    invalidJsonObject: "リクエスト本文は有効な JSON オブジェクトではありません",
  },
  backoffRoutes: {
    ipRequired: "ip パラメータがありません",
    listFailed: "ログイン試行制限の一覧読み込みに失敗しました",
    statusFailed: "ログイン試行制限の状態読み込みに失敗しました",
    resetFailed: "ログイン試行制限の解除に失敗しました",
  },
  systemInfoRoutes: {
    loadAccessEntryFailed:
      "アクセス用エントリーポイントの読み込みに失敗しました",
  },
  securityOverviewRoutes: {
    loadFailed: "セキュリティ概要の読み込みに失敗しました",
  },
  ipLocationRoutes: {
    batchLimit: "一度に照会できる IP は最大 {max} 件です",
    enqueueFailed: "IP 位置検索キューへの追加に失敗しました",
  },
  gatewayPortal: {
    syncConfigFailed: "ポータル表示設定のゲートウェイへの同期に失敗しました",
    syncHostRulesFailed: "Host ルートの同期に失敗しました",
  },
  gatewayVisibility: {
    customCidrInvalid: "カスタム CIDR の形式が正しくありません: {cidrs}",
    emptyEnabledConfig:
      "公開範囲を有効にするには、地域またはカスタム CIDR を 1 件以上追加してください",
    syncFailed: "ゲートウェイの公開範囲設定を同期できませんでした",
  },
  gatewayCrawlerBlocker: {
    syncFailed: "クローラーブロック設定の同期に失敗しました",
  },
  scanner: {
    settingsLoadFailed: "スキャナー設定の読み込みに失敗しました",
    settingsUpdateFailed: "スキャナー設定の更新に失敗しました",
    invalidRequestBody: "リクエスト本文が正しくありません",
    atLeastOneIpRequired: "1 つ以上の IP を指定してください",
    blacklistLoadFailed: "スキャナー ブラックリストの読み込みに失敗しました",
    recordNotFound: "レコードが見つかりません",
    blacklistRecordLoadFailed:
      "スキャナー ブラックリスト レコードの読み込みに失敗しました",
    blacklistRecordDeleteFailed:
      "スキャナー ブラックリスト レコードの削除に失敗しました",
    blacklistRecordsDeleteFailed:
      "スキャナー ブラックリスト レコードの一括削除に失敗しました",
    cidrExemptionsInvalid: "CIDR 免除の形式が正しくありません: {cidrs}",
    pathWhitelistInvalid: "パス許可リストの形式が正しくありません",
    pathRequired: "パスは必須です",
    pathMustBeAbsolute: "パスは / で始めてください",
    pathContainsControlCharacters: "パスに制御文字は使用できません",
    ipRequired: "IP は必須です",
    pathWhitelistOperationFailed: "パス許可リストの操作に失敗しました",
  },
  gatewayLogging: {
    syncConfigFailed: "ゲートウェイのリクエストログ設定の同期に失敗しました",
  },
  sslGateway: {
    clearFailed: "ゲートウェイ証明書のクリアに失敗しました",
    syncFailed: "ゲートウェイ証明書の同期に失敗しました",
  },
  sslRoutes: {
    statusReadFailed: "SSL 状態の読み込みに失敗しました",
    gatewayStatusReadFailed: "ゲートウェイの SSL 状態を読み取れません",
    readSharedFileFailed: "共有ディレクトリファイルの読み込みに失敗しました",
    emptyDomains:
      "ドメイン一覧が空です。先にドメインまたは IP を追加してください",
    certOrKeyInvalid: "証明書または秘密鍵が無効です",
    hostRequired: "ホストは必須です",
    localCaCertificateLabel: "ローカル CA 証明書",
    rootCaNotInitialized: "ルート CA が初期化されていません",
    success: "成功",
    certNotInstalled: "証明書がインストールされていません",
    certReadFailed: "SSL 証明書を読み取れませんでした",
    certZipCreateFailed: "SSL 証明書 zip を作成できませんでした",
    manualCertificateLabel: "手動アップロード証明書",
    certNotFound: "証明書が存在しません",
    caInitFailed: "ローカル CA の初期化に失敗しました",
    caHostLoadFailed: "ローカル CA Host リストの読み込みに失敗しました",
    caHostSaveFailed: "ローカル CA Host リストの保存に失敗しました",
    certSaveFailed: "SSL 証明書の保存に失敗しました",
    certActivateFailed: "SSL 証明書の有効化に失敗しました",
    deploymentModeSaveFailed: "SSL デプロイモードの保存に失敗しました",
    certDeleteFailed: "SSL 証明書の削除に失敗しました",
    certClearFailed: "SSL 証明書設定のクリアに失敗しました",
  },
  redis: {
    defaultCredential: "デフォルトの認証情報",
    certificateLabels: {
      acme: "ACME 証明書",
      ca: "自己署名証明書",
      manual: "証明書を手動でアップロードします",
      external: "外部自動デプロイ証明書",
      current: "現在の証明書",
    },
    ssl: {
      certFormatInvalid: "無効な証明書形式: {message}",
      keyFormatInvalid: "無効な秘密鍵形式: {message}",
      certKeyMismatch: "証明書と秘密鍵が一致しません",
      certKeyCheckFailed: "証明書と秘密キーの検証に失敗しました: {message}",
      certContentRequired: "証明書の内容は必須です",
      certNotFound: "証明書が存在しません",
      certOrKeyInvalid: "証明書または秘密鍵が無効です",
    },
    acme: {
      domainRequired: "ドメインは必須です",
      domainsRequired: "ドメイン一覧は必須です",
      dnsProviderRequired: "DNS プロバイダーは必須です",
      primaryDomainDuplicated:
        "メインドメイン {primaryDomain} は別の申請設定ですでに使用されています",
      applicationNotFound: "申請設定が見つかりません",
      noMatchingIssuedCertificate:
        "この申請設定には、現在のドメイン設定に一致する発行済み証明書がありません。",
      jobDataInvalid: "ACME タスクデータが正しくありません",
      multipleApplicationsUseNewApi:
        "現在複数の申請項目があります。ACME 申請項目を管理するには新しいインターフェースを使用してください。",
    },
  },
  acmeService: {
    waiting: "操作待ち",
    sendSignalFailed: "{signal} を {target} に送信できませんでした: {detail}",
    setDefaultCaFailed:
      "デフォルトの認証局の設定に失敗しました (終了コード: {code}) {brief}",
    registerAccountFailed:
      "ACME アカウントの登録に失敗しました（終了コード: {code}）{brief}",
    bundledZipMissing: "組み込みの acmesh.zip リソースが見つかりません",
    extractingBundled: "組み込みの acme.sh リソースを解凍しています...",
    unzipFailed: "解凍に失敗しました。終了コード: {code}",
    extractedAcmeMissing: "解凍は成功しましたが、acme.sh が見つかりません",
    writingDataDir: "データディレクトリへ書き込み中...",
    writtenAcmeMissing: "書き込み後に acme.sh が見つかりません",
    checkInstallFailed: "インストール状況の確認に失敗しました: {detail}",
    ready: "acme.sh の準備ができました",
    notInstalled: "acme.sh がインストールされていません",
    initializingBundled: "組み込みの acme.sh を初期化しています...",
    registeringAccount: "ACME アカウントを登録中...",
    savingDefaultCa: "デフォルトの認証局を保存しています...",
    installSuccess: "インストールしました。アカウントのメールアドレス: {email}",
    installFailed: "インストールに失敗しました: {detail}",
    installFirst: "まず acme.sh をインストールしてください",
    installingCannotDelete: "acme.sh がインストール中のため削除できません",
    deleted: "acme.sh を削除しました",
    deleteFailed: "削除に失敗しました: {detail}",
    domainsRequired: "ドメイン一覧は必須です",
    dnsTypeRequired: "DNS 検証方式が指定されていません",
    issueFailed: "証明書の発行に失敗しました（終了コード: {code}）{brief}",
  },
  acmeJobRunner: {
    manualStop: "ユーザーが ACME タスクを手動で停止しました",
    lockMessages: {
      manualRequest: "証明書の申請",
      autoRenew: "証明書の自動更新",
    },
    activeTaskRunning:
      "現在 ACME のタスクが実行中です。後でもう一度お試しください。",
    flowFailed: "証明書の申請処理に失敗しました: {message}",
    stopSignalSent:
      "停止シグナルを送信し、acme.sh プロセスを {count} 件終了しました",
    noRunningProcess: "実行中の acme.sh プロセスが見つかりません",
    stopProcessError: "プロセスの停止中にエラーが発生しました: {message}",
    processStillRunning:
      "終了していない acme.sh プロセスがまだあります: {pids}",
    lockLost:
      "ACME 実行ロックが失われたため、タスクを停止しました。申請をやり直してください。",
    lockRefreshFailed: "ACME 実行ロックの更新に失敗しました: {message}",
    lockLeaseExpired:
      "{message}。ロックのリース期限が切れたためタスクを停止しました。申請をやり直してください。",
    applicationChangedSkipped:
      "実行中に申請設定のドメインが変更されたため、古い証明書の書き込みをスキップしました。申請をやり直してください。",
    issuedButApplicationChanged:
      "証明書は発行されましたが、申請設定のドメインが変更されていたため、現在の申請設定には保存しませんでした。",
    issuedButCertReadFailed:
      "証明書は発行されましたが、証明書ファイルの読み取りに失敗しました。後で再試行するか、acme.sh ディレクトリを確認してください。",
    clearedDomainWorkingState:
      "acme.sh のドメイン作業ディレクトリをクリアしました。今後の証明書一覧と更新はシステムタスクが管理します。",
    clearDomainWorkingStateFailed:
      "証明書は保存されましたが、acme.sh のドメイン状態をクリアできませんでした: {message}",
    linkedLibrarySyncedGateway:
      "関連する証明書ストア エントリが同期され、ゲートウェイ証明書リストが更新されました。",
    linkedLibraryUpdated: "関連する証明書ストア エントリを更新しました",
    addedToLibraryAndSyncedGateway:
      "発行した証明書を証明書ストアへ自動追加し、ゲートウェイの証明書一覧を更新しました",
    addedToLibrary: "発行した証明書を証明書ストアへ自動追加しました",
    addToLibraryFailed:
      "証明書は発行され、保存されましたが、証明書ストアへの自動追加に失敗しました: {message}",
    stoppedIgnoredProcessError:
      "タスクが停止され、プロセス終了後のエラーは無視されました",
  },
  acmeRoutes: {
    invalidRequestBody: "リクエスト本文が正しくありません",
    loadStatusFailed: "ACME 状態の読み込みに失敗しました",
    loadClientSettingsFailed: "ACME クライアント設定の読み込みに失敗しました",
    saveClientSettingsFailed: "ACME クライアント設定の保存に失敗しました",
    switchCertificateAuthorityFailed: "ACME 認証局の切り替えに失敗しました",
    loadOverviewFailed: "ACME 概要の読み込みに失敗しました",
    loadApplicationOverviewFailed: "ACME 申請設定の概要読み込みに失敗しました",
    loadConfigFailed: "ACME 設定の読み込みに失敗しました",
    loadSubdomainRecommendationFailed:
      "サブドメイン証明書推奨の読み込みに失敗しました",
    loadApplicationsFailed: "ACME 申請設定一覧の読み込みに失敗しました",
    loadApplicationFailed: "ACME 申請設定の読み込みに失敗しました",
    updateApplicationFailed: "ACME 申請設定の更新に失敗しました",
    deleteApplicationFailed: "ACME 申請設定の削除に失敗しました",
    syncLibraryFailed: "ACME 証明書の証明書ストア同期に失敗しました",
    deployCertificateFailed: "ACME 証明書の展開に失敗しました",
    loadJobFailed: "ACME タスクの読み込みに失敗しました",
    loadJobLogsFailed: "ACME タスクログの読み込みに失敗しました",
    loadJobPollFailed: "ACME タスクのポーリングに失敗しました",
    stopJobFailed: "ACME タスクの停止に失敗しました",
    loadCertificateInfoFailed: "ACME 証明書情報の読み込みに失敗しました",
    deleteCertificateFailed: "ACME 証明書の削除に失敗しました",
    uninstallFailed: "ACME クライアントのアンインストールに失敗しました",
    createCertificateZipFailed: "ACME 証明書 zip の作成に失敗しました",
    loadCertificateFailed: "ACME 証明書の読み込みに失敗しました",
    domainsInvalid: "ドメイン一覧が空か、形式が正しくありません",
    dnsTypeRequired: "DNS 検証方式が指定されていません",
    unsupportedDnsProvider: "対応していない DNS プロバイダーです",
    missingDnsCredentials:
      "DNS API 認証情報がありません。次のオプションのいずれかを入力してください: {requirements}",
    cloudflareInvalidKey:
      "Cloudflare API キーが正しくありません（X-Auth-Key の形式が不正です）",
    cloudflareInvalidEmail:
      "Cloudflare のメールアドレスが正しくありません（X-Auth-Email の形式が不正です）",
    cloudflareInvalidHeaders:
      "Cloudflare API のリクエストヘッダーが不正です。API キーまたはメールアドレスを確認してください。",
    acmeFrequencyLimited:
      "申請回数の制限に達しました（Retry-After={seconds}秒。600秒を超える場合は自動再試行しません）。しばらく待ってから再試行してください。",
    dnsApiRateLimited:
      "DNS API のレート制限に達しました（429）。後でもう一度お試しください。",
    logUnknownFailure:
      "ログでエラーが検出されましたが、自動的に関連付けられませんでした",
    installingRetryLater:
      "acme.sh がインストールされています。後でもう一度試してください。",
    installFirst: "まず acme.sh をインストールしてください",
    multipleApplicationsUseNewApi:
      "現在複数の申請項目があります。ACME 申請項目を管理するには新しいインターフェースを使用してください。",
    applicationNotFound: "申請設定が見つかりません",
    notFound: "見つかりません",
    installingCannotDelete: "acme.sh がインストール中のため削除できません。",
    installingCannotSwitchCa:
      "acme.sh のインストール中は認証局を切り替えられません。",
    noMatchingIssuedCertificate:
      "この申請設定には、現在のドメイン設定に一致する発行済み証明書がありません。",
    success: "成功",
    dns01Only: "DNS-01 検証だけに対応しています",
    certNotFound: "証明書が存在しません",
    certOrKeyInvalid: "証明書または秘密鍵が無効です",
  },
  acmeDnsProviders: {
    groups: {
      common: "よく使われる",
      domestic: "国内",
      international: "インターナショナル",
      selfHostedAdvanced: "セルフホスト / 高度",
    },
    credentialSchemes: {
      default: "デフォルトの認証情報",
    },
    fields: {
      accountEmail: "アカウントのメールアドレス",
      sshPrivateKeyPath: "SSH 秘密鍵ファイルのパス",
    },
    labels: {
      aliyun: "Alibaba Cloud DNS",
      tencentCloudDnspod: "Tencent Cloud DNSPod（TencentCloud）",
      huaweiCloudDns: "Huawei Cloud DNS",
      jdCloudDns: "JD Cloud DNS",
      westCn: "West.cn",
    },
    cloudflare: {
      globalKeyDescription:
        "Cloudflare レガシー グローバル API キー方式と互換性があります。",
      apiTokenDescription:
        "推奨方式です。トークンだけで使用できます。Zone ID または Account ID も入力すると、自動検出を省略できます。",
    },
    gcloud: {
      description:
        "実行環境の gcloud コマンドと認証済み設定を使用します。空欄の場合は gcloud のデフォルト設定を使用します。",
    },
    azure: {
      managedIdentityDescription:
        "AZUREDNS_MANAGEDIDENTITY に true を設定してください。",
    },
    descriptions: {
      boolean01: "0 または 1 を入力してください。",
      optionalBoolean01: "省略可能。0 または 1 を入力してください。",
    },
    requirements: {
      optionalSuffix: "、省略可能: {keys}",
      orSeparator: "、または ",
    },
  },
  acmePatches: {
    duckdns: {
      scriptMissing: "DuckDNS DNS API スクリプトが見つかりません: {path}",
      proxyApplied: "DuckDNS API を {from} から {to} へ切り替えました",
    },
  },
  reverseProxyTrustedIps: {
    syncFailed: "リバースプロキシのレート制限除外 IP の同期に失敗しました",
  },
  commonAuthLocations: {
    cidrLookupFailed: "CIDR の検索に失敗しました",
    syncFailed: "共通地域の除外設定をゲートウェイへ同期できませんでした",
  },
  generalBlacklist: {
    invalidRequestBody: "リクエスト本文が正しくありません",
    invalidIp: "IP アドレスが正しくありません",
    invalidIpWithValue: "IP アドレスが正しくありません: {ip}",
    atLeastOneValidIpRequired: "有効な IP を少なくとも 1 つ指定してください",
    backendRequestFailed:
      "共通ブラックリストのバックエンドリクエストに失敗しました",
    backendResponseMissingData:
      "共通ブラックリストのバックエンド応答にデータがありません",
  },
  fnosDataShare: {
    invalidPath: "共有ファイルのパスが正しくありません",
    shareMissing:
      "FNOS 共有ディレクトリが見つかりませんでした。アプリケーション リソースが正しく設定されていることを確認してください。",
    fileOnly: "共有ディレクトリ内のファイルだけを読み取れます",
    fileTooLarge:
      "ファイルが大きすぎます。証明書または秘密鍵のテキスト ファイルのみを入れてください。",
  },
  autoHttps: {
    listenEacces:
      "ポート 80 を待ち受ける権限がありません。現在のデバイスまたはコンテナで、プロセスが特権ポートへバインドできることを確認してください。",
    listenEaddrinuse:
      "ポート 80 が別のプログラムで使用されているため、自動 HTTPS を起動できません。FNOS の「システム設定 → セキュリティ → ポート設定」を編集し、ポート 80 と 443 のリダイレクトを無効にしてください。",
    listenFailedWithMessage: "ポート 80 の待受に失敗しました: {message}",
    listenFailed: "ポート 80 の待受に失敗しました",
  },
  wafCollector: {
    drainFailed: "WAF イベントの取得に失敗しました",
  },
  hostMappingBookmarks: {
    defaultFolderTitle: "fn-knock サブドメインマッピング",
  },
  whitelist: {
    listFailed: "ホワイトリスト レコードの読み込みに失敗しました",
    addFailed: "ホワイトリスト レコードの追加に失敗しました",
    updateRecordsFailed: "ホワイトリスト レコードの更新に失敗しました",
    deleteFailed: "ホワイトリスト レコードの削除に失敗しました",
    commentUpdateFailed: "ホワイトリストの備考更新に失敗しました",
    regionListFailed: "地域ホワイトリストの読み込みに失敗しました",
    regionAddFailed: "地域ホワイトリストの追加に失敗しました",
    regionDeleteFailed: "地域ホワイトリストの削除に失敗しました",
    regionRequired: "少なくとも 1 つの地域を選択してください",
    regionEmpty: "選択した地域で使用可能な CIDR が見つかりませんでした",
    regionNotFound: "地域ホワイトリストが見つかりませんでした",
    recordNotFound: "ホワイトリスト レコードが見つかりませんでした",
    domainResolveFailed: "ドメイン名解決に失敗しました",
    refreshFailed: "ホワイトリスト レコードをすぐに更新できませんでした",
  },
  whitelistManager: {
    dnsRecordQueryFailedWithCode:
      "{label} レコードの照会に失敗しました（{code}）: {message}",
    dnsRecordQueryFailed: "{label} レコードの照会に失敗しました: {message}",
    targetFormatInvalid: "IP、CIDR、またはドメイン名の形式が正しくありません",
    autoGrantIpOnly: "ログイン時の自動許可は単一の IP だけに対応しています",
    cidrInvalid: "CIDR 形式が正しくありません",
    domainInvalid: "ドメイン名の形式が正しくありません",
    ipInvalid: "IP 形式が正しくありません",
    autoOwnerMissing: "自動ホワイトリストの所有者 ID がありません",
    domainResolveFailed: "ドメイン名解決に失敗しました",
    resolvedIpCount: "{count} 件の IP を解決しました",
    noAaaaRecords: "A／AAAA レコードを解決できませんでした",
    syncAllowedStateFailed:
      "ドメインの名前解決結果は更新しましたが、システムの許可状態を同期できませんでした",
  },
  terminal: {
    defaultTitle: "Web ターミナル",
    defaultSessionTitlePrefix: "セッション-",
    operationFailed: "ターミナル操作に失敗しました",
    operationFailedWithMessage: "ターミナル操作に失敗しました: {message}",
    sessionLimitReached: "ターミナルセッションの制限に達しました ({count})",
    sessionTitleRequired: "セッション名は必須です",
    sessionMissingOrExpired: "ターミナルセッションが存在しないか、期限切れです",
    attachmentExpired: "ターミナル接続の有効期限が切れました",
    inputSendFailed: "ターミナル入力の送信に失敗しました",
    resizeFailed: "ターミナルサイズ調整に失敗しました",
    sessionNotFound: "ターミナルセッションが見つかりません",
  },
  waf: {
    manifestInvalid: "システムルールマニフェストの形式が正しくありません",
    manifestMissingZipInfo:
      "システムルールマニフェストに ZIP ファイル情報がありません",
    manifestRequestFailed:
      "システムルールマニフェストの取得に失敗しました: HTTP {status}",
    manifestRefreshFailed: "システムルールマニフェストの更新に失敗しました",
    confOnly: ".conf ルールファイルだけに対応しています",
    ruleFilenameInvalid: "ルールファイル名が正しくありません",
    fileTooLarge: "{filename} が 1 MB を超えています",
    fileInvalidUtf8: "{filename} は有効な UTF-8 テキストではありません",
    filesystemDirectiveBlocked:
      "{filename} には、許可されていないファイルシステムディレクティブが含まれています",
    systemRuleDescription: "システムセキュリティルール",
    customRuleDescription: "ユーザーアップロードルール",
    enableNeedsRule:
      "WAF を有効にする前にルールファイルを1つ以上有効にしてください",
    rulesLoadFailed: "WAF ルールの読み込みに失敗しました",
    configSyncFailed: "WAF 設定をゲートウェイへ同期できませんでした",
    sourceInvalid: "ルールのソースが正しくありません",
    ruleFileNotFound: "ルールファイルが存在しません",
    zipInvalid: "システムルールの ZIP 形式が正しくありません",
    zipDirectoryInvalid: "システムルールの ZIP ディレクトリが正しくありません",
    zipUnpackedTooLarge: "解凍後のシステムルールパッケージが大きすぎます",
    zipHeaderInvalid: "システムルールの ZIP ファイルヘッダーが正しくありません",
    zipMethodUnsupported: "ZIP 圧縮方式 {method} には対応していません",
    zipSizeInvalid: "システムルールの ZIP ファイルサイズが正しくありません",
    zipPathInvalid:
      "システムルールの ZIP ファイルパスが正しくありません: {path}",
    downloadFailed: "システムルールのダウンロードに失敗しました: HTTP {status}",
    zipTooLarge: "システムルールパッケージが大きすぎます",
    zipHashMismatch: "システムルールパッケージのハッシュ検証に失敗しました",
    zipEmpty: "システムルールパッケージが空です",
    zipDuplicateFile:
      "システムルールパッケージに重複したファイルがあります: {path}",
    zipConfRootOnly:
      "システムルールパッケージの .conf ファイルはルートディレクトリに配置してください",
    zipNoConf: "システムルールパッケージに .conf ファイルがありません",
    systemRulePathInvalid: "システムルールファイルのパスが正しくありません",
    manifestEmpty: "システムルールマニフェストが空です",
    keepOneEnabledRule:
      "WAF が有効な間は、ルールファイルを1つ以上有効にしておいてください",
    uploadSelectConf: "アップロードする .conf ファイルを選択してください",
    base64Invalid: "ルールファイルの内容は有効な Base64 ではありません",
    reloadRulesFailed: "WAF ルールの再読み込みに失敗しました",
    detailsLoadFailed: "WAF 詳細の読み込みに失敗しました",
    statusReadFailed: "WAF 状態の読み取りに失敗しました",
    invalidRequestBody: "リクエスト本文が正しくありません",
    dateInvalid: "日付形式が正しくありません。YYYY-MM-DD を指定してください",
    configSaveOrLoadFailed: "WAF 設定の保存または読み込みに失敗しました",
    systemRulesSyncFailed: "システムルールの同期に失敗しました",
    ruleToggleFailed: "WAF ルールの有効状態の切り替えに失敗しました",
    ruleReadFailed: "WAF ルールの読み取りに失敗しました",
    customRuleUploadFailed: "カスタムルールのアップロードに失敗しました",
    customRuleDeleteFailed: "カスタムルールの削除に失敗しました",
    eventsDrainFailed: "WAF イベントの取得に失敗しました",
    logsQueryFailed: "WAF ログの検索に失敗しました",
    logNotFound: "WAF ログが存在しません",
    logLoadFailed: "WAF ログの読み込みに失敗しました",
    logsDeleteFailed: "WAF ログの削除に失敗しました",
  },
  oidc: {
    callbackStateExpired:
      "ログイン状態の有効期限が切れています。再度ログインしてください。",
    loginFailedRetry: "外部ログインに失敗しました。もう一度お試しください。",
    loginMethodUnavailable:
      "現在のログインモードでは外部ログインは利用できません。",
    reservedExtraAuthParam:
      "extra_auth_params には OIDC 予約パラメータ: {key} が含まれます",
    urlInvalid: "{label} には有効な URL を指定してください",
    urlMustUseHttps: "{label} には HTTPS URL を指定してください",
    providerUnsupported: "サポートされていない外部ログインプロバイダー",
    providerMissingRequiredConfig:
      "{provider} に必須設定がありません: {fields}",
    providerMissingRequiredFields:
      "外部ログインプロバイダーに必須設定がありません: {fields}",
    accessTokenMissing: "access_token が返されませんでした",
    idTokenMissing: "id_token が返されませんでした",
    callbackUrlBuildFailed:
      "外部ログインのコールバック URL を生成できません。public_auth_base_url を設定してください",
    issuerMissing: "OIDC Issuer が設定されていません",
    discoveryMissingFields: "OIDC Discovery ドキュメントに必須項目がありません",
    nonceCheckFailed: "OIDC ノンス検証に失敗しました",
    issuerCheckFailed: "OIDC 発行者の検証に失敗しました",
    subjectEmpty: "OIDC Subject が空です",
    githubUserIdEmpty: "GitHub ユーザー ID が空です",
    providerNotFound: "外部ログインプロバイダーが見つかりません",
    connectionTestSuccess: "接続テストに成功しました",
    oauthEndpointIncomplete: "OAuth2 エンドポイントが完全に設定されていません",
    connectionTestFailed: "接続テストに失敗しました",
    totpMissing: "TOTP 認証情報が存在しません",
    selectProvider: "外部ログインプロバイダーを選択してください",
    providerUnavailable: "外部ログインプロバイダーは利用できません",
    bindingNotFound: "外部アカウントの紐付けが見つかりません",
    inviteInvalid: "紐付け用の招待リンクが無効です",
    inviteExpired: "紐付け用の招待リンクの有効期限が切れました",
    inviteProviderNotAllowed:
      "この招待リンクはこのプロバイダーの使用を許可されていません",
    authorizationEndpointMissing: "認証エンドポイントが設定されていません",
    authorizationEndpointInvalid: "認証エンドポイントの形式が正しくありません",
    bindStateInvalid: "紐付け用招待の状態が無効です",
    accountNotBoundCannotLogin:
      "この外部アカウントは紐付けられていないため、ログインできません",
    tokenEndpointMissing: "トークンエンドポイントが設定されていません",
    clientIdMissing: "client_id が設定されていません",
    bindProviderMismatch: "紐付け用招待とログインプロバイダーが一致しません",
    inviteTotpMissing: "紐付け用招待に関連付けられた TOTP は存在しません",
    accountAlreadyBoundOtherTotp:
      "この外部アカウントは別の TOTP に紐付けられています",
    inviteUsed: "紐付け用の招待リンクは使用済みです",
    externalAccountFallback: "外部アカウント",
    loginFailedWithDetail: "外部ログインに失敗しました: {detail}",
    tokenRequestFailed: "外部ログイントークンの取得に失敗しました: {detail}",
    readResponseFailed: "外部ログイン応答の読み取りに失敗しました: {detail}",
    httpResponseFailed:
      "外部ログインリクエストに失敗しました: HTTP {status}: {detail}",
    jsonResponseInvalid:
      "外部ログイン応答は有効な JSON ではありません: {detail}",
    jwksUriMissing: "OIDC JWKS URI が設定されていません",
    jwksFetchFailed: "OIDC JWKS の取得に失敗しました: {detail}",
    jwksInvalid: "OIDC JWKS 応答が無効です: {detail}",
    tokenHeaderInvalid: "OIDC token header が無効です: {detail}",
    signingKeyUnavailable: "OIDC 署名キーを使用できません",
    signingKeyInvalid: "OIDC 署名キーが無効です: {detail}",
    idTokenVerificationFailed: "OIDC id_token の検証に失敗しました: {detail}",
    githubProfileRequestFailed:
      "GitHub プロフィールのリクエストに失敗しました: {detail}",
    providerErrors: {
      accessDenied:
        "外部ログイン認証をキャンセルしたか、プロバイダーによって認証リクエストが拒否されました。",
      temporarilyUnavailable:
        "外部ログインサービスは一時的に利用できません。しばらくしてからもう一度お試しください。",
      serverError:
        "外部ログインプロバイダーがサービスエラーを返しました。後でもう一度試してください。",
      invalidScope:
        "外部ログイン許可範囲が正しく設定されていません。管理者に連絡してプロバイダーの設定を確認してください。",
      rejected:
        "外部ログイン要求はプロバイダーによって拒否されました。外部ログイン設定を確認して、もう一度お試しください。",
      incomplete:
        "外部ログインが完了していません。ログインをやり直してください。",
    },
    bindWithProvider: "{provider} で紐付け",
    selectProviderTitle: "外部アカウントプロバイダーを選択",
    bindToTotp: "外部アカウントを {totp} に紐付けます。",
    linkMissingToken: "リンクにトークンがありません",
    inviteMissingExpiredUsed:
      "招待が存在しないか、有効期限切れ、または使用済みです。",
    noProvidersTitle: "利用可能な外部ログインプロバイダーはありません",
    noProvidersBody:
      "この招待で紐付け可能な外部アカウントプロバイダーがありません。",
    bindFailedTitle: "外部アカウントの紐付けに失敗しました",
    bindStartFailed: "外部アカウントの紐付けを開始できません。",
    startFailed: "外部ログインの開始に失敗しました",
    callbackMissingParams:
      "外部ログイン コールバックに必要なパラメータがありません。ログインを再度開始してください。",
    loginFailed: "外部ログインに失敗しました",
    operationAborted:
      "外部ログイン要求が中断されました。ログインを再度開始してください。",
    loginFailedRetryAfter: "{message}、{seconds} 秒後にもう一度お試しください",
    createProviderFailed: "外部ログインプロバイダーの作成に失敗しました",
    updateProviderFailed: "外部ログインプロバイダーの更新に失敗しました",
    deleteProviderFailed: "外部ログインプロバイダーの削除に失敗しました",
    testProviderFailed: "外部ログインプロバイダーのテストに失敗しました",
    deleteBindingFailed: "外部アカウントの紐付け削除に失敗しました",
    createInviteFailed: "紐付け用招待の作成に失敗しました",
    listProvidersFailed: "外部ログインプロバイダー一覧の取得に失敗しました",
    providerPayloadObject:
      "プロバイダーのペイロードはオブジェクトである必要があります",
    loadProviderFailed: "外部ログインプロバイダーの読み込みに失敗しました",
    listBindingsFailed: "外部アカウントの紐付け一覧取得に失敗しました",
    invitationPayloadObject: "招待ペイロードはオブジェクトである必要があります",
    totpRequired: "TOTP 認証情報が必要です",
    loadTotpFailed: "TOTP 認証情報の読み込みに失敗しました",
    loadConfigFailed: "設定の読み込みに失敗しました",
    inviteUrlBuildFailed: "外部アカウント招待 URL の作成に失敗しました",
    connectionConfigInvalid:
      "外部ログインプロバイダーの接続設定が正しくありません",
    oauthEndpointIncompleteWithField:
      "OAuth2 エンドポイント設定が不完全です: {field}",
    discoveryHttpFailed:
      "OIDC discovery リクエストに失敗しました: HTTP {status}: {detail}",
    discoveryInvalid: "OIDC Discovery ドキュメントが正しくありません",
    discoveryMissingFieldsWithList:
      "OIDC discovery ドキュメントに必須フィールドがありません: {fields}",
    providerTypeRequired: "外部ログインプロバイダーの種類が必要です",
    storedProviderInvalid: "保存済みの外部ログインプロバイダーが無効です",
    storedProviderTypeInvalid:
      "保存済みの外部ログインプロバイダーの種類が無効です",
    catalog: {
      googleDescription: "Google アカウントでサインインします。",
      microsoftDescription: "Microsoft / Azure AD アカウントでログインします。",
      githubDescription: "GitHub OAuth を使用してログインします。",
      customLabel: "カスタム OIDC",
      customDescription:
        "標準の OpenID Connect Discovery を使用するカスタムプロバイダー。",
    },
  },
  ldap: {
    catalog: {
      openldapLabel: "OpenLDAP",
      activeDirectoryLabel: "Active Directory",
      customLabel: "カスタム LDAP",
    },
    listProvidersFailed: "LDAP プロバイダーの取得に失敗しました",
    providerPayloadObject: "プロバイダー設定はオブジェクトである必要があります",
    createProviderFailed: "LDAP プロバイダーの作成に失敗しました",
    providerNotFound: "LDAP プロバイダーが見つかりません",
    loadProviderFailed: "LDAP プロバイダーの読み込みに失敗しました",
    updateProviderFailed: "LDAP プロバイダーの更新に失敗しました",
    deleteProviderFailed: "LDAP プロバイダーの削除に失敗しました",
    connectionTestSuccess: "LDAP 接続テストに成功しました",
    testProviderFailed: "LDAP 接続テストに失敗しました",
    testCredentialsRequired:
      "ディレクトリユーザー名とパスワードを両方指定してください",
    listBindingsFailed: "LDAP 連携の取得に失敗しました",
    bindingNotFound: "LDAP 連携が見つかりません",
    deleteBindingFailed: "LDAP 連携の削除に失敗しました",
    invitationFieldsRequired: "TOTP 認証情報と LDAP プロバイダーが必要です",
    loadTotpFailed: "TOTP 認証情報の読み込みに失敗しました",
    totpMissing: "TOTP 認証情報が見つかりません",
    providerUnavailable: "LDAP プロバイダーを利用できません",
    loadConfigFailed: "設定の読み込みに失敗しました",
    inviteUrlBuildFailed: "LDAP 招待 URL の生成に失敗しました",
    createInviteFailed: "LDAP 招待の作成に失敗しました",
    loginMethodUnavailable:
      "現在のログインモードでは LDAP ログインを利用できません",
    inviteInvalid: "LDAP 招待が無効です",
    inviteExpired: "LDAP 招待の期限が切れているか、使用済みです",
    serviceUnavailable: "ディレクトリサービスを一時的に利用できません",
    invalidCredentialsWithRetry:
      "ディレクトリ認証情報が無効です。{seconds} 秒後に再試行してください。",
    invalidCredentials: "ディレクトリ認証情報が無効か、アカウントが未連携です",
    bindingConflict: "このディレクトリ ID は連携済みか、招待が使用済みです",
    bindingFailed: "ディレクトリ ID の連携に失敗しました",
    createSessionFailed: "ログインセッションの作成に失敗しました",
    loginSuccessful: "LDAP ログインに成功しました",
  },
  subdomainMode: {
    recommendationMissingBase:
      "ルートドメインまたは認証サービスが未設定のため、推奨する証明書ドメインを生成できません。",
    recommendationWildcardSummary:
      "推奨ドメインは {rootDomain} と *.{rootDomain} です。同じ親ドメイン配下のルートドメイン、認証サービス、アプリ用サブドメインをカバーします。",
    authOutOfRootWarning:
      "認証サービス {authHost} はルートドメイン {rootDomain} の配下ではないため、完全なドメインとして個別に追加しました。選択した DNS プロバイダーでこれらのドメインを管理できることを確認してください。",
    recommendationSingleHostSummary:
      "ルートドメインが未設定のため、認証サービス {authHost} 用の単一ドメイン証明書だけを推奨できます。",
    wildcardSuggestion:
      "複数のアプリ用サブドメインをカバーする場合は、先にルートドメインを設定してからワイルドカード証明書を申請してください。",
    configureRootOrAuth:
      "先にサブドメインモードでルートドメインを設定するか、Host マッピングで認証サービスを指定してください。",
    authMissingWarning:
      "認証サービスが指定されていないため、ルートドメインだけを基に推奨範囲を算出しています。",
    uncoveredHostMappingsWarning:
      "推奨証明書でカバーされない Host マッピングが {count} 件あります。公開する場合は、証明書の追加またはドメイン設定の見直しが必要です。",
    coverageNoSsl:
      "SSL 証明書が有効になっていないため、認証サービスとアプリ用サブドメインは HTTPS で保護されていません。",
    coverageReadyConcrete:
      "展開中の証明書は、認証サービスと設定済みのすべての Host マッピングをカバーしています。",
    coverageReadyRecommended:
      "現在展開されている証明書は、サブドメインモードで推奨される範囲を満たしています。",
    coveragePartialConcrete:
      "現在の証明書は、サブドメインモードに必要なドメインの一部しかカバーしていません。認証サービスまたは一部のアプリ用 Host で証明書が一致しない可能性があります。",
    coveragePartialRecommended:
      "現在の証明書がカバーするのは推奨ドメインの一部だけです。今後サブドメインモードを有効にすると、証明書が一致しない可能性があります。",
    coverageMismatchConcrete:
      "現在展開されている証明書はサブドメインモードと一致せず、認証サービスとサービス用 Host を正しくカバーしていません。",
    coverageMismatchRecommended:
      "現在展開されている証明書は、サブドメインモードで推奨されるドメインをカバーしていません。",
    coverageMissingRequiredWarning:
      "現在の証明書では必須ドメインが {count} 件不足しています。証明書を再発行または置き換えてください。",
    coverageMissingRecommendedWarning:
      "現在の証明書では推奨ドメインが {count} 件不足しています。今後使用する場合は、証明書を再発行または置き換えてください。",
    coverageAuthHostMissingWarning:
      "現在の証明書は認証サービス {authHost} をカバーしていません。",
    inventoryEmpty:
      "証明書ストアにはサブドメインモードで使用できる証明書がありません。",
    inventoryActiveReady:
      "現在有効な証明書は、サブドメインモードで必要なドメインをすべてカバーしています。",
    inventoryOneReady:
      "証明書ストアには、サブドメインモードを完全にカバーし、そのまま有効化できる証明書が 1 件あります。",
    inventoryMultipleReady:
      "証明書ストアには、現在のサブドメインモードを完全にカバーできる証明書が {count} 件あります。",
    inventoryCombinedReady:
      "証明書ストア内の証明書を組み合わせると、必要なドメインをすべてカバーできます。",
    inventoryCandidateReady:
      "現在のサブドメインモードをカバーできる証明書が、証明書ストアにすでにあります。",
    inventoryCombinedNeedsMultiSni:
      "証明書ストア内の証明書を組み合わせれば現在のサブドメインモードをカバーできますが、ゲートウェイが単一証明書モードのため、すべてを同時に有効化できません。",
    inventoryPartialCandidates:
      "証明書ストアにはすでに候補証明書がいくつかありますが、認証サービスとすべてのホスト マッピングを完全にカバーすることはできません。",
    inventoryNoCertificateCoversRecommendation:
      "現在、サブドメインモードで推奨されるドメインをカバーできる証明書はありません。",
    inventoryMultiCertRequiresSniWarning:
      "必要なドメインをカバーするには複数の証明書が必要ですが、ゲートウェイが単一証明書モードのため、すべてを同時に有効化できません。",
    inventorySwitchRecommendedWarning:
      "現在有効な証明書はサブドメインモードと完全には一致しません。推奨証明書へ切り替えてください。",
    inventoryBetterForSniWarning:
      "証明書ストアの内容は、今後マルチ証明書 SNI で展開する設定に適しています。",
  },
  cloudflared: {
    configReadFailed: "Cloudflared 設定の読み込みに失敗しました",
    statusLoadFailed:
      "Cloudflared スーパーバイザー状態の読み込みに失敗しました",
    configWriteFailed: "Cloudflared 設定の保存に失敗しました",
    missingToken: "先に Cloudflare トークンを設定してください",
    startFailedWithDetail: "Cloudflared の起動に失敗しました: {detail}",
    processExited: "cloudflared プロセスが終了しました",
    processExitedWithCode:
      "Cloudflared プロセスが終了しました (終了コード {code})",
    processCrashed: "cloudflared プロセスが異常終了しました: {message}",
    resumeOnBoot:
      "再開: 前回 cloudflared が実行中だったため、自動的に復旧しています...",
    unknownError: "不明なエラー",
    notInitialized: "Cloudflared が初期化されていません",
    startFailed: "起動に失敗しました",
    stopFailed: "Cloudflared の停止に失敗しました",
    logsListFailed: "Cloudflared ログの読み込みに失敗しました",
    logsClearFailed: "Cloudflared ログの消去に失敗しました",
    logsPollFailed: "Cloudflared ログのポーリングに失敗しました",
  },
  dnsmasq: {
    notDetectedInstallFirst:
      "dnsmasq が検出されません。最初にインストールを完了してください。",
    dnsPortUnavailable:
      "DNS ポート 53 を使用できません。ポートを解放して再試行してください。",
    dnsPortUnavailableWithDetail:
      "DNS ポート 53 を使用できません。ポートを解放して再試行してください: {detail}",
    detectedWithVersion:
      "dnsmasq が検出されました: {version}、初期化またはサービスの起動を待機しています",
    detected:
      "dnsmasq が検出されました。サービスの初期化または開始を待機しています。",
    missingServiceAutoComplete:
      "システムサービスがないため、初期化時に自動でセットアップします。",
    servicePackageMissing:
      "dnsmasq 実行可能ファイルは検出されましたが、システム サービスがインストールされていません。最初に dnsmasq パッケージをインストールしてください",
    completingService: "dnsmasq システムサービスをセットアップ中...",
    completeServiceFailed:
      "dnsmasq システムサービスのセットアップに失敗しました",
    serviceDefinitionMissingAfterInstall:
      "dnsmasq サービスのインストール後に、利用可能なシステム サービス定義が検出されません。",
    executableMissing: "dnsmasq 実行ファイルを検出できません",
    configTestFailed: "dnsmasq 設定の検証に失敗しました",
    enableServiceFailed: "dnsmasq の自動起動を有効にできませんでした",
    restartFailed: "dnsmasq の再起動に失敗しました",
    stopServiceFailed: "dnsmasq を停止できませんでした",
    disableServiceFailed: "dnsmasq の自動起動を無効にできませんでした",
    serviceDefinitionMissing:
      "dnsmasq システムサービス定義が検出されません。サービス環境を完成させるために、まず初期化を完了してください。",
    readyWithVersion: "dnsmasq の準備が完了しました: {version}",
    ready: "dnsmasq の準備ができました",
    refreshingApt: "Debian パッケージリストを更新中...",
    aptUpdateFailed: "apt-get update の実行に失敗しました",
    installing: "dnsmasq をインストールしています...",
    aptInstallFailed: "apt-get install dnsmasq の実行に失敗しました",
    enablingService: "dnsmasq サービスを有効にしています...",
    verifyingService: "dnsmasq サービスを確認しています...",
    installMissingAfterComplete: "dnsmasq が検出されません",
    installFailed: "dnsmasq のインストールに失敗しました",
    checkingEnvironment: "dnsmasq 環境を確認しています...",
    validatingConfig: "dnsmasq 設定を確認しています...",
    startingService: "dnsmasq サービスを開始しています...",
    initializeFailed: "dnsmasq の初期化に失敗しました",
  },
  firewall: {
    goBackendCallFailed:
      "Go バックエンド API の呼び出しに失敗しました: {message}",
    clearLegacyTcpRedirectFailed:
      "従来の TCP リダイレクト {listenPort} → {targetPort} のクリアに失敗しました",
    initDefaultRulesFailed:
      "デフォルトのファイアウォールルールを初期化できませんでした",
    syncWhitelistTargetFailed:
      "ホワイトリストの同期先 {target} を同期できませんでした",
    cleanRulesFailed: "ファイアウォールルールを消去できませんでした",
    syncAuthGatewayConfigFailed: "認証ゲートウェイ設定の同期に失敗しました",
    syncReverseProxyThrottleFailed:
      "リバースプロキシのレート制限設定の同期に失敗しました",
    syncGatewayVisibilityConfigFailed:
      "ゲートウェイの公開範囲設定を同期できませんでした",
    syncGatewayProxyHeadersConfigFailed:
      "ゲートウェイのプロキシヘッダー設定の同期に失敗しました",
    syncGatewayHostResponseConfigFailed:
      "ゲートウェイの Host ヘッダー設定の同期に失敗しました",
    syncGatewayCrawlerBlockerConfigFailed:
      "クローラーブロック設定の同期に失敗しました",
    enableProxyProtocolForceFailed:
      "Proxy Protocol 強制モードを有効にできませんでした",
    disableProxyProtocolForceFailed:
      "Proxy Protocol 強制モードを無効にできませんでした",
    disableStreamRulesFailed:
      "プロトコルマッピングのリスナーを無効にできませんでした",
    flushPathRoutesFailed: "パスルートのクリアに失敗しました",
    syncHostRoutesFailed: "Host ルートの同期に失敗しました",
    syncDefaultRouteFailed: "デフォルトルートの同期に失敗しました",
    flushHostRoutesFailed: "Host ルートのクリアに失敗しました",
    syncPathRoutesFailed: "パスルートの同期に失敗しました",
    syncStreamRulesFailed: "プロトコルマッピングの同期に失敗しました",
    syncAuthEntryRouteFailed: "認証エントリルートの同期に失敗しました",
    syncAuthDefaultRouteFailed: "認証用デフォルトルートの同期に失敗しました",
  },
  updateManager: {
    manifestFieldInvalid: "更新マニフェストの {field} が正しくありません",
    manifestFormatInvalid: "更新マニフェストの形式が正しくありません",
    manifestMissingVersion: "更新マニフェストにバージョンがありません",
    manifestMissingUpdateAvailable:
      "更新マニフェストに update_available がありません",
    manifestMissingForceUpdate: "更新マニフェストに force_update がありません",
    manifestMissingDownloadUrl: "更新マニフェストに download_url がありません",
    manifestArm64FieldsIncomplete:
      "更新マニフェストの ARM64 ダウンロード情報が不足しています",
    architectureUnsupported:
      "現在のシステムアーキテクチャは自動更新に対応していません: {arch}",
    manifestMissingArm64DownloadUrl:
      "更新マニフェストに ARM64 のダウンロード URL がありません",
    manifestMissingArm64Checksum:
      "更新マニフェストに ARM64 のチェックサムがありません",
    checkHttpFailed: "更新の確認に失敗しました: HTTP {status}",
    checkFailed: "更新の確認に失敗しました",
    noUpdateInfo: "更新情報をまだ取得していません",
    featureDisabled: "更新機能は現在無効です",
    alreadyLatest: "最新バージョンです",
    downloadHttpFailed: "ダウンロードに失敗しました: HTTP {status}",
    responseBodyUnreadable:
      "ダウンロードに失敗しました: 応答本文を読み取れません",
    checksumFailed:
      "チェックサムが一致しません: 期待値 {expected}、実際の値 {actual}",
    downloadFailed: "ダウンロードに失敗しました",
    noInstallableUpdate: "現在、インストールするアップデートはありません",
    downloadPackageFirst:
      "まずアップデートパッケージをダウンロードして確認してください",
    packageMissing:
      "アップデートパッケージが存在しません。再度ダウンロードしてください。",
    packageChecksumFailed:
      "アップデートパッケージの検証に失敗しました。もう一度ダウンロードしてください",
    installStartFailed:
      "アップデートのインストールプロセスの開始に失敗しました",
  },
  tunnelManagers: {
    cloudflared: {
      macAutoDownloadUnsupported:
        "macOS では自動ダウンロードに対応していません。brew install cloudflared で手動インストールしてください。",
      platformUnsupported: "現在のプラットフォームには対応していません",
      downloadStarted: "Cloudflared のダウンロードを開始しました",
      responseBodyUnreadable: "ダウンロード応答本文が読めません",
      downloadCancelled: "ダウンロードがキャンセルされました",
      unknownError: "不明なエラー",
      deleteSuccess: "Cloudflared を削除しました",
      deleteFailed: "Cloudflared の削除に失敗しました: {detail}",
      macManualRemove: "macOS では cloudflared を手動で削除してください",
      notInstalledBrew:
        "cloudflared がインストールされていません。先に brew install cloudflared を実行してください。",
      notInitialized:
        "Cloudflared は初期化されていません。最初にダウンロードしてください",
    },
    frp: {
      platformUnsupported: "現在のプラットフォームには対応していません",
      packageMissing: "FRP インストールパッケージがありません",
      extractFailed: "解凍に失敗しました。終了コード {code}",
      downloadStarted: "FRP のダウンロードを開始しました",
      responseBodyUnreadable: "ダウンロード応答本文が読めません",
      connectionFailed: "接続に失敗しました",
      downloadFailed: "ダウンロードに失敗しました: {detail}",
      unknownError: "不明なエラー",
      downloadCancelled: "ダウンロードがキャンセルされました",
      deleteSuccess: "FRP を削除しました",
      deleteFailed: "FRP の削除に失敗しました: {detail}",
      notInitialized:
        "FRP が初期化されていません。先にダウンロードしてください",
    },
  },
  frpc: {
    instanceNotFound: "FRP インスタンスが見つかりません: {id}",
    instanceLimitExceeded: "追加できる FRP インスタンスは最大 {limit} 件です",
    primaryName: "メイン FRP",
    instanceName: "FRP インスタンス",
    verifyFailedWithDetail: "frpc verify に失敗しました: {detail}",
    verifyFailedWithCode: "frpc verify に失敗しました（終了コード {code}）",
    verifyFrpNotInitialized:
      "FRP が初期化されていないため frpc.toml を検証できません。先にシステム設定から FRP リソースをダウンロードしてください。",
    pidInvalidForInstance:
      "PID は有効期限が切れているか、このインスタンスに属していません",
    processExited: "frpc プロセスは終了しました",
    processExitedWithCode: "frpc プロセスが終了しました (終了コード {code})",
    processCrashed: "frpc プロセスが異常終了しました: {message}",
    processStillRunning: "FRP プロセスはまだ終了していません pid={pid}",
    primaryDeleteDenied: "メイン FRP インスタンスは削除できません",
    notInitialized: "FRP が初期化されていません",
    startFailedWithDetail: "frpc の起動に失敗しました: {detail}",
    pidReadFailed: "frpc PID の読み取りに失敗しました",
    startedWithPid: "frpc が起動しました pid={pid}",
    stoppedWithPid: "frpc が停止しました pid={pid}",
    alreadyStopped: "frpc は既に停止しています",
    pidCleanedForInstance:
      "PID はこのインスタンスに属しません。このインスタンスの実行記録はクリアされました。",
    resumeOnBoot:
      "再開: 前回この FRP インスタンスが実行中だったため、自動的に復旧しています...",
    routes: {
      saveConfigFailed: "設定の保存に失敗しました",
      startFailed: "起動に失敗しました",
      stopFailed: "停止に失敗しました",
      createInstanceFailed: "インスタンスの作成に失敗しました",
      startInstanceFailed: "インスタンスの起動に失敗しました",
      stopInstanceFailed: "インスタンスの停止に失敗しました",
      restartInstanceFailed: "インスタンスの再起動に失敗しました",
      getInstanceLogsFailed: "インスタンスログの取得に失敗しました",
      clearInstanceLogsFailed: "インスタンスログのクリアに失敗しました",
      pollInstanceFailed: "インスタンスのポーリングに失敗しました",
      getInstanceDetailFailed: "インスタンスの詳細を取得できませんでした",
      updateInstanceFailed: "インスタンスの更新に失敗しました",
      deleteInstanceFailed: "インスタンスの削除に失敗しました",
    },
  },
  dockerAdminPanel: {
    passwordTooShort: "管理パネルのパスワードは 6 文字以上にしてください",
    passwordTooLong:
      "管理パネルのパスワードは 128 文字を超えることはできません",
    passwordWhitespace:
      "管理パネルのパスワードには空白文字を含めることはできません",
    passwordNeedsLettersAndNumbers:
      "管理パネルのパスワードには文字と数字の両方を含める必要があります",
    passwordAlreadyConfigured: "管理パネルのパスワードは設定済みです",
    passwordNotConfigured: "管理パネルのパスワードは未設定です",
    newPasswordSameAsCurrent:
      "新しいパスワードは現在のパスワードと同じにすることはできません",
    resetHelp:
      "fn-knock 管理パネルパスワードのリセットツール\n\n使用方法:\n  fn-knock-reset-panel-password\n\n実行内容:\n  - 管理パネルのパスワードをクリア\n  - すべての管理パネルログインセッションをクリア\n  - ログイン失敗による試行制限をクリア\n\n完了後、次回管理画面へアクセスすると初回パスワード設定が表示されます。",
    resetCleared: "[fn-knock] 管理パネルのパスワード状態をクリアしました",
    resetNextVisit:
      "[fn-knock] 次回管理画面へアクセスしたときに、管理パネルのパスワードを再設定してください",
    resetFailed: "[fn-knock] 管理パネルのパスワードをクリアできませんでした:",
  },
  passkeyRoutes: {
    notFoundWithRetry:
      "パスキーが見つかりません。{seconds}秒後にもう一度お試しください",
    verifyFailedWithRetry:
      "認証に失敗しました。{seconds} 秒後にもう一度お試しください",
    bindTokenExpired: "紐付け用の認証情報の有効期限が切れています",
    loginMethodUnavailable:
      "現在のログインモードではパスキーを利用できません。",
    loadStatusFailed: "パスキー状態の読み込みに失敗しました",
    createOptionsFailed: "パスキーオプションの作成に失敗しました",
    loadPasskeysFailed: "パスキー一覧の読み込みに失敗しました",
    noPasskeyAvailable: "利用可能なパスキーがありません",
    noValidPasskeyAvailable: "有効なパスキーがありません",
    invalidRpConfig: "パスキーの RP 設定が正しくありません",
    invalidResponse: "パスキーの応答が正しくありません",
    challengeExpired: "パスキーのチャレンジが期限切れです",
    verifyFailed: "パスキーの検証に失敗しました",
    notFound: "パスキーが見つかりません",
    createSessionFailed: "認証セッションの作成に失敗しました",
    loginSuccessful: "ログインしました",
    unauthorizedOrMissingTotp: "未認可、または TOTP ID がありません",
    createBindTokenFailed: "パスキー紐付けトークンの作成に失敗しました",
    createRegistrationOptionsFailed:
      "パスキー登録オプションの作成に失敗しました",
    registerFailed: "パスキーの登録に失敗しました",
    registrationFailed: "パスキー登録に失敗しました",
    alreadyRegistered: "パスキーはすでに登録されています",
    unknownDevice: "不明なデバイス",
  },
  authRoutes: {
    pathNotFound: "認証 API パスが見つかりません",
    loadBootstrapFailed: "認証ブートストラップの読み込みに失敗しました",
    authenticationRequired: "認証が必要です",
    loadSessionFailed: "認証セッションの読み込みに失敗しました",
    loadCaptchaConfigFailed: "Captcha 設定の読み込みに失敗しました",
    createCaptchaChallengeFailed: "Captcha チャレンジの作成に失敗しました",
    loadOidcProvidersFailed: "OIDC プロバイダーの読み込みに失敗しました",
    loadOidcInviteFailed: "OIDC 招待の読み込みに失敗しました",
    inspectOidcInviteFailed: "OIDC 招待の確認に失敗しました",
    loadAuthConfigFailed: "認証設定の読み込みに失敗しました",
    loadLoginCredentialsFailed: "ログイン認証情報の読み込みに失敗しました",
    createSessionFailed: "認証セッションの作成に失敗しました",
    loginSuccessful: "ログインしました",
    loginMethodUnavailable: "現在のログイン方法は利用できません。",
    verifyFailed: "認証状態の確認に失敗しました",
    localNetworkAccessAllowed: "ローカルネットワークアクセスが許可されました",
    authenticated: "認証済みです",
    invalidCaptchaProof: "Captcha proof が無効です",
    invalidCaptchaAlgorithm: "Captcha アルゴリズムが無効です",
    invalidCaptchaChallenge: "Captcha チャレンジが無効です",
    invalidCaptchaSignature: "Captcha 署名が無効です",
    captchaChallengeExpired: "Captcha チャレンジの有効期限が切れました",
    captchaChallengeAlreadyUsed: "Captcha チャレンジはすでに使用されています",
    captchaVerifyFailed: "Captcha の検証に失敗しました",
    turnstileResponseInvalid: "Turnstile 応答が無効です",
    unknownTotp: "不明な TOTP",
  },
  maintenanceClear: {
    confirmPhrase: "すべてのデータを消去",
    confirmationMismatch: "確認テキストが一致しません",
    clearFailed: "すべてのデータを消去できませんでした",
  },
  maintenanceBackup: {
    automaticIntervalInvalid:
      "自動バックアップ間隔は 1～8760 時間で指定してください",
    automaticRetentionInvalid:
      "自動バックアップの保存日数は 1～3650 日で指定してください",
    automaticDirectoryReadFailed:
      "自動バックアップディレクトリを読み込めませんでした",
    automaticSettingsReadFailed: "自動バックアップ設定を読み込めませんでした",
    automaticSettingsSaveFailed: "自動バックアップ設定を保存できませんでした",
    automaticSettingsInvalidRequest:
      "自動バックアップ設定のリクエスト形式が無効です",
    commandMissing: "システム環境に {command} コマンドがありません",
    commandFailed: "{command} コマンドの実行に失敗しました",
    commandCheckFailed: "{command} コマンドの確認に失敗しました",
    commandsMissingNoApt:
      "システム環境に {commands} コマンドが不足しており、Debian apt-get が見つからず、自動的にインストールできません。",
    commandsMissingNoPackageManager:
      "システム環境に {commands} コマンドがなく、opkg または Debian apt-get が見つからず、自動インストールできません。",
    opkgUpdateFailed: "opkg 更新の実行に失敗しました",
    aptUpdateFailed: "apt-get update の実行に失敗しました",
    packageInstallFailed: "{packages} のインストールに失敗しました",
    commandsStillMissingAfterInstall:
      "自動インストールが完了しても、{commands} コマンドが検出されません。",
    commandErrorWithDetail: "{message} (終了コード: {code}): {detail}",
    commandError: "{message} (終了コード: {code})",
    shareDirectoryMissing:
      "FNOS 共有ディレクトリが見つかりませんでした。アプリケーション リソースが正しく設定されていることを確認してください。",
    invalidBackupPath: "バックアップファイルのパスが不正です",
    invalidRedisStreamData:
      "Redis ストリーム データ形式が無効です: {key} ({id})",
    unsupportedRedisExportType:
      "Redis データ型 {type}（{key}）のエクスポートには対応していません",
    createArchiveFailed: "バックアップ アーカイブの生成に失敗しました",
    buildResponseFailed: "バックアップのダウンロード応答の生成に失敗しました",
    invalidBackupExtension:
      "バックアップ ファイルの拡張子は {extension} である必要があります",
    stringArrayRequired: "{label} は文字列配列でなければなりません",
    stringArrayOnlyStrings: "{label} には文字列のみを含めることができます",
    objectRequired: "{label} はオブジェクトである必要があります",
    fieldStringRequired: "{label}.{field}は文字列である必要があります",
    arrayRequired: "{label} は配列でなければなりません",
    zsetMemberRequired:
      "{label}[{index}] には文字列メンバーが含まれている必要があります",
    zsetScoreRequired:
      "{label}[{index}] には有効な数値スコアが含まれている必要があります",
    streamIdRequired:
      "{label}[{index}] には文字列 ID が含まれている必要があります",
    streamFieldsInvalid:
      "{label}[{index}].fields は偶数長の空でない文字列配列でなければなりません",
    entryObjectRequired:
      "エントリ [{index}] はオブジェクトである必要があります",
    entryKeyPrefixRequired:
      "エントリ[{index}].key は {prefix} で始まる必要があります",
    entryTypeUnsupported: "エントリ[{index}].type は対応していません",
    entryTtlInvalid:
      "エントリ[{index}].ttl_ms は正の整数または null でなければなりません",
    entryValueStringRequired:
      "エントリ[{index}].value は文字列である必要があります",
    jsonParseFailed: "バックアップ ファイル JSON を解析できません",
    payloadObjectInvalid:
      "バックアップ ファイルの内容は有効なオブジェクトではありません",
    unsupportedSchemaVersion:
      "version={version} のバックアップファイルだけに対応しています",
    unsupportedPrefix:
      "プレフィックス {prefix} のバックアップファイルだけに対応しています",
    missingAppVersion: "バックアップファイルに app_version がありません",
    appVersionUnsupported:
      "現在のバージョン {currentVersion} では、{range} のバージョンでエクスポートされたバックアップのみインポートできます。バックアップのバージョン: {appVersion}",
    missingExportedAt: "バックアップファイルに exported_at がありません",
    missingEntries: "バックアップ ファイルにエントリ配列がありません",
    duplicateRedisKey: "バックアップ ファイルに重複した Redis キーがあります",
    archiveMissingPayload: "バックアップ アーカイブに {filename} がありません",
    archivePasswordInvalid:
      "バックアップ アーカイブのパスワード検証に失敗しました",
    readArchiveFailed: ".knock バックアップ アーカイブの読み取りに失敗しました",
    payloadUtf8Invalid:
      "バックアップ ファイルの内容は有効な UTF-8 テキストではありません",
    writeRedisFailed: "Redis バックアップ データの書き込みに失敗しました",
    unknownError: "不明なエラー",
    syncSteps: {
      runModeGatewayRoutes: "動作モードとゲートウェイルーティング",
      directModeWhitelist: "ダイレクトモードのホワイトリスト",
      trustedClientIps: "ゲートウェイの信頼済みクライアント IP",
      gatewayLogging: "ログ設定のリクエスト",
      gatewayMemory: "Go ゲートウェイのメモリ設定",
      wafRuntime: "WAF 設定と実行状態",
      sslDeployment: "SSL 証明書の展開",
      autoHttps: "HTTPS 自動リダイレクト",
      smartConnect: "スマート接続",
      fnosPortIconHijack: "FNOS ポートアイコンの引き継ぎ",
      fnosNetworkTuning: "FNOS ネットワークチューニング",
      transactionFinalize: "バックアップインポート処理の確定",
      locale: "言語設定",
      legacyAuthLogCleanup: "従来の認証ログをクリーンアップ",
      systemResourceMonitorReset: "システムリソース監視状態をリセット",
    },
    archiveEmpty: "バックアップ アーカイブのコンテンツが空です",
    archiveTooLarge:
      "バックアップ アーカイブが大きすぎるためインポートできません",
    exportTooLarge: "バックアップデータが大きすぎるためエクスポートできません",
    directoryImportFileNotFound:
      "インポートするバックアップ ファイルが見つかりません",
    directoryImportFileUnreadable:
      "インポートするバックアップ ファイルを読み取れません",
    directoryImportFileOnly:
      "バックアップディレクトリ内のファイルだけをインポートできます",
    directoryImportExtensionOnly:
      "{extension} バックアップファイルだけをインポートできます",
    directoryImportTooLarge:
      "バックアップ ファイルが大きすぎるため、FNOS ディレクトリからインポートできません。",
    archiveContentMissing: "バックアップ アーカイブ コンテンツが欠落しています",
    archiveBase64Invalid:
      "バックアップ アーカイブは有効な Base64 データではありません",
  },
  captcha: {
    powServerNotConfigured: "サーバー側で PoW CAPTCHA が設定されていません",
    providerMismatch: "CAPTCHA の種類が一致しません",
    turnstileNotConfigured:
      "Turnstile が設定されていません。管理者へ設定を依頼してください。",
    turnstileSecretMissing: "Cloudflare Turnstile の secret_key が未設定です",
    turnstileTokenRequired: "Turnstile トークンは必須です",
    turnstileServiceUnavailable:
      "Turnstile 検証サービスは一時的に利用できません",
    turnstileVerifyFailedWithReason: "Turnstile の検証に失敗しました: {reason}",
    turnstileVerifyFailed: "Turnstile の検証に失敗しました",
    providerUnavailable: "利用可能な CAPTCHA プロバイダーがありません",
    powNotEnabled: "PoW CAPTCHA が有効になっていません",
    powUnavailable: "PoW CAPTCHA を利用できません",
    providerConfigMismatch: "CAPTCHA プロバイダーが現在の設定と一致しません",
  },
  hmac: {
    missingTimestamp: "HMAC タイムスタンプがありません",
    missingNonce: "HMAC nonce がありません",
    missingSignature: "HMAC 署名がありません",
    timestampExpired: "HMAC タイムスタンプの有効期限が切れています",
    invalidKey: "HMAC キーが無効です",
    invalidSignature: "HMAC 署名が無効です",
    nonceReused: "HMAC nonce はすでに使用されています",
    nonceVerifyFailed: "HMAC nonce の検証に失敗しました",
  },
  cidr: {
    serviceError: "CIDR サービスエラー",
    emptyResponse: "<空の応答>",
    upstreamUrl: "アップストリーム URL: {url}",
    status: "ステータス: {status}{statusText}",
    contentType: "Content-Type: {contentType}",
    upstreamCode: "アップストリームコード: {code}",
    upstreamMessage: "アップストリームメッセージ: {message}",
    requestId: "リクエスト ID: {requestId}",
    responsePreview: "レスポンスのプレビュー: {preview}",
    provinceRequired: "都道府県は必須です",
    invalidApiUrl: "CIDR API URL が無効です: {error}",
    upstreamTimeout: "CIDR アップストリームリクエストのタイムアウト",
    upstreamRequestFailedGeneric:
      "CIDR アップストリームリクエストが失敗しました: {error}",
    upstreamRequestFailed:
      "CIDR アップストリームリクエストが失敗しました ({status})",
    invalidJson: "CIDR アップストリームが無効な JSON を返しました",
    upstreamUnexpected: "CIDR アップストリームから予期しない応答が返されました",
    provinceWideLabel: "{province} 全域",
    provinceWideUnsupported:
      "浙江省と広東省では省全体の CIDR を選択できません。都市を選択してください",
    operatorInvalid:
      "通信事業者は China Telecom、China Unicom、China Mobile のいずれかを指定してください",
    operatorUnsupported:
      "現在の CIDR サービスは通信事業者フィルターに対応していません。CIDR コンテナを 0.1.3 以降へ更新してください",
  },
  dashboard: {
    inbound: "受信",
    outbound: "送信",
    upstreamUnavailable: "アップストリームサービスを利用できません",
    hostRequired: "ホストは必須です",
    streamRequired: "有効な TCP または UDP マッピングを選択してください",
    statsLoadFailed: "ダッシュボード統計の読み込みに失敗しました",
    configLoadFailed: "ダッシュボード設定の読み込みに失敗しました",
    displayConfigSaveFailed: "ダッシュボード表示設定の保存に失敗しました",
  },
  acme: {
    alreadyInstalled: "acme.sh はインストール済みです",
    installInProgress: "インストールタスクが進行中です",
    installSubmitted: "インストールタスクを登録しました",
    issueSucceeded: "証明書を発行しました",
  },
  ddns: {
    ipv6OnlyUnavailable:
      "更新対象は IPv6 のみですが、使用可能な IPv6 アドレスを検出できませんでした",
    ipv4OnlyUnavailable:
      "更新対象は IPv4 のみですが、使用可能な IPv4 アドレスを検出できませんでした",
    dualStackUnavailable:
      "更新対象に使用可能な IPv4 または IPv6 アドレスがありません",
    domainConfigIncomplete: "ドメイン設定が不完全です",
    domainNotInZone: "ドメイン {fqdn} はルートゾーン {zone} に属していません",
    invalidJsonResponse: "レスポンスが有効な JSON ではありません: {text}",
    aRecordFailed: "A レコードの処理に失敗しました",
    aaaaRecordFailed: "AAAA レコード処理に失敗しました",
    providerDnsUpdateSuccess: "{provider} の DNS 更新に成功しました",
    aliyunParamKeyMissing:
      "Alibaba Cloud のリクエストパラメーターにキー名がありません",
    requestFailed: "リクエストに失敗しました",
    tencentMissingResponse:
      "HTTP {status}: Tencent Cloud API のレスポンスに Response がありません",
    invalidHeaderFormat: "無効なヘッダー形式: {header}",
    publicCheckSourceEmpty: "{family} のグローバル IP 検出元は空にできません",
    publicCheckSourceInvalidUrl:
      "{family} のグローバル IP 検出元が無効です: {source}",
    publicCheckSourceUnsupportedProtocol:
      "{family} のグローバル IP 検出元は HTTP/HTTPS のみ対応しています: {source}",
    publicCheckSourceListEmpty:
      "{family} のグローバル IP 検出元が設定されていません",
    publicCheckSourceRequestFailed:
      "検出元 {url} のリクエストに失敗しました: HTTP {status}",
    publicCheckSourceInvalidPayload:
      "検出元 {url} は有効な {family} アドレスを返しませんでした",
    publicCheckTestFailed: "グローバル IP 検出元のテストに失敗しました",
    publicDnsResolveFailed:
      "パブリック DNS で {host} の {family} アドレスを名前解決できませんでした: {detail}",
    publicDnsNoAddress:
      "パブリック DNS から {host} の {family} アドレスが返されませんでした",
    publicDnsNoUsableServer:
      "選択したインターフェースからパブリック DNS サーバーに接続できません",
    publicCheckTimeout: "グローバル IP 検出リクエストがタイムアウトしました",
    publicCheckTooManyRedirects:
      "グローバル IP 検出リクエストのリダイレクト回数が多すぎます",
    interfaceSourceLabel: "インターフェース {name}",
    selectedInterfaceSourceLabel: "選択したインターフェース",
    publicSourceLabel: "インターネット",
    staticSourceLabel: "静的 IP",
    domainSourceLabel: "ドメイン {domain}",
    domainSourceLabelEmpty: "取得元ドメイン",
    staticIpv4Invalid: "静的 IPv4 アドレスが無効です: {value}",
    staticIpv6Invalid: "静的 IPv6 アドレスが無効です: {value}",
    sourceDomainRequired: "名前解決する取得元ドメインを入力してください",
    sourceDomainInvalid: "取得元ドメインが無効です: {domain}",
    sourceDomainResolveFailed:
      "取得元ドメイン {domain} の名前解決に失敗しました: {error}",
    singleAddressProviderUnsupported:
      "{provider} では一度に 1 つのアドレスのみ更新できます。更新対象を IPv4 のみ、または IPv6 のみに設定してください",
    interfaceIpv6Unavailable:
      "IP の取得元はインターフェースですが、選択したインターフェースに使用可能な IPv6 アドレスがありません",
    interfaceIpv4Unavailable:
      "IP の取得元はインターフェースですが、選択したインターフェースに使用可能な IPv4 アドレスがありません",
    interfaceDualStackUnavailable:
      "IP の取得元はインターフェースですが、選択したインターフェースに使用可能な IPv4 または IPv6 アドレスがありません",
    publicIpv6Unavailable:
      "IP の取得元はインターネットですが、使用可能な IPv6 アドレスを取得できませんでした",
    publicIpv4Unavailable:
      "IP の取得元はインターネットですが、使用可能な IPv4 アドレスを取得できませんでした",
    publicDualStackUnavailable:
      "IP の取得元はインターネットですが、使用可能な IPv4 または IPv6 アドレスを取得できませんでした",
    staticIpv6Unavailable:
      "IP の取得元は静的 IP ですが、使用可能な IPv6 アドレスが入力されていません",
    staticIpv4Unavailable:
      "IP の取得元は静的 IP ですが、使用可能な IPv4 アドレスが入力されていません",
    staticDualStackUnavailable:
      "IP の取得元は静的 IP ですが、使用可能な IPv4 または IPv6 アドレスが入力されていません",
    domainIpv6Unavailable:
      "IP の取得元はドメインの名前解決ですが、使用可能な IPv6 アドレスを解決できませんでした",
    domainIpv4Unavailable:
      "IP の取得元はドメインの名前解決ですが、使用可能な IPv4 アドレスを解決できませんでした",
    domainDualStackUnavailable:
      "IP の取得元はドメインの名前解決ですが、使用可能な IPv4 または IPv6 アドレスを解決できませんでした",
    selectInterfaceAddress:
      "インターフェースから直接取得するには、{family} アドレスを選択してください",
    selectedInterfaceAddressUnavailable:
      "選択したインターフェースの {index} 番目の {family} アドレスは使用できなくなりました。選択し直してください",
    interfaceSelectorFamilyInvalid:
      "インターフェイスアドレス選択ルールのアドレスファミリが無効です",
    interfaceSelectorInvalid:
      "インターフェイスアドレス選択ルールが無効です: {message}",
    interfaceSelectorNoMatch:
      "インターフェイスアドレス選択ルールに使用可能な {family} アドレスが一致しませんでした",
    interfaceSelectorMultiple:
      "{family} 選択ルールは {count} 件に一致し、{address} を選択しました（{reason}）",
    interfaceSelectorResolved:
      "{family} アドレス選択: モード {mode}、{count} 件一致、{address} を選択（{reason}）",
    interfacePreferredRecoveryDeferred:
      "{family} の優先アドレス {preferred} の復旧を連続確認中です（{count}/{required}）。頻繁な切り替えを防ぐため、現在の {current} を一時的に維持します。",
    ipv4FailedContinueIpv6:
      "IPv4 の検出に失敗したため、IPv6 で続行します（{error}）",
    ipv4Failed: "IPv4 の検出に失敗しました（{error}）",
    ipv6FailedContinueIpv4:
      "IPv6 の検出に失敗したため、IPv4 で続行します（{error}）",
    ipv6Failed: "IPv6 の検出に失敗しました（{error}）",
    publicIpv6NotSelectable:
      "検出したグローバル IPv6 アドレス（{ip}）は、このマシンまたは Docker ホストで選択可能なインターフェースのアドレスに含まれていません。外部から到達できない場合は、インターフェースからの直接取得に切り替え、ホストのグローバル IPv6 アドレスを選択してください",
    interfaceRequired:
      "インターフェースから直接取得するには、送信インターフェースを選択してください",
    interfaceNotFound: "使用可能なインターフェースが見つかりません: {name}",
    dockerHostInterfaceLabel: "ホスト {name} ({summary})",
    curlStatusLineParseFailed: "CURL 応答ステータス行を解析できません: {line}",
    curlNoHeaders: "curl は応答ヘッダーを返しませんでした",
    requestCanceled: "リクエストがキャンセルされました",
    curlRequestFailed: "curl リクエストが失敗しました: {detail}",
    nodeTransportInterfaceAddressUnavailable:
      "組み込み HTTP リクエストをインターフェース {name} にバインドできません: 使用可能な {family} ローカルアドレスがありません",
    nodeTransportInterfaceNoAddress:
      "組み込み HTTP リクエストをインターフェース {name} にバインドできません: 使用可能なローカルアドレスがありません",
    nodeTransportUnsupportedProtocol:
      "組み込み HTTP リクエストはこのプロトコルに対応していません: {protocol}",
    nodeTransportRedirectLimitExceeded:
      "組み込み HTTP リクエストのリダイレクト回数が上限 {max} を超えました",
    triggerCron: "スケジュール実行",
    triggerEnable: "自動更新の有効化後に即時実行",
    triggerStartup: "起動時実行",
    triggerMessage: "{trigger}: {message}",
    notConfigured: "未設定",
    skippedNoProvider:
      "DDNS プロバイダーが選択されていないため、スキップされました",
    skippedIncompleteConfig: "現在の設定は不完全であるためスキップされました",
    skippedPublicIpUnavailable:
      "グローバル IP を取得できないため、スキップしました",
    skippedReason: "{reason}。スキップしました",
    targetIpNoChange: "更新対象の IP に変更がないため、更新は不要です",
    none: "なし",
    ipChange: "{family}: {before} -> {after}",
    targetIpChanged: "更新対象の IP 変更を検出しました: {changes}",
    dnsUpdateSuccess: "DNS 更新に成功しました [{provider}]: {message}",
    dnsUpdateFailed: "DNS 更新に失敗しました [{provider}]: {message}",
    taskError: "タスクでエラーが発生しました: {message}",
    intervalOutOfRange:
      "自動同期間隔には {min}～{max} 分の整数を指定してください",
    primaryDomainName: "メインドメイン",
    noProviderSelected: "プロバイダーが選択されていません",
    duplicateTarget:
      "同じプロバイダーとドメインの組み合わせを持つ DDNS エントリがすでに存在します",
    domainTargets: {
      invalidDomain: "FQDN が無効です: {domain}",
      tooMany: "FQDN は 2 件まで設定できます",
      invalidPair:
        "2 件の FQDN には、ワイルドカードドメインと対応するベースドメインを指定してください",
      mismatchedPair: "ワイルドカードドメインとベースドメインが一致しません",
      pairUnsupported:
        "{provider} はワイルドカードドメインとベースドメインの同時更新に対応していません",
      rootMissing:
        "ワイルドカードドメインとベースドメインを組み合わせる前に {field} を設定してください",
      rootMismatch:
        "ベースドメインが {field} の管理範囲外です（ゾーン: {expected}、指定値: {actual}）",
      allSucceeded: "{count} 件のドメイン",
      itemSucceeded: "{domain}: 成功",
      itemFailed: "{domain}: 失敗（{detail}）",
    },
    primaryInitFailed: "メインドメインの DDNS エントリを初期化できませんでした",
    primaryDomainScope: "メインドメイン",
    additionalDomainScope: "追加ドメイン",
    targetNotFound: "DDNS エントリが見つかりませんでした",
    unknownProvider: "不明な DDNS プロバイダーです: {provider}",
    primaryDeleteForbidden: "メインドメインのエントリは削除できません",
    primaryDisableForbidden:
      "メインドメインのエントリだけを無効にすることはできません",
    unknownProviderShort: "不明なプロバイダー: {provider}",
    selectProviderFirst: "先に DDNS プロバイダーを選択してください",
    primaryConfigIncomplete:
      "メインドメインの設定が不完全です。すべての必須項目を入力してください",
    targetConfigIncomplete:
      "このエントリの設定が不完全です。すべての必須項目を入力してください",
    manualTestStart:
      "手動テストを開始しました。現在の更新対象 IP を取得しています…",
    manualTestPrefix: "手動テスト",
    currentTargetIp:
      "現在の更新対象 IP（{source}）— IPv4: {ipv4}、IPv6: {ipv6}",
    testAborted: "{message}。テストを中止しました",
    updateSuccess: "更新に成功しました: {message}",
    updateFailed: "更新に失敗しました: {message}",
    testError: "テストでエラーが発生しました: {message}",
    statusLoadFailed: "DDNS の状態を読み込めませんでした",
    toggleFailed: "DDNS 有効状態の更新に失敗しました",
    settingsLoadFailed: "DDNS 自動同期設定の読み込みに失敗しました",
    settingsSaveFailed: "DDNS 自動同期設定の保存に失敗しました",
    logsLoadFailed: "DDNS ログの読み込みに失敗しました",
    logsClearFailed: "DDNS ログの消去に失敗しました",
    pollFailed: "DDNS のログと状態を取得できませんでした",
    providerSetFailed: "プロバイダーの設定に失敗しました",
    configSaveFailed: "DDNS の保存に失敗しました",
    createTargetFailed: "DDNS エントリの作成に失敗しました",
    updateTargetFailed: "DDNS エントリの更新に失敗しました",
    deleteTargetFailed: "DDNS エントリの削除に失敗しました",
    updateTargetEnabledFailed: "DDNS エントリの有効状態を更新できませんでした",
    providers: {
      common: {
        fields: {
          root_domain: {
            label: "ルートドメイン",
            description: "ゾーンの判定に使用します（例: example.com）",
          },
          domain: {
            label: "FQDN",
            shortLabel: "ドメイン",
            description: "更新する完全修飾ドメイン名",
            hostDescription: "更新する完全修飾ホスト名",
          },
          ttl: {
            description: "デフォルト {seconds} 秒",
          },
          access_key_id: {
            label: "アクセスキー ID",
            description:
              "DNS レコードの読み書き権限を持つクラウドプロバイダーのアクセスキー ID",
          },
          access_key_secret: {
            label: "アクセスキー Secret",
            description: "アクセスキー ID と組み合わせて使用する Secret",
          },
          secret_access_key: {
            label: "アクセスキー Secret",
            description: "アクセスキー ID と組み合わせて使用する Secret",
          },
          secret_id: {
            label: "SecretId",
            description:
              "選択した DNS サービス権限を持つ Tencent Cloud API SecretId",
          },
          secret_key: {
            label: "SecretKey",
            description:
              "SecretId と組み合わせて使用する Tencent Cloud API SecretKey",
          },
          api_key: {
            label: "API キー",
            description: "プロバイダーのコンソールで生成した API Key",
          },
          api_secret: {
            label: "API Secret",
            description: "API Key と組み合わせて使用する API Secret",
          },
          secret_api_key: {
            label: "Secret API Key",
            description: "Porkbun コンソールで生成した Secret API Key",
          },
          api_token: {
            label: "API トークン",
            description: "プロバイダーのコンソールで生成した API Token",
          },
          token_id: {
            label: "Token ID",
            description: "DNSPod コンソールで生成した API Token ID",
          },
          token_key: {
            label: "Token Key",
            description: "DNSPod コンソールで生成した API Token Key",
          },
          zone_id: {
            label: "Zone ID",
            description: "プロバイダーのコンソールにある Zone またはサイト ID",
          },
        },
      },
      dynv6: {
        fields: {
          token: {
            description: "dynv6.com アカウントで生成",
          },
          zone: {
            label: "ゾーン名",
            description: "dynv6 ゾーンのドメイン名",
          },
          ipv6prefix: {
            description: "任意。dynv6 API にそのまま渡されます",
          },
        },
        configIncomplete: "dynv6 設定が不完全です",
        empty: "（空）",
        success: "dynv6: {detail}（送信内容: {params}）",
        updateFailed: "dynv6 の更新に失敗しました [{status}]: {detail}",
        requestError: "dynv6 のリクエストでエラーが発生しました: {detail}",
      },
      duckdns: {
        fields: {
          domains: {
            label: "サブドメイン",
            description:
              ".duckdns.org を付けずに DuckDNS のサブドメインのみを入力してください。複数指定する場合はカンマで区切ります",
          },
          token: {
            description:
              "DuckDNS コンソールのホーム画面に表示されるアカウントトークン",
          },
        },
        configIncomplete: "DuckDNS 設定が不完全です",
        noIpAvailable:
          "DuckDNS 更新に失敗しました: IPv4 または IPv6 アドレスが利用できません",
        updateFailedWithStatus:
          "DuckDNS 更新に失敗しました [{status}]: {detail}",
        requestFailed: "リクエストに失敗しました",
        updateFailed: "DuckDNS 更新に失敗しました: {detail}",
        nonOkResponse: "OK 以外のレスポンスが返されました",
        success: "DuckDNS の更新に成功しました{detail}",
        requestError: "DuckDNS のリクエストでエラーが発生しました: {detail}",
      },
      dnspod: {
        fields: {
          record_line: {
            label: "回線",
            description: "指定しない場合は「デフォルト」回線を使用します",
          },
        },
        defaultLine: "デフォルト",
        configIncomplete: "DNSPod 設定が不完全です",
        queryRecordFailed: "レコードの照会に失敗しました",
        updateRecordFailed: "レコードの更新に失敗しました",
        createRecordFailed: "レコードの作成に失敗しました",
      },
      dnshe: {
        label: "DNSHE",
        fields: {
          api_key: {
            label: "API Key",
            description: "DNSHE API 管理で生成した API Key",
          },
          api_secret: {
            label: "API Secret",
            description:
              "DNSHE API Key と対になる API Secret です。安全に保管してください。",
          },
          root_domain: {
            label: "DNSHE 管理ドメイン",
            description:
              "DNSHE アカウントに登録された完全な無料ドメインです。例: example.com",
          },
          domain: {
            description:
              "更新する完全なドメインです。設定した DNSHE 管理ドメインに属している必要があります。",
          },
        },
        configIncomplete: "DNSHE の設定が不完全です",
        noIpAvailable:
          "DNSHE の更新に失敗しました: 使用可能な IPv4 または IPv6 アドレスがありません",
        managedDomainNotFound:
          "DNSHE アカウントに管理ドメインが見つかりません: {domain}",
        managedDomainInactive:
          "DNSHE 管理ドメインを使用できません: {domain}（状態: {status}）",
        unknownStatus: "不明",
        recordIdMissing: "DNSHE が返した {type} レコードに内部 ID がありません",
        apiError: "DNSHE API リクエストに失敗しました: {detail}",
        requestError: "DNSHE リクエストエラー: {detail}",
      },
      cloudflare: {
        fields: {
          api_token: {
            label: "API トークン",
            description: "Zone.DNS の編集権限が必要です",
          },
          zone_id: {
            description:
              "Cloudflare のドメイン画面で三点メニューを開き、「Zone ID をコピー」を選択します",
          },
          proxied: {
            label: "Cloudflare プロキシ",
            description: "Cloudflare プロキシ（オレンジ色の雲）を有効にします",
            options: {
              dnsOnly: "DNS のみ",
              orangeCloud: "プロキシ有効",
            },
          },
        },
        configIncomplete: "Cloudflare 設定が不完全です",
        zoneLookupFailed: "Cloudflare ゾーンの検索に失敗しました: {detail}",
        zoneMismatch:
          "ベースドメインが Cloudflare ゾーンの範囲外です（ゾーン: {expected}、指定値: {actual}）",
        searchRecordFailed: "{type} レコードの検索に失敗しました: {detail}",
        updateRecordFailed: "{type} レコードの更新に失敗しました: {detail}",
        createRecordFailed: "{type} レコードの作成に失敗しました: {detail}",
        recordOperationError:
          "{type} レコードの操作でエラーが発生しました: {detail}",
        success: "Cloudflare DNS の更新に成功しました",
      },
      godaddy: {
        configIncomplete: "GoDaddy の設定が不完全です",
        updateFailed: "更新に失敗しました",
        updateFailedWithStatus: "[{status}] {detail}",
      },
      porkbun: {
        configIncomplete: "Porkbun の設定が不完全です",
        queryRecordFailed: "レコードの照会に失敗しました",
        updateRecordFailed: "レコードの更新に失敗しました",
        createRecordFailed: "レコードの作成に失敗しました",
      },
      alidns: {
        label: "アリババクラウド DNS",
        fields: {
          access_key_secret: {
            placeholder: "Alibaba Cloud AccessKey シークレット",
          },
          line: {
            label: "回線",
            description:
              "指定しない場合は Alibaba Cloud の「default」回線を使用します",
          },
        },
        configIncomplete: "Alibaba Cloud DNS の設定が不完全です",
        requestFailed: "リクエストに失敗しました",
        updateFailed: "更新に失敗しました",
        createFailed: "作成に失敗しました",
        recordIdMissing:
          "Alibaba Cloud DNS が RecordId のないレコードを返しました",
      },
      baidu: {
        label: "百度クラウド DNS",
        fields: {
          access_key_id: {
            placeholder: "Baidu スマート クラウド アクセス キー",
          },
          secret_access_key: {
            placeholder: "Baidu スマートクラウド秘密キー",
          },
        },
        configIncomplete: "Baidu Cloud DNS の設定が不完全です",
        queryFailed: "照会に失敗しました",
        updateFailed: "更新に失敗しました",
        createFailed: "作成に失敗しました",
      },
      huawei: {
        label: "ファーウェイクラウド DNS",
        fields: {
          access_key_id: {
            placeholder: "ファーウェイクラウド AK",
          },
          secret_access_key: {
            placeholder: "ファーウェイクラウド SK",
          },
        },
        webCryptoUnsupported:
          "現在の実行環境は Web Crypto に対応していないため、Huawei Cloud AK/SK 署名を生成できません",
        configIncomplete: "Huawei Cloud DNS の設定が不完全です",
        requestFailed:
          "Huawei Cloud DNS リクエストが失敗しました: HTTP {status} {statusText}、{detail}",
        zoneNotFound: "Huawei Cloud のゾーンが見つかりません: {zone}",
        recordsetIdMissing:
          "Huawei Cloud DNS が ID のないレコードセットを返しました",
      },
      tencentcloud: {
        label: "Tencent Cloud DNS",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          record_line: {
            label: "回線",
            description: "指定しない場合は「デフォルト」回線を使用します",
          },
          record_line_id: {
            label: "回線 ID",
            description: "任意。指定した場合は回線 ID が優先されます",
          },
        },
        defaultLine: "デフォルト",
        configIncomplete: "Tencent Cloud DNS の設定が不完全です",
        missingUpdatedRecordId:
          "Tencent Cloud から更新後の RecordId が返されませんでした",
        missingCreatedRecordId:
          "Tencent Cloud から作成後の RecordId が返されませんでした",
      },
      noip: {
        fields: {
          hostname: {
            description:
              "完全修飾ホスト名を入力してください。複数指定する場合はカンマで区切ります",
          },
          username: {
            label: "ユーザー名",
            description: "NO-IP コンソールで生成した DDNS Key のユーザー名",
          },
          password: {
            label: "パスワード",
            description:
              "メインアカウントのパスワードではなく、DDNS Key と対になるパスワードを使用してください",
          },
        },
        statusMessages: {
          "911":
            "NO-IP サーバーで一時的な障害が発生しました。公式では、少なくとも 30 分後に再試行することを推奨しています。",
          nohost:
            "指定されたホスト名が存在しないか、現在の DDNS キーに属していません",
          badauth: "ユーザー名またはパスワードが間違っています",
          badagent:
            "NO-IP によってクライアントが無効化されています。User-Agent またはクライアントの状態を確認してください",
          "!donator": "現在のアカウントは要求された拡張機能に対応していません",
          abuse: "不正利用により、この DDNS Key は NO-IP にブロックされました",
        },
        unknownStatus: "不明なステータスが返されました: {code}",
        updateFailed: "NO-IP 更新に失敗しました: {detail}",
        updateSuccess: "NO-IP の更新に成功しました{detail}",
        ipUnchanged: "NO-IP の IP に変更はありません{detail}",
        configIncomplete: "NO-IP 設定が不完全です",
        noIpAvailable:
          "NO-IP の更新に失敗しました: 使用可能な IPv4 または IPv6 アドレスがありません",
        updateFailedWithStatus: "NO-IP 更新に失敗しました [{status}]: {detail}",
        requestFailed: "リクエストに失敗しました",
        emptyResponse:
          "NO-IP の更新に失敗しました: 空のレスポンスが返されました",
        requestError: "NO-IP のリクエストでエラーが発生しました: {detail}",
      },
      esa: {
        label: "アリババクラウド ESA DNS",
        fields: {
          access_key_secret: {
            placeholder: "Alibaba Cloud AccessKey シークレット",
          },
          site_name: {
            label: "サイト名",
            description:
              "ESA のサイト名（通常はルートドメイン）。Site ID を指定した場合は、フォールバック検索にのみ使用します",
          },
          site_id: {
            description:
              "任意。指定すると、サイト一覧を検索せずに対象サイトを直接操作します",
          },
          proxied: {
            label: "ESA プロキシ",
            description:
              "デフォルトは DNS のみです。プロキシを有効にすると、ビジネスタイプが自動的に送信されます",
            options: {
              dnsOnly: "DNS のみ",
              enabled: "プロキシ有効",
            },
          },
          biz_name: {
            label: "業種",
            description:
              "ESA プロキシが有効な場合のみ適用されます。デフォルトは Web です",
            options: {
              web: "ウェブサイト",
              api: "API",
              imageVideo: "音声・動画",
            },
          },
        },
        configIncomplete: "Alibaba Cloud ESA DNS の設定が不完全です",
        siteNameMissing: "Alibaba Cloud ESA DNS のサイト名がありません",
        siteLookupFailed:
          "Alibaba Cloud ESA サイトの検索に失敗しました: {detail}",
        siteMismatch:
          "設定された Site ID がサイトの検索結果と一致しません（設定値: {expected}、検索結果: {actual}）",
        siteNotFound: "ESA サイトが見つかりません: {site}",
        noIpAvailable:
          "Alibaba Cloud ESA DNS には更新可能な IP アドレスがありません",
        createRecordFailed: "CreateFailed: レコードを作成できませんでした",
        success: "Alibaba Cloud ESA DNS の更新に成功しました",
        recordIdMissing: "UpdateFailed: レコードに RecordId がありません",
      },
      dynu: {
        fields: {
          api_key: {
            description: "Dynu の API Credentials で生成した API-Key",
          },
          domain: {
            description:
              "更新する Dynu の完全修飾ホスト名。ワイルドカードドメインとベースドメインを組み合わせる場合、ベースドメインは別サービス配下の通常のサブドメインではなく、Dynu に独立した DDNS Service として登録されている必要があります。更新時はベースドメイン用のレコードを個別に作成せず、IP を設定して Wildcard Alias を有効にします",
          },
          group: {
            description: "任意。Dynu DNS レコードに書き込むグループ",
          },
        },
        actionFailed: "{action}に失敗しました",
        actions: {
          resolveRoot: "Dynu ルートドメインの名前解決",
          readDnsService: "Dynu DNS サービスの読み込み",
          updateWildcardAlias: "Dynu ワイルドカード エイリアスの更新",
          queryRecord: "Dynu {type} レコードの照会",
          updateRecord: "Dynu {type} レコードの更新",
          createRecord: "Dynu {type} レコードの作成",
        },
        invalidRootInfo:
          "Dynu から有効なルートドメイン情報が返されませんでした",
        wildcardUnsupported:
          "Dynu REST は *.{domain} を DNS レコードの nodeName に指定できません。Dynu DDNS Services で {domain} を独立したサービスとして追加して Wildcard Alias を有効にするか、DDNS 設定を {domain} に変更してください",
        wildcardUnchanged: "Dynu Wildcard Alias の IP に変更はありません",
        wildcardSuccess: "Dynu Wildcard Alias の更新に成功しました",
        configIncomplete: "Dynu 設定が不完全です",
        noIpAvailable:
          "Dynu の更新に失敗しました: 使用可能な IPv4 または IPv6 アドレスがありません",
        recordIdMissing:
          "Dynu から返された DNS レコードには RecordId がありません",
        requestError: "Dynu のリクエストでエラーが発生しました: {detail}",
      },
      edgeone: {
        label: "Tencent Cloud EdgeOne",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          zone_id: {
            description: "ホストゾーンの特定に使用する EdgeOne のサイト ID",
          },
          domain: {
            description:
              "更新する完全修飾ホスト名。国際化ドメイン名は先に Punycode へ変換してください",
          },
          location: {
            label: "回線",
            placeholder: "デフォルトまたは CN.BJ",
            description:
              "任意。空欄の場合はデフォルトのグローバル回線を使用します",
          },
          ttl: {
            description:
              "デフォルトは 300 秒、EdgeOne では 60 ～ 86400 が許可されます",
          },
          overseas_access: {
            label: "海外アクセス制御",
            description:
              "有効にすると、EdgeOne Security Policy API で海外 IP からのアクセスをブロックします。香港・マカオ・台湾は海外として扱いません。この設定は変更時に一度だけ同期され、DDNS の更新時には再同期されません",
            options: {
              off: "オフ",
              blockOverseas: "海外 IP をブロック",
            },
          },
          endpoint: {
            description:
              "デフォルトは中国本土向けエンドポイントです。https://teo.intl.tencentcloudapi.com または地域別エンドポイントも指定できます",
          },
          region: {
            placeholder: "空欄",
            description: "任意。通常は空欄のままで使用できます",
          },
        },
        configIncomplete: "Tencent Cloud EdgeOne の設定が不完全です",
        zoneLookupFailed: "EdgeOne サイトの検索に失敗しました: {detail}",
        zoneMismatch:
          "ベースドメインが EdgeOne ゾーンの範囲外です（ゾーン: {expected}、指定値: {actual}）",
        configTargetIncomplete:
          "Tencent Cloud EdgeOne の設定が不完全です。Zone ID またはドメインがありません",
        missingRecordId: "EdgeOne から返されたレコードに RecordId がありません",
        missingCreatedRecordId:
          "EdgeOne から作成後の RecordId が返されませんでした",
        overseasAccess: {
          describeRulesFailed:
            "EdgeOne 海外アクセス制御で既存のカスタムルールを読み込めませんでした（provider_target={target}、zone_id={zoneId}、endpoint_host={endpointHost}、region={region}、entity={entity}、scope={scope}）: {message}",
          syncFailedWithAttempt:
            "EdgeOne 海外アクセス制御の同期に失敗しました（{attempt}、submitted_rule_count={count}）: {message}",
          syncAllScopesFailed:
            "EdgeOne 海外アクセス制御の同期に失敗しました: すべてのルール スコープの試行が失敗しました",
          cleanupAllScopesFailed:
            "EdgeOne 海外アクセス制御のクリーンアップに失敗しました: すべてのルール スコープの試行が失敗しました",
          syncSuccess:
            "EdgeOne の海外 IP ブロックポリシーを同期しました。中国本土・香港・マカオ・台湾からのアクセスのみ許可します",
          cleanupSuccess: "EdgeOne の海外 IP ブロックポリシーを削除しました",
        },
      },
      edgeone_cname: {
        label: "Tencent Cloud EdgeOne（CNAME 接続）",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          zone_id: {
            description:
              "高速化ドメインが属するサイトの特定に使用する EdgeOne サイト ID",
          },
          domain: {
            label: "高速化ドメイン",
            description:
              "EdgeOne で作成済みの高速化ドメイン。IP_DOMAIN タイプのオリジンのみ対応し、一度に更新できるオリジンアドレスは 1 件です",
          },
          overseas_access: {
            label: "海外アクセス制御",
            description:
              "有効にすると、EdgeOne Security Policy API で海外 IP からのアクセスをブロックします。香港・マカオ・台湾は海外として扱いません。この設定は変更時に一度だけ同期され、DDNS の更新時には再同期されません",
            options: {
              off: "オフ",
              blockOverseas: "海外 IP をブロック",
            },
          },
          endpoint: {
            description:
              "デフォルトは中国本土向けエンドポイントです。https://teo.intl.tencentcloudapi.com または地域別エンドポイントも指定できます",
          },
          region: {
            placeholder: "空欄",
            description: "任意。通常は空欄のままで使用できます",
          },
        },
        configIncomplete:
          "Tencent Cloud EdgeOne（CNAME 接続）の設定が不完全です",
        singleAddressOnly:
          "Tencent Cloud EdgeOne（CNAME 接続）では、一度に 1 つのオリジンアドレスのみ更新できます。DDNS の更新対象を「IPv4 のみ」または「IPv6 のみ」に設定してください",
        noIpAvailable:
          "Tencent Cloud EdgeOne（CNAME 接続）に更新可能な IP アドレスがありません",
        domainNotFound: "EdgeOne の高速化ドメインが見つかりません: {domain}",
        unsupportedOriginType:
          "現在の高速化ドメインのオリジンタイプは {originType} です。DDNS で更新できるのは IP_DOMAIN タイプの高速化ドメインのみです",
        originUnchanged:
          "Tencent Cloud EdgeOne（CNAME 接続）のオリジンはすでに最新です",
        successWithInvalidHostHeaderIgnored:
          "Tencent Cloud EdgeOne（CNAME 接続）のオリジンを更新しました（無効な Host ヘッダーは無視されました）",
        success: "Tencent Cloud EdgeOne（CNAME 接続）のオリジンを更新しました",
      },
    },
  },
  smartConnect: {
    runTypes: {
      direct: "ダイレクト接続モード",
      reverseProxy: "リバースプロキシモード",
      subdomain: "サブドメインモード",
    },
    currentMode: "現在のモード",
    unavailableReason:
      "この機能はサブドメインモードでのみ利用できます。現在のモード: {mode}",
    selectLocalIp: "LAN IP を選択してください",
    selectValidLocalIpv4: "有効なローカル LAN IPv4 アドレスを選択してください",
    dnsmasqNotInstalled:
      "dnsmasq が見つかりません。先にインストールしてください",
    dnsmasqNotInitialized:
      "dnsmasq の初期化が完了していません。先に環境を初期化してください",
    syncFailed: "Smart Connect の同期に失敗しました",
  },
  scanDiscovery: {
    localIpv4CidrOnly:
      "スキャン範囲にはローカル IPv4 CIDR のみ指定できます: {cidrs}",
    maxCidrsExceeded: "一度に選択できるスキャン範囲は {max} 件までです",
    maxHostsExceededWithCurrent:
      "一度にスキャンできるホストは {max} 台までです（現在の選択: {current} 台）",
    maxHostsExceeded: "一度にスキャンできるホストは {max} 台までです",
    selectAtLeastOneCidr:
      "ローカル IPv4 のスキャン範囲を 1 件以上選択してください",
    scanJobNotFound: "スキャンジョブが見つからないか、期限切れです",
    loadTargetsFailed: "スキャン対象の読み込みに失敗しました",
    loadConfigFailed: "設定の読み込みに失敗しました",
    saveTargetsFailed: "スキャン対象の保存に失敗しました",
    loadSettingsFailed: "検出設定の読み込みに失敗しました",
    saveSettingsFailed: "検出設定の保存に失敗しました",
    invalidIntensityMode: "スキャン強度モードが無効です",
    invalidIntensityLevel: "スキャン強度レベルが無効です",
    targetLabels: {
      docker: "{cidr}（Docker ホスト LAN）",
      loopback: "{cidr}（ローカルループバック）",
      interface: "{cidr}({name})",
      mapping: "{cidr}（既存のマッピング先）",
      custom: "{cidr}（カスタム）",
      saved: "{cidr}（保存済み）",
    },
    serviceLabels: {
      lottery: "宝くじアシスタント",
      dlymusic: "Daoliyu Music Manager",
      kuake: "Quark 自動転送",
      xunlei: "Xunlei",
      nowen: "Nebula Portal",
      fnos: "FNOS",
      fnys: "FNOS Video",
      xiaoyaAlist: "Xiaoya Alist",
    },
  },
  gatewayProxyHeaders: {
    runTypes: {
      direct: "ダイレクト接続モード",
      reverseProxy: "リバースプロキシモード",
      subdomain: "サブドメインモード",
    },
    unavailableReason:
      "この機能はサブドメインモードでのみ利用できます。現在のモード: {mode}",
    syncFailed: "ゲートウェイのプロキシヘッダー設定を同期できませんでした",
  },
  sshSecurity: {
    logSourceUnavailable:
      "このシステムに journalctl または /var/log/auth.log が見つかりません",
    openWrtUnsupported:
      "OpenWrt ビルドは SSH セキュリティにまだ対応していません",
    enableUnavailable: "この環境では SSH セキュリティを有効にできません",
    syncFirewallUnavailable:
      "この環境では SSH ファイアウォールを同期できません",
    clearFirewallUnavailable:
      "この環境では SSH ファイアウォールを消去できません",
    logSourceUnavailableShort: "SSH ログの取得元を利用できません",
    customCidrInvalid: "カスタム CIDR の形式が無効です: {cidrs}",
    customCidrsMustBeArray: "custom_cidrs は配列である必要があります",
    syncSshPolicyFailed: "SSH 専用ファイアウォールルールの同期に失敗しました",
    clearSshPolicyFailed: "SSH 専用ファイアウォールルールの消去に失敗しました",
    blockRecordInvalid: "ブロックレコード形式が正しくありません",
    routes: {
      loadConfigFailed: "SSH セキュリティ設定の読み込みに失敗しました",
      updateConfigFailed: "SSH セキュリティ設定の更新に失敗しました",
      syncFirewallSuccess:
        "許可 CIDR {allowedCidrs} 件と SSH ブロック IP {synced} 件をポート {ports} に同期しました",
      syncFirewallFailed: "SSH ファイアウォールの同期に失敗しました",
      clearFirewallSuccess: "SSH 専用ファイアウォールルールを消去しました",
      clearFirewallFailed: "SSH ファイアウォールの消去に失敗しました",
      readLoginLogsFailed: "SSH ログインログの読み取りに失敗しました",
      listBlocksFailed: "SSH ブロック一覧の取得に失敗しました",
      blockNotFound: "ブロック記録が見つかりません",
      loadBlockFailed: "SSH ブロック記録の取得に失敗しました",
      removeBlockFailed: "ブロックを解除できませんでした",
      selectIps: "ブロックを解除するには IP を選択してください",
      removeBlocksFailed: "一括ブロック解除に失敗しました",
    },
  },
  systemEvents: {
    routes: {
      unsupportedSystemEventType:
        "サポートされていないシステムイベントタイプです",
      unsupportedSystemEventSource:
        "サポートされていないシステムイベントソースです",
      unsupportedSystemEventLevel:
        "サポートされていないシステムイベントレベルです",
      unsupportedSubjectKind: "サポートされていないイベント主体タイプです",
      unsupportedEventType: "サポートされていないイベントタイプです",
      unsupportedEventLevel: "サポートされていないイベントレベルです",
      unsupportedEventSource: "サポートされていないイベントソースです",
      loadConfigFailed: "システムイベント設定の読み込みに失敗しました",
      writeEventFailed: "システムイベントの書き込みに失敗しました",
      listEventsFailed: "システムイベント一覧の取得に失敗しました",
      deleteEventsFailed: "システムイベントの削除に失敗しました",
      clearEventsFailed: "システムイベントの全削除に失敗しました",
    },
  },
  notifications: {
    brand: {
      prefix: "Knock ",
      defaultTitle: "Knock 通知",
    },
    templates: {
      events: {
        authLoginSuccess: "ログイン成功",
        authLogout: "ログアウト",
        authLoginFailure: "ログイン失敗",
        authSessionIpDrift: "セッション IP の変化",
        securityScannerBlocked: "スキャナーをブロック",
        ddnsUpdateCompleted: "DDNS 更新",
        wolWakeCompleted: "Wake-on-LAN 完了",
        wolShutdownCompleted: "SSH リモートシャットダウン完了",
        gatewayThrottleBlocked: "ゲートウェイのレート制限",
        gatewayVisibilityBlocked: "ゲートウェイ公開範囲によるブロック",
        wafBlocked: "WAF がブロック",
        sshLoginSuccess: "SSH ログイン成功",
        sshLoginFailure: "SSH ログイン失敗",
        sshIpBlocked: "SSH で IP をブロック",
        appUpdateAvailable: "アプリの更新が利用可能",
        cpuAlert: "CPU 使用率アラート",
        cpuRecovered: "CPU 使用率が復旧",
        memoryAlert: "メモリ使用率アラート",
        memoryRecovered: "メモリ使用率が復旧",
        frpConnected: "FRP 接続",
        frpDisconnected: "FRP 切断",
        cloudflaredConnected: "Cloudflared 接続",
        cloudflaredDisconnected: "Cloudflared 切断",
        runtimeStarted: "コンポーネント起動",
        runtimeStopped: "コンポーネント停止",
        runtimeRestarted: "コンポーネント再起動",
        runtimeHealthFailed: "ヘルスチェック失敗",
        runtimeRecovered: "コンポーネント復旧",
        runtimeAbnormalExit: "コンポーネント異常終了",
        panelSyncFailed: "ナビゲーションパネルへの同期失敗",
        panelSyncRecovered: "ナビゲーションパネルへの同期復旧",
        terminalAudit: "ターミナル監査",
      },
      ruleName: "{event}通知",
      levels: {
        info: "お知らせ",
        warn: "注意",
        error: "エラー",
        critical: "重大",
      },
      sources: {
        serverAdmin: "管理バックエンド",
        goReauthProxy: "認証プロキシ",
        systemMonitor: "システム監視",
        runtimeMonitor: "ランタイム監視",
      },
      authMethods: {
        oidc: "外部アカウント",
        ldap: "ディレクトリアカウント",
      },
      grantTypes: {
        browserSession: "ブラウザセッション",
        loginIpGrant: "ログイン IP の許可",
      },
      wafModes: {
        detection: "検出",
        blocking: "ブロック",
        off: "オフ",
      },
      wafActions: {
        block: "ブロック",
        deny: "拒否",
        detect: "検出",
        log: "記録",
        pass: "許可",
      },
      logoutSources: {
        userLogout: "ユーザーがログアウト",
        adminSessionDelete: "管理者がセッションを終了",
      },
      driftSources: {
        proxySession: "プロキシセッション",
        fnosToken: "FNOS トークン",
        sessionRefresh: "セッション更新",
        browserSession: "ブラウザセッション",
      },
      ddnsTriggers: {
        cron: "スケジュールされたタスク",
        enable: "有効化後の最初の実行",
        startup: "起動時チェック",
        manualTest: "手動テスト",
      },
      ddnsUpdateScopes: {
        ipv4Only: "IPv4 のみ",
        ipv6Only: "IPv6 のみ",
      },
      ddnsIpSources: {
        public: "グローバル IP の検出",
        interface: "インターフェースから取得",
        static: "静的 IP",
        domain: "ドメイン名解決",
      },
      updateCheckReasons: {
        cron: "スケジュール確認",
        manual: "手動確認",
        manualCheckAndDownload: "手動で確認してダウンロード",
        downloadBootstrap: "ダウンロード前の確認",
      },
      terminalActions: {
        targetCreated: "SSH ターゲットを作成",
        targetUpdated: "SSH ターゲットを更新",
        targetDeleted: "SSH ターゲットを削除",
        hostKeyConfirmed: "ホストフィンガープリントを確認",
        connectionTestSucceeded: "SSH 接続テストに成功",
        connectionTestFailed: "SSH 接続テストに失敗",
        localTerminalEnabled: "ローカルターミナルを有効化",
        localTerminalDisabled: "ローカルターミナルを無効化",
        sessionCreationStarted: "ターミナルセッションの作成を開始",
        sessionCreationFailed: "ターミナルセッションの作成に失敗",
        sessionEnded: "ターミナルセッションを終了",
        sessionExited: "Shell が終了",
        sessionLost: "ターミナルセッションの接続が切断",
      },
      credential: "認証情報",
      unknownCredential: "不明な認証情報",
      credentialLinkedTotp:
        "{authMethod}「{credential}」に TOTP「{totp}」を紐付け",
      credentialName: "認証情報「{credential}」",
      sessionCommentCompact: "備考: {comment}",
      appendSessionComment: "{text}（備考: {comment}）",
      yes: "はい",
      no: "いいえ",
      wafOutcomeBlocked: "ブロック",
      wafOutcomeLogged: "記録",
      sections: {
        overview: "イベント概要",
        aggregation: "集計",
        advice: "推奨対応",
      },
      aggregationText:
        "この通知には、{seconds} 秒間に発生した同様のイベント {count} 件が集約されています",
      details: {
        units: {
          seconds: "{count} 秒",
          minutes: "{count} 分",
          times: "{count} 回",
          ratePerSecond: "{count} 回/秒",
        },
        listSeparator: "、",
        unknown: "不明",
        unknownIp: "不明 IP",
        unknownMethod: "不明な方式",
        unknownProvider: "不明なプロバイダー",
        unknownUser: "不明なユーザー",
        unknownHost: "不明なホスト",
        currentSession: "現在のセッション",
        memoryMetric: "メモリ",
        connected: "接続",
        disconnected: "切断",
        parenthesized: "（{value}）",
        sessionCommentSentence: "現在のセッションの備考:「{comment}」",
        aggregationStatsValue: "{count} 件 / {seconds} 秒間",
        facts: {
          credentialName: "認証情報名",
          linkedTotp: "紐付け済み TOTP",
          sessionComment: "セッションの備考",
          loginIp: "ログイン IP",
          ipLocation: "IP の所在地",
          authMethod: "認証方式",
          loginProvider: "ログインプロバイダー",
          grantType: "認可方法",
          rememberLogin: "ログイン状態を保持",
          sessionExpiresAt: "セッションの有効期限",
          sessionId: "セッション ID",
          logoutSource: "ログアウト元",
          loginTime: "ログイン時間",
          sourceIp: "ソース IP",
          failureAttempts: "失敗回数",
          retryWait: "再試行までの待機時間",
          limitUntil: "制限解除時刻",
          originalIp: "変更前の IP",
          originalLocation: "変更前の所在地",
          currentIp: "現在の IP",
          currentLocation: "現在の所在地",
          driftSource: "変化の検出元",
          hitCount: "ヒット数",
          observationWindow: "監視期間",
          triggerThreshold: "トリガー閾値",
          blockedAt: "ブロック時間",
          recentPaths: "最近のパス",
          target: "更新対象",
          provider: "プロバイダー",
          targetType: "対象の種類",
          trigger: "実行方法",
          updateScope: "更新対象",
          ipSource: "IP の取得元",
          ipv4Change: "IPv4 変更",
          ipv6Change: "IPv6 変更",
          result: "実行結果",
          blockDuration: "ブロック期間",
          blockedUntil: "ブロック解除時刻",
          rateLimit: "レート制限",
          burstCapacity: "バースト容量",
          targetHost: "対象ホスト",
          requestMethod: "リクエストメソッド",
          requestScheme: "リクエストスキーム",
          requestPath: "リクエストパス",
          routeType: "ルート種別",
          routeKey: "ルート識別子",
          visibilityScope: "公開範囲",
          visibilityMode: "公開モード",
          authRoute: "認証ルート",
          requestAddress: "リクエストアドレス",
          outcome: "処理結果",
          wafAction: "WAF アクション",
          wafMode: "WAF モード",
          ruleIds: "ルール ID",
          ruleBundle: "ルールパック",
          statusCode: "ステータスコード",
          user: "ユーザー",
          port: "ポート",
          logTime: "ログ記録時刻",
          invalidUser: "無効なユーザー",
          threshold: "しきい値",
          window: "ウィンドウ",
          blockedReason: "ブロックの理由",
          relatedUser: "関連ユーザー",
          currentVersion: "現在のバージョン",
          latestVersion: "最新バージョン",
          checkReason: "確認方法",
          forceUpdate: "強制更新",
          releaseNotes: "リリースノート",
          hostname: "ホスト名",
          currentUsage: "現在の使用率",
          alertThreshold: "アラート閾値",
          recoverThreshold: "復旧閾値",
          sampleInterval: "サンプリング間隔",
          sustainDuration: "継続時間",
          tunnelType: "トンネルタイプ",
          connectionStatus: "接続状態",
          processPid: "プロセス PID",
          runtimeFeedback: "実行時メッセージ",
          terminalAction: "ターミナル操作",
          terminalTarget: "SSH ターゲット",
          terminalSession: "SSH セッション",
          terminalRevision: "ターゲットリビジョン",
          errorCode: "エラーコード",
          eventType: "イベントタイプ",
          riskLevel: "リスクレベル",
          eventSource: "イベントソース",
          happenedAt: "発生時刻",
          aggregationStats: "集計統計",
        },
        authLoginSuccess: {
          loginViaProvider: "{provider} からログイン",
          loginWithMethod: "{method} を使用",
          authViaProvider: "{provider} を経由",
          authWithMethod: "{method} を使用",
          summaryOidc:
            "{credential} が {method} で認証成功（IP: {ip}）{totpPart}",
          linkedTotpPart: "、TOTP「{totp}」を紐付け済み",
          summaryTotp:
            "{method}「{credential}」（紐付け済み TOTP「{totp}」）が {ip} からログインしました",
          summaryCredential:
            "認証情報「{credential}」が {ip} からログインしました",
          overview:
            "今回のログインは {auth} で認証されました。許可方式: {grantType}{locationPart}。{commentPart}",
          locationPart: "、所在地: {location}",
          advice:
            "心当たりがない場合は、すぐにセッションを無効化し、アクセスポリシーを確認してください",
        },
        authLogout: {
          summaryTotp:
            "{method}「{credential}」（紐付け済み TOTP「{totp}」）がログアウトしました",
          summaryCredential: "認証情報「{credential}」がログアウトしました",
          overview:
            "{ip}{locationPart} からのセッションがログアウトしました。ログアウト元: {source}。{commentPart}",
          advice:
            "予期しないログアウトの場合は、管理者によるセッション終了や異常なクリーンアップが発生していないか確認してください",
        },
        authLoginFailure: {
          summary: "{ip} からのログイン失敗が {attempts} 回に達しました",
          overview:
            "ログイン認証の連続失敗を検出しました。送信元 IP: {ip}{retryPart}{blockedPart}",
          retryPart: "、{seconds} 秒後に再試行可能",
          blockedPart: "、{time} まで制限",
          advice:
            "心当たりがない場合は認証情報の安全性をすぐに確認し、送信元 IP のブロックやログイン保護の強化を検討してください",
        },
        authSessionIpDrift: {
          summary: "{session} の IP が {fromIp} から {toIp} に変化しました",
          overview:
            "{session} のアクセス元 IP が変化しました。検出元: {source}。{commentPart}通常はネットワークの切り替え、プロキシの変更、またはセッションの異常が原因です",
          advice:
            "予期しない IP の変化の場合は、現在のセッションが乗っ取られていないか、すぐに確認してください",
        },
        securityScannerBlocked: {
          summary: "{ip} をスキャン行為のためブロックしました",
          overview:
            "この送信元では {minutes} 分間に {hits} 回のスキャンを検出し、閾値 {threshold} 回を超えました{pathsPart}",
          pathsPart: "。最近一致したパス: {paths}",
          advice:
            "ゲートウェイログで悪意のある探索か確認してください。誤検知の場合はスキャン閾値を調整できます",
        },
        ddnsUpdateCompleted: {
          defaultTarget: "DDNS の更新対象",
          summarySuccess: "{target} の DDNS 更新に成功しました",
          summaryFailure: "{target} の DDNS 更新に失敗しました",
          currentTask: "今回のタスク",
          overview:
            "{trigger}で DDNS 更新を実行しました。更新対象: {scope}、IP の取得元: {ipSource}。{resultPart}",
          resultPart: "結果: {message}",
          adviceSuccess:
            "DNS の変更がまだ反映されていない場合は、キャッシュの更新を待ってから外部アクセスを再確認してください",
          adviceFailure:
            "プロバイダーの認証情報、DNS レコード設定、グローバル IP の検出状態を確認してください",
          primaryDomain: "メインドメイン",
          additionalDomain: "追加ドメイン",
        },
        gatewayThrottleBlocked: {
          summary: "{ip} をリクエスト過多のため {seconds} 秒間ブロックしました",
          overview:
            "この送信元がゲートウェイのレート制限に達しました。上限: {rate} 回/秒、バースト容量: {burst}{targetPart}",
          targetPart: "、対象リクエスト: {target}",
          advice:
            "アクセスログで、急激なトラフィック、誤検知、悪意のあるリクエストのいずれかを確認し、必要に応じてレート制限を調整してください",
        },
        gatewayVisibilityBlocked: {
          summary:
            "{ip} から {host} へのアクセスが公開範囲ルールでブロックされました",
          overview:
            "{ip} から {host}{pathPart}{methodPart} へのアクセスが公開範囲ポリシーでブロックされました。有効範囲は{scope}、モードは{mode}です。",
          pathPart: " の {path}",
          methodPart: "（{method}）",
          scopeGateway: "ゲートウェイ全体",
          scopeHost: "このホスト",
          modeInherit: "全体設定を継承",
          modeCustom: "カスタム",
          advice:
            "この送信元を許可すべきか確認してください。想定外のブロックの場合は、ゲートウェイまたはホストの地域・CIDR 公開範囲設定を確認してください。",
        },
        wafBlocked: {
          summary: "{ip} のリクエストを WAF が{outcome}しました",
          overview:
            "WAF が送信元 {ip}{hostPart}{pathPart} を{outcome}しました{actionPart}{modePart}。{rulesPart}",
          hostPart: "（アクセス先: {host}）",
          pathPart: " {path}",
          actionPart: "、アクション: {action}",
          modePart: "、現在のモード: {mode}",
          rulesPart: "一致したルール: {rules}",
          adviceBlocked:
            "WAF ログの Trace ID から一致内容を確認してください。誤検知の場合はプロジェクトのメンテナーへ報告してください",
          adviceLogged:
            "WAF ログの Trace ID から一致内容を確認し、ルールとリクエストの内容に基づいてポリシーの調整が必要か判断してください",
        },
        sshLoginSuccess: {
          summary: "SSH ユーザー「{username}」が {ip} からログインしました",
          overview:
            "{ip}{locationPart}{authPart} からの SSH ログインに成功しました",
          authPart: "、認証方式: {authMethod}",
          advice:
            "心当たりがない場合は、SSH アカウント、鍵、送信元アクセスポリシーを確認してください",
        },
        sshLoginFailure: {
          summary:
            "SSH ユーザー「{username}」が {ip} からのログインに失敗しました",
          overview:
            "この送信元では {minutes} 分間に SSH ログインが {attempts}/{threshold} 回失敗しました{locationPart}",
          locationPart: "、所在地: {location}",
          advice:
            "失敗回数がブロック閾値に近づいていないか確認し、必要に応じて SSH の公開範囲を狭めるか認証情報を見直してください",
        },
        sshIpBlocked: {
          reasonCidrNotAllowed: "許可された地域範囲外",
          reasonFailedThreshold: "失敗数がしきい値に達しました",
          summary: "{ip} を SSH セキュリティがブロックしました",
          overview:
            "SSH セキュリティが送信元 {ip}{locationPart} をブロックしました。理由: {reason}",
          advice:
            "送信元が信頼できるか確認してください。誤ってブロックされた場合は、SSH セキュリティのブロック一覧から解除できます",
        },
        appUpdateAvailable: {
          currentVersionUnknown: "現在のバージョン不明",
          targetVersionUnknown: "更新先バージョン不明",
          summary: "新しいバージョン {version} が利用可能です",
          currentCheck: "今回の確認",
          overview:
            "{reason}により、fn-knock を {localVersion} から {latestVersion} に更新できることを確認しました{forcePart}",
          forcePart: "。早めに更新してください",
          releaseNotesAdvice: "リリースノート: {releaseNotes}",
          advice:
            "適切なメンテナンス時間帯に更新し、インストール前に現在の設定とサービス状態を確認してください",
        },
        systemMetric: {
          recoveredSummary:
            "{hostname} {metric} 使用率が {usage}% に戻りました",
          alertSummary: "{hostname} {metric} 使用率が {usage}% に増加しました",
          recoveredOverview:
            "{hostname} の {metric} 使用率は {usage}% まで低下しました。復旧閾値: {recover}%、直前のアラート閾値: {threshold}%",
          alertOverview:
            "{hostname} の {metric} 使用率は現在 {usage}% で、アラート閾値 {threshold}% を超えています。復旧閾値: {recover}%",
          recoveredAdvice:
            "リソース使用率は安全な範囲に戻りました。再び変動しないか、引き続き監視してください",
          alertAdvice:
            "リソースの逼迫が続かないよう、高負荷のプロセス、バックグラウンドタスク、外部トラフィックの変化を確認してください",
        },
        tunnel: {
          connectedSummary: "{tunnel} が接続されました",
          disconnectedSummary: "{tunnel} が切断されました",
          connectedOverview:
            "{tunnel} のトンネル接続が復旧しました{messagePart}",
          connectedMessagePart: "。実行時メッセージ: {message}",
          disconnectedOverview:
            "{tunnel} のトンネル接続が切断されました{messagePart}",
          disconnectedMessagePart: "。現在のメッセージ: {message}",
          connectedAdvice:
            "アクセス障害を調査していた場合は、外部公開エンドポイントが復旧したか再確認してください",
          disconnectedAdvice:
            "トンネル設定、アップストリームネットワークの状態、リモートサービスへの到達性を確認してください",
        },
        short: {
          loginFailureAttempts: "{count} 回失敗",
          scanHits: "スキャン {count} 回",
          scanBlocked: "スキャナーをブロック",
          success: "成功",
          failure: "失敗",
          blockSeconds: "{seconds} 秒間ブロック",
          blockTriggered: "ブロック発動",
          visibilityBlocked: "公開範囲でブロック",
          rules: "ルール {rules}",
          sshLoginSuccess: "SSH ログイン成功",
          sshLoginFailure: "SSH ログイン失敗",
          regionNotAllowed: "許可地域外",
          failureThreshold: "失敗しきい値",
          currentVersion: "現在 {version}",
        },
        titles: {
          ddnsUpdateSuccess: "{target} の更新に成功しました",
          ddnsUpdateFailure: "{target} の更新に失敗しました",
          credentialIpDrift: "認証情報「{credential}」の IP が変化しました",
          appUpdateAvailable: "新しいバージョン {version} が利用可能です",
        },
      },
    },
    providers: {
      catalog: {
        email: {
          label: "メール",
          description:
            "SMTP でメール通知を送信します。メールボックスの接続情報を一元管理するため、任意で IMAP 設定も保存できます",
          fields: {
            smtp_host: {
              label: "SMTP ホスト",
              description:
                "メール送信サーバーのアドレス（例: smtp.example.com）",
            },
            smtp_port: {
              label: "SMTP ポート",
              description:
                "一般的なポートは 465（SSL/TLS）または 587（STARTTLS）です",
            },
            smtp_security: {
              label: "SMTP 暗号化方式",
              options: {
                none: "暗号化なし",
              },
            },
            smtp_auth_mode: {
              label: "SMTP 認証方式",
              description:
                "AUTH PLAIN を優先し、必要に応じて AUTH LOGIN にフォールバックします",
              options: {
                auto: "自動選択",
                none: "認証なし",
              },
            },
            smtp_username: {
              label: "SMTP ユーザー名",
            },
            smtp_password: {
              label: "SMTP パスワード",
            },
            from_address: {
              label: "送信元アドレス",
              description: "MAIL FROM とメールヘッダーの From に使用します",
            },
            from_name: {
              label: "送信者名",
            },
            to_addresses: {
              label: "デフォルトの受信者",
              description:
                "複数のメールアドレスはカンマまたは改行で区切ります。テスト送信ではこの宛先を使用し、ルールごとに上書きできます",
              targetLabel: "受信者のオーバーライド",
              targetDescription:
                "任意。プロバイダーのデフォルト宛先を使用する場合は空欄にします",
              addressLabel: "受信者",
            },
            cc_addresses: {
              label: "デフォルト CC",
              targetLabel: "CC の上書き",
              addressLabel: "CC",
            },
            bcc_addresses: {
              label: "デフォルトの BCC",
              targetLabel: "BCC の上書き",
              addressLabel: "BCC",
            },
            reply_to: {
              label: "デフォルトの返信アドレス",
              targetLabel: "返信アドレスの上書き",
              addressLabel: "返信アドレス",
            },
            allow_invalid_tls: {
              label: "無効な証明書を許可",
              description:
                "セルフホスト型メールサーバーや自己署名証明書のデバッグ時にのみ使用してください。本番環境では無効にします",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            imap_host: {
              label: "IMAP ホスト",
              description:
                "任意。受信用メールボックスの設定として保存します。現在の通知送信は SMTP のみを使用し、IMAP からの読み込みは行いません",
            },
            imap_port: {
              label: "IMAP ポート",
            },
            imap_security: {
              label: "IMAP 暗号化方式",
              options: {
                none: "暗号化なし",
              },
            },
            imap_username: {
              label: "IMAP ユーザー名",
            },
            imap_password: {
              label: "IMAP パスワード",
            },
            imap_mailbox: {
              label: "IMAP メールボックス",
            },
            subject_prefix: {
              label: "件名の接頭辞",
              description: "任意（例: [本番環境]）",
              placeholder: "[本番環境]",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
            details: "詳細:",
            actionLinks: "操作リンク:",
            severity: "レベル: {value}",
            eventId: "イベント ID: {value}",
            occurredAt: "発生時刻: {value}",
          },
          errors: {
            invalidEmailAddress:
              "{field} には無効な電子メール アドレスが含まれています: {value}",
            smtpConnectionClosed: "SMTP 接続が閉じられました",
            smtpReaderDisposed: "SMTP リーダーが破棄されました",
            invalidSmtpResponse: "SMTP レスポンスを解析できません: {line}",
            smtpConnectionTimeout: "SMTP 接続がタイムアウトしました",
            smtpTlsHandshakeTimeout:
              "SMTP TLS ハンドシェイクがタイムアウトしました",
            smtpCommandFailed: "{message}: {code} {response}",
            unknownResponse: "不明な応答",
            authPlainUnsupported:
              "SMTP サーバーは AUTH PLAIN に対応していません",
            authLoginUnsupported:
              "SMTP サーバーは AUTH LOGIN に対応していません",
            unsupportedAuthMechanisms:
              "対応していない SMTP 認証方式です: {mechanisms}",
            authFailed: "SMTP 認証に失敗しました",
            usernameAuthFailed: "SMTP ユーザー名認証に失敗しました",
            passwordAuthFailed: "SMTP パスワード認証に失敗しました",
            dataStartFailed: "SMTP DATA フェーズの開始に失敗しました",
            submitFailed: "SMTP メール送信に失敗しました",
            invalidFromAddress: "送信元アドレスの形式が無効です",
            recipientRequired:
              "少なくとも 1 つの受信メール アドレスを設定する必要があります",
            handshakeFailed: "SMTP サーバーからのグリーティングに失敗しました",
            ehloFailed: "SMTP EHLO に失敗しました",
            startTlsUnsupported:
              "SMTP サーバーは STARTTLS 対応を通知していません",
            startTlsFailed: "SMTP STARTTLS に失敗しました",
            ehloAfterTlsFailed: "SMTP TLS アップグレード後 EHLO が失敗しました",
            credentialsRequired:
              "SMTP ユーザー名とパスワードを空にすることはできません",
            noAuthMechanism:
              "SMTP サーバーから使用可能な認証方式が提示されませんでした",
            mailFromFailed: "SMTP 送信者の設定に失敗しました",
            recipientSetFailed:
              "SMTP 受信者 {recipient} を設定できませんでした",
            quitFailed: "SMTP 終了に失敗しました",
            missingSmtpHost: "SMTP ホストが指定されていません",
            deliveryFailed: "メール配信に失敗しました",
          },
        },
        pushplus: {
          label: "PushPlus",
          description:
            "PushPlus の標準 API で通知を送信します。ルールごとに WeChat 公式アカウント、アプリ、メールなどのチャネルを選択できます",
          fields: {
            server_url: {
              label: "サービス URL",
              description: "必要がなければ公式 API の URL のままにします",
            },
            token: {
              label: "トークン",
              description:
                "PushPlus のユーザートークンまたはメッセージトークン。安全に管理してください",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            topic: {
              label: "トピックコード",
              description:
                "任意。指定したトピックへメッセージを送信します。空欄の場合はトークンの所有者へ送信します",
            },
            template: {
              label: "メッセージテンプレート",
              description:
                "デフォルトは Markdown です。送信先に応じてプレーンテキストや HTML に切り替えられます",
              options: {
                markdown: "Markdown",
                html: "HTML",
                txt: "プレーンテキスト",
                json: "JSON",
              },
            },
            channel: {
              label: "送信チャンネル",
              description:
                "デフォルトは WeChat 公式アカウントです。PushPlus で他のチャネルを設定済みの場合は、ここで切り替えられます",
              options: {
                wechat: "WeChat 公式アカウント",
                webhook: "サードパーティ Webhook",
                cp: "WeCom アプリ",
                mail: "メール",
                sms: "SMS",
                voice: "音声",
                extension: "プラグイン / デスクトップアプリ",
                app: "アプリ",
                clawbot: "WeChat ClawBot",
              },
            },
            option: {
              label: "チャネルオプション",
              description:
                "任意。cp、Webhook、メールなどでは通常、PushPlus のアカウントセンターで設定したチャネルコードが必要です",
            },
            to: {
              label: "フレンドトークン / ユーザー ID",
              description:
                "任意。WeChat 公式アカウントではフレンドトークン、WeCom アプリではユーザー ID を指定します。複数の宛先は PushPlus の形式で入力します",
              placeholder: "friend_token または user1、user2",
            },
            callback_url: {
              label: "コールバック URL",
              description:
                "任意。PushPlus の非同期配信が完了すると、この URL に結果が通知されます",
            },
            pre: {
              label: "前処理コード",
              description:
                "任意。PushPlus アカウントに対応する前処理ロジックを設定している場合のみ入力します",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingToken: "PushPlus トークンが指定されていません",
            requestFailed: "PushPlus のリクエストに失敗しました",
          },
        },
        wxpusher: {
          label: "WxPusher",
          description:
            "WxPusher の標準 API で指定した UID またはトピックへ通知を送信します。ルールの宛先が空欄の場合はプロバイダーのデフォルト設定を継承します",
          fields: {
            server_url: {
              label: "サービス URL",
              description: "必要がなければ公式サービスの URL のままにします",
            },
            app_token: {
              label: "AppToken",
              description:
                "WxPusher バックエンドアプリの AppToken。安全に管理してください",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            uids: {
              label: "デフォルト UID リスト",
              targetLabel: "UID リスト",
              description:
                "任意。テスト送信ではこの UID を優先し、ルールの宛先が空欄の場合にも使用します",
              targetDescription:
                "任意。プロバイダーのデフォルト UID を上書きします。継承する場合は空欄にします",
            },
            topic_ids: {
              label: "デフォルトのトピック",
              description:
                "任意。テスト送信ではこのトピックを優先します。チャネルを直接テストできるよう、デフォルト UID またはトピックを 1 件以上設定してください",
              targetDescription:
                "任意。プロバイダーのデフォルトトピックを上書きします。継承する場合は空欄にします",
            },
            url: {
              label: "デフォルトのメッセージ URL",
              targetLabel: "メッセージ URL",
              description:
                "任意。ルールの宛先が空欄の場合にこのリンクを継承し、テスト送信でも使用します",
              targetDescription:
                "任意。プロバイダーのデフォルトリンクを上書きします。継承する場合は空欄にします",
            },
            verify_pay_type: {
              label: "デフォルトのサブスクリプション検証",
              targetLabel: "サブスクリプションの検証",
              description:
                "任意。ルールの宛先が空欄の場合に、この購読検証ポリシーを継承します",
              targetDescription:
                "任意。プロバイダーのデフォルト購読検証ポリシーを上書きします。「プロバイダーのデフォルトを使用」を選ぶと個別には上書きしません",
              options: {
                "0": "未検証",
                "1": "有料会員限定",
                "2": "退会または期限切れのユーザーのみ",
                __inherit__: "プロバイダーのデフォルトを使用",
              },
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingAppToken: "WxPusher AppToken が指定されていません",
            invalidTopicIds: "トピック ID の形式が無効です: {values}",
            recipientRequired:
              "WxPusher には UID またはトピック ID が 1 件以上必要です。プロバイダーのデフォルト設定か、ルールの宛先に指定してください",
            targetsFailed:
              "WxPusher の送信先 {total} 件中 {failed} 件への送信に失敗しました",
            requestFailed: "WxPusher のリクエストに失敗しました",
          },
        },
        harmonyosmeow: {
          label: "HarmonyOSMeoW",
          description:
            "MeoW Push API を通じて HarmonyOS デバイスに Markdown 通知を送信します。",
          fields: {
            server_url: {
              label: "サービス URL",
              description:
                "必要がなければ公式 API URL のまま使用してください。",
            },
            nickname: {
              label: "受信者ニックネーム",
              description:
                "MeoW アプリで設定したユーザーニックネームです。非公開の受信者識別子として扱ってください。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
          },
          errors: {
            missingNickname: "MeoW の受信者ニックネームがありません",
            invalidNickname:
              "MeoW の受信者ニックネームにスラッシュは使用できません",
            invalidServerUrl: "MeoW サービス URL が無効です",
            requestFailed: "MeoW リクエストに失敗しました",
          },
        },
        bark: {
          label: "Bark",
          description:
            "Bark の公式サービスまたはセルフホストした Bark Server を使い、iPhone へ APNs プッシュ通知を送信します",
          fields: {
            server_url: {
              label: "サービス URL",
              description:
                "セルフホストした Bark Server を使用する場合を除き、公式サービスの URL のままにします",
            },
            device_key: {
              label: "Device Key",
              description:
                "Bark アプリからコピーした Device Key。複数指定する場合はカンマで区切ります",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            level: {
              label: "通知レベル",
              description:
                "active は通常の即時通知、timeSensitive は集中モードを突破できる通知、critical は重大な通知です",
              options: {
                active: "通常",
                timeSensitive: "時間指定",
                passive: "受動的",
                critical: "重大",
              },
            },
            group: {
              label: "メッセージグループ",
              description:
                "任意。同じグループのメッセージを Bark クライアントでまとめて表示します",
            },
            sound: {
              label: "通知音",
              description:
                "任意。Bark が対応するシステム通知音またはカスタム通知音の名前",
            },
            url: {
              label: "タップ時に開く URL",
              description:
                "任意。通知をタップするとこのリンクを開きます。空欄の場合はメッセージアクションの最初のリンクを使用します",
            },
            icon: {
              label: "アイコン URL",
              description:
                "任意。iOS 15 以降ではカスタムアイコンを表示できます",
            },
            badge: {
              label: "バッジ数",
              description: "任意。Bark アプリアイコンのバッジに表示する数値",
            },
            call: {
              label: "繰り返し鳴る",
              description: "有効にすると、Bark が約 30 秒間鳴り続けます。",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingDeviceKey: "Bark Device Key が指定されていません",
            requestFailed: "Bark のリクエストに失敗しました",
            pushFailed: "Bark プッシュに失敗しました",
            targetsFailed:
              "Bark の送信先 {total} 件中 {failed} 件への送信に失敗しました",
          },
        },
        serverchan: {
          label: "ServerChan",
          description:
            "ServerChan Turbo で Markdown 通知を送信し、Web サイト側で設定したデフォルト受信チャネルを使用します",
          fields: {
            server_url: {
              label: "サービス URL",
              description: "必要がなければ公式 API の URL のままにします",
            },
            sendkey: {
              label: "SendKey",
              description:
                "ServerChan Turbo が発行する SendKey。安全に管理してください",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            channel: {
              label: "メッセージチャンネル",
              description:
                "任意。この通知で使用するチャネルを最大 2 つ指定します。9|66 のように | で区切ります",
            },
            openid: {
              label: "OpenID / UID",
              description:
                "任意。テストアカウントでは openid、WeCom アプリのメッセージでは受信者の UID を使用します。複数指定する場合は ServerChan のドキュメントに従って入力します",
              placeholder: "openid1,openid2 または uid1|uid2",
            },
            short: {
              label: "カード概要",
              description:
                "任意。メッセージカードに表示する 64 文字以内の概要。空欄の場合は ServerChan が本文から生成します",
              placeholder: "ログイン異常、早めに対処してください",
            },
            noip: {
              label: "送信元 IP を非表示",
              description:
                "有効にすると、この通知に呼び出し元の IP を表示しません",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingSendKey: "ServerChan SendKey が指定されていません",
            requestReturned: "ServerChan が HTTP {status} を返しました",
            requestFailed: "ServerChan のリクエストに失敗しました",
          },
        },
        dingtalk: {
          label: "DingTalk Bot",
          description:
            "DingTalk Bot の Webhook でグループチャットへ Markdown 通知を送信します。署名検証にも対応します",
          fields: {
            webhook_url: {
              label: "Webhook URL",
              description: "DingTalk Bot が生成した完全な Webhook URL",
            },
            secret: {
              label: "署名キー",
              description:
                "任意。Bot の署名を有効にしている場合は、セキュリティ設定に表示される SEC から始まるシークレットを入力します",
            },
            keyword_prefix: {
              label: "キーワードプレフィックス",
              description:
                "任意。Bot でカスタムキーワード検証を有効にしている場合、その固定キーワードを指定します。送信時に件名の先頭へ自動追加されます",
              placeholder: "監視アラーム",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            at_mobiles: {
              label: "@ メンバーの携帯番号",
              description:
                "任意。複数指定する場合はカンマまたは改行で区切ります。グループメンバーの携帯番号を指定してください",
            },
            at_user_ids: {
              label: "@ ユーザー ID",
              description:
                "任意。複数指定する場合はカンマまたは改行で区切ります。本文に @userId が自動追加されます",
            },
            is_at_all: {
              label: "@ 全員",
              description:
                "有効にすると、isAtAll がリクエストに含まれ、@Everyone がテキストに追加されます。",
            },
          },
          mentionAll: "@ 全員",
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingWebhookUrl: "DingTalk Webhook URL が指定されていません",
            requestReturned: "DingTalk が HTTP {status} を返しました",
            requestFailed: "DingTalk のリクエストに失敗しました",
          },
        },
        feishu: {
          label: "Feishu Bot",
          description:
            "Feishu Bot の Webhook でグループチャットへリッチテキスト通知を送信します。署名検証にも対応します",
          fields: {
            webhook_url: {
              label: "Webhook URL",
              description: "Feishu Bot が生成した完全な Webhook URL",
            },
            secret: {
              label: "署名キー",
              description:
                "任意。Bot の署名検証を有効にしている場合は、セキュリティ設定からコピーしたシークレットを入力します",
            },
            keyword_prefix: {
              label: "キーワードプレフィックス",
              description:
                "任意。Bot でカスタムキーワード検証を有効にしている場合、その固定キーワードを指定します。送信時に件名の先頭へ自動追加されます",
              placeholder: "アプリアラート",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            mention_user_ids: {
              label: "@ ユーザー ID",
              description:
                "任意。複数指定する場合はカンマまたは改行で区切ります。all も指定できます。外部グループで個人をメンションする場合は Open ID のみ対応します",
            },
          },
          mentionAll: "全員",
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingWebhookUrl: "Feishu Webhook URL が指定されていません",
            requestReturned: "Feishu が HTTP {status} を返しました",
            requestFailed: "Feishu のリクエストに失敗しました",
          },
        },
        webhook: {
          label: "Webhook",
          description:
            "HTTP JSON に対応する任意のエンドポイントへ標準形式の通知を送信します",
          fields: {
            url: {
              label: "Webhook URL",
              description: "標準形式の通知 JSON を受信するエンドポイント",
            },
            method: {
              label: "リクエストメソッド",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            shared_secret: {
              label: "共有シークレット",
              description:
                "任意。指定すると X-Fn-Knock-Signature リクエストヘッダーで送信します",
            },
            endpoint_path: {
              label: "追加パス",
              description: "任意。送信前にベース Webhook URL へ追加します",
            },
            extra_headers_json: {
              label: "追加ヘッダー JSON",
              description: '任意（例: {"X-Env":"prod"}）',
            },
            extra_body_json: {
              label: "追加ボディ JSON",
              description: "任意。payload.extra_body に追加します",
            },
          },
          errors: {
            missingUrl: "Webhook URL が指定されていません",
            requestReturned: "Webhook が HTTP {status} を返しました",
            requestFailed: "Webhook のリクエストに失敗しました",
          },
        },
        magicpush: {
          label: "MagicPush",
          description:
            "セルフホストした MagicPush サービスから設定済みのチャネルへ通知を送信します。標準プッシュと MagicPush インバウンドモードに対応します",
          fields: {
            server_url: {
              label: "ベース API URL",
              description:
                "MagicPush サービスのルート URL（例: http://192.168.31.98:3000）。/api/push または /api/inbound を含む URL も使用できます",
            },
            delivery_mode: {
              label: "配信モード",
              description:
                "標準プッシュは /api/push へ送信します。インバウンドモードは /api/inbound/:token へ送信し、MagicPush 側のインバウンドルールでフィールドをマッピングします",
              options: {
                push: "標準プッシュ",
                inbound: "インバウンドモード",
              },
            },
            token: {
              label: "トークン",
              description:
                "MagicPush API トークン。標準プッシュでは Authorization: Bearer で送信し、インバウンドモードでは /api/inbound/:token の末尾に追加します",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingBaseUrl: "MagicPush ベース API URL が指定されていません",
            missingToken: "MagicPush トークンが指定されていません",
            invalidBaseUrl: "MagicPush ベース API URL が無効です",
            requestReturned: "MagicPush が HTTP {status} を返しました",
            requestFailed: "MagicPush のリクエストに失敗しました",
          },
        },
        telegram: {
          label: "Telegram",
          description:
            "Telegram Bot API で指定したチャットまたはチャネルへ、インラインアクションボタン付きのテキスト通知を送信します",
          fields: {
            server_url: {
              label: "Bot API URL",
              description:
                "通常は公式 Bot API のままにします。公式エンドポイントへ接続できない場合は中継先として https://tgapi.fnknock.cn を指定できます。セルフホストした Local Bot API Server のルート URL も使用できます",
            },
            bot_token: {
              label: "Bot Token",
              description: "@BotFather で Bot を作成した際に取得した Bot Token",
            },
            chat_id: {
              label: "Chat ID",
              description:
                "送信先のチャット ID またはチャネルユーザー名（例: @channelusername）。@UserIdzhBot へメッセージを送ると Chat ID を確認できます。テスト送信でもこの宛先を使用します",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            message_thread_id: {
              label: "トピック ID",
              description:
                "任意。グループトピックへ送信する場合のトピック ID（message_thread_id）",
            },
            disable_notification: {
              label: "サイレント送信",
              description:
                "有効にすると、Telegram は通知音を鳴らさずに配信します",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingBotToken: "Telegram Bot Token が指定されていません",
            missingChatId: "Telegram Chat ID が指定されていません",
            requestReturned: "Telegram が HTTP {status} を返しました",
            requestFailed: "Telegram のリクエストに失敗しました",
          },
        },
        wecom: {
          label: "WeCom グループ Bot",
          description:
            "WeCom のグループ Webhook で、指定したグループチャットへテキストまたは Markdown 通知を送信します",
          fields: {
            webhook_url: {
              label: "Webhook URL",
              description:
                "WeCom のメッセージ送信画面で生成された完全な Webhook URL。安全に管理してください",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            mentioned_list: {
              label: "メンションするメンバーの UserID",
              description:
                "任意。複数指定する場合はカンマまたは改行で区切ります。@all も指定できます",
            },
            mentioned_mobile_list: {
              label: "メンションする携帯番号",
              description:
                "任意。複数指定する場合はカンマまたは改行で区切ります。@all も指定できます",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingWebhookUrl: "WeCom Webhook URL が指定されていません",
            requestReturned: "WeCom が HTTP {status} を返しました",
            requestFailed: "WeCom のリクエストに失敗しました",
          },
        },
        pushdeer: {
          label: "PushDeer",
          description:
            "PushDeer の公式サービスまたはセルフホストしたサービスで、紐付け済みデバイスへ Markdown 通知を送信します",
          fields: {
            server_url: {
              label: "サービス URL",
              description:
                "セルフホストした PushDeer を使用する場合を除き、公式サービスの URL のままにします",
            },
            pushkey: {
              label: "PushKey",
              description:
                "PushDeer クライアントで生成した PushKey。複数指定する場合はカンマで区切ります",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
          },
          message: {
            fallbackTitle: "fn-knock 通知",
          },
          errors: {
            missingPushKey: "PushDeer PushKey が指定されていません",
            requestReturned: "PushDeer が HTTP {status} を返しました",
            apiReturnedCode: "PushDeer API がコード {code} を返しました",
            requestFailed: "PushDeer のリクエストに失敗しました",
          },
        },
      },
    },
    routes: {
      createProviderFailed: "通知プロバイダーの作成に失敗しました",
      testProviderFailed: "通知プロバイダーのテストに失敗しました",
      getProviderFailed: "通知プロバイダーの取得に失敗しました",
      updateProviderFailed: "通知プロバイダーの更新に失敗しました",
      deleteProviderFailed: "通知プロバイダーの削除に失敗しました",
      createRuleFailed: "通知ルールの作成に失敗しました",
      updateRuleFailed: "通知ルールの更新に失敗しました",
      deleteRuleFailed: "通知ルールの削除に失敗しました",
      unsupportedDeliveryStatus: "対応していない配信状態です",
      clearDeliveriesFailed: "配信履歴の消去に失敗しました",
    },
    service: {
      unnamed: "無名",
      invalidJsonBody: "リクエスト本文は有効な JSON である必要があります",
      invalidJson: "{field} には有効な JSON を指定してください",
      invalidSelectValue: "{field} の値が無効です",
      fieldRequired: "{field} を空にすることはできません",
      testMessage: {
        title: "テスト通知",
        summary:
          "通知チャネルは正しく設定され、テストメッセージの送信に成功しました",
        bodyText:
          "これは、プロバイダーへの接続、構造化されたメッセージ、表示形式を確認するために fn-knock が送信したテスト通知です",
        bodyMarkdown:
          "**接続確認に成功しました。**\n\nこれは、プロバイダーへの接続、構造化されたメッセージ、表示形式を確認するために fn-knock が送信したテスト通知です。",
        sendType: "送信タイプ",
        providerTest: "プロバイダーテスト",
        sentAt: "送信日時",
      },
      providerNotFound: "通知プロバイダーが存在しません",
      unsupportedProviderType: "サポートされていない通知プロバイダーの種類です",
      providerDefinitionMissing: "通知プロバイダー定義が存在しません",
      providerReferencedByRule:
        "このプロバイダーはまだルール「{rule}」によって参照されています",
      testSendFailed: "テスト送信に失敗しました",
      testSendSuccess: "テスト送信に成功しました",
      providerRequestReturnedStatus:
        "{provider} のリクエストが HTTP ステータス {status} を返しました",
      barkPartialFailed:
        "Bark の送信先 {total} 件中 {failed} 件への送信に失敗しました",
      providerTypeMismatch: "プロバイダーのタイプが既存の設定と一致しません",
      providerTestName: "{provider} テスト",
      invalidProviderRecord: "通知プロバイダーレコードが無効です",
      ruleProviderMissing: "ルールは存在しない通知プロバイダーを参照しています",
      invalidTemplateOverrideMode: "送信先テンプレートの上書きモードが無効です",
      unsupportedEventType: "対応していないシステムイベント種別です",
      invalidGroupBy: "集約単位が無効です",
      invalidMessageTemplateMode: "メッセージテンプレートモードが無効です",
      invalidEventLevelFilter: "イベントレベルのフィルタ条件が不正です",
      invalidEventSourceFilter: "イベントソースのフィルター条件が不正です",
      targetRequired: "通知先を 1 件以上指定してください",
      duplicateEventRule:
        "このイベントには通知ルールがすでに存在します。先に既存のルールを削除してください",
      ruleNotFound: "通知ルールが存在しません",
      invalidRuleRecord: "通知ルールレコードが無効です",
      deletedProvider: "プロバイダーが削除されました",
      storageUnavailable: "通知ストレージは一時的に利用できません",
    },
  },
};
