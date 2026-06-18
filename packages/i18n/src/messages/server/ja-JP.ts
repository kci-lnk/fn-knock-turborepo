export const jaJPServer = {
  success: "成功",
  notFound: "見つかりません",
  invalidLocale: "サポートされていないロケールです",
  dockerAdminDenied:
    "Docker 管理パネルでは、イントラネットまたは信頼できるリバース プロキシ アクセスのみが許可されます",
  dockerAdminDeniedTitle: "アクセスが拒否されました",
  dockerAdminDeniedDescription:
    "Docker 管理パネルは、デフォルトでホストローカル、LAN、VPN、または設定された信頼できるリバースプロキシからのアクセスのみを許可します。公衆ネットワークへの直接接続は拒否されます。",
  dockerAdminCurrentIp: "現在の識別ソース IP: {ip}",
  dockerAdminProxyRequired:
    "{port}管理ポータルからバックエンドインターフェースにアクセスしてください",
  dockerAdminLoginRequired: "まずはDocker管理パネルにログインしてください",
  captchaUnavailable: "認証コードサービスは一時的にご利用いただけません",
  tooManyAttempts: "何度も試しました。後でもう一度試してください。",
  tooManyAttemptsWithRetry:
    "試行回数が多すぎます。{seconds} 秒後にもう一度お試しください",
  loginCredentialMissing: "サーバーにはログイン認証情報が設定されていません",
  invalidOtpWithRetry:
    "確認コードが正しくありません。{seconds} 秒後にもう一度お試しください。",
  runtimeProfile: {
    capabilities: {
      default: "現在の動作環境はこの機能をサポートしていません",
      direct_mode_available: {
        docker:
          "Docker 導入ではホスト ダイレクト ファイアウォール モードがサポートされません",
        platform:
          "現在の動作環境はホスト直接接続ファイアウォールモードをサポートしていません。",
        permission:
          "現在のプロセスには、ホストのファイアウォールに直接接続する機能がありません。",
      },
      host_firewall_available: {
        docker:
          "Docker 導入ではホスト ファイアウォール管理がサポートされていません",
        platform:
          "現在のオペレーティング環境はホストのファイアウォール管理をサポートしていません",
        permission:
          "現在のプロセスにはホスト ファイアウォール管理機能がありません",
      },
      smart_connect_available: {
        docker:
          "Docker デプロイメントはスマート コネクトをまだサポートしておらず、ホスト dnsmasq とポート 53 に依存しています。",
        platform:
          "現在の動作環境はまだスマートコネクトをサポートしていません。",
        permission:
          "現在のプロセスには、スマート コネクトに必要なホスト管理機能がありません",
      },
      system_clock_sync_available: {
        docker: "Docker 導入ではホスト システムの時刻同期がサポートされません",
        platform: "現在の動作環境はシステム時刻同期をサポートしていません",
        permission:
          "現在のプロセスには、システム時刻の同期に必要なホスト権限がありません。",
      },
      self_update_available: {
        docker:
          "Docker のデプロイメントはアプリ内 FPK アップデートをサポートしていません。新しいイメージを取得してアップグレードしてください。",
        openwrt:
          "OpenWrt の展開ではアプリ内 FPK アップデートがサポートされていません。デバイス アーキテクチャに一致する IPK をインストールし、opkg 経由でアップグレードしてください。",
        deployment:
          "現在の展開フォームはアプリ内アップデートをサポートしていません。",
      },
      terminal_available: {
        docker: "Docker デプロイメントは Web ターミナルをサポートしていません",
        openwrt: "OpenWrt 導入では現在 Web 端末をサポートしていません",
        platform: "現在の動作環境はWeb端末をサポートしていません",
      },
      shared_root_available: {
        missing:
          "現在の実行環境にはマウントできる共有ディレクトリがありません。",
      },
    },
  },
  systemClock: {
    unknown: "不明",
    actionSeparator: ";",
    listSeparator: "、",
    duration: {
      seconds: "{seconds} 秒",
      minutes: "{minutes} 分",
      minutesSeconds: "{minutes}分 {seconds}秒",
    },
    networkCheckFailed: "オンラインでシステム時間を確認できませんでした",
    issues: {
      timezone: {
        title: "システムのタイムゾーンは北京時間ではありません",
        message:
          "現在のシステムのタイムゾーンは{timezone}で、{expected}に設定する必要があります。",
      },
      timeMismatch: {
        title: "システム時刻がネットワーク検証結果と一致しません",
        message:
          "現在のシステム時刻とネットワーク検証結果の差は約{drift}です。",
      },
    },
    statusRefreshed: "システム時間ステータスが更新されました",
    syncFailed: "システム時刻の同期に失敗しました",
    networkTimeUnavailable: "ネットワークから標準時刻を取得できませんでした",
    sourceFetchFailed: "{source}からの時間を取得できませんでした",
    missingDateHeader:
      "{source} 利用可能な日付応答ヘッダーが返されませんでした",
    invalidDateHeader: "{source} は解析できない時間を返しました",
    commandFailed: "{command} の実行に失敗しました",
    timezoneSet: "システムのタイムゾーンは {timezone} に設定されました",
    missingZoneinfoFile: "システムにタイムゾーンファイル {path} がありません",
    timezoneWritten: "はシステムのタイムゾーン {timezone} に書き込まれています",
    clockAdjusted: "校正されたシステム時間",
    ntpEnabled: "有効 NTP 自動時刻修正",
    serviceRestarted: "{service} サービスを再開しました",
  },
  updateRoutes: {
    downloadStarted: "がアップデートパッケージのダウンロードを開始しました",
    downloadStartFailed: "ダウンロードの開始に失敗しました",
    installStarted: "アップデートのインストールプロセスが開始されました",
    installStartFailed: "インストールの開始に失敗しました",
    checkAndDownloadStarted: "チェックを開始し、ダウンロードを開始しました",
    startFailed: "起動に失敗しました",
  },
  gatewayHostResponse: {
    runTypes: {
      direct: "ダイレクト接続モード",
      reverseProxy: "アンチジェネレーションモード",
      subdomain: "サブドメインモード",
    },
    unavailableReason:
      "サブドメイン モードのみが利用可能で、現在は {mode} です。",
    editSubdomainOnly:
      "ホスト応答はサブドメイン マッピング モードでのみ編集できます",
    syncFailed: "同期ゲートウェイ ホスト応答設定に失敗しました",
    hostRoutesSyncFailed: "ホストルートの同期に失敗しました",
    updateFailed: "ゲートウェイのホスト応答の更新に失敗しました",
    updateFailedRolledBack:
      "ゲートウェイのホスト応答の更新に失敗し、設定はロールバックされました",
    updateFailedRollbackFailed:
      "{error};ロールバックが失敗しました: {rollbackError}",
    restoreConfigFailed: "元の構成に応答するようにホストを復元できませんでした",
    restoreRuntimeFailed: "ホストの応答を実行状態に復元できませんでした",
    restoreGatewayRuntimeFailed:
      "ゲートウェイホストの応答を実行状態に復元できませんでした",
  },
  admin: {
    runTypes: {
      direct: "ダイレクト接続モード",
      reverseProxy: "アンチジェネレーションモード",
      subdomain: "サブドメインモード",
    },
    validation: {
      required: "{label} を空にすることはできません",
      httpUrlRequired:
        "{label}は http:// または https:// で始まる必要があります",
      proxyTargetUrlRequired:
        "{label} は http://、https://、ws://、または wss:// で始まり、ホストを含む必要があります",
      invalidFormat: "{label}形式が正しくありません",
    },
    rollback: {
      failed: "{message};ロールバックが失敗しました: {rollbackError}",
      restoreConfigFailed: "以前の構成を復元できませんでした",
      restoreSmartConnectFailed:
        "以前のスマート接続の実行状態を復元できませんでした",
      restoreRuntimeFailed: "以前の実行状態の復元に失敗しました",
      restoreProtocolConfigFailed:
        "プロトコル マッピング設定の復元に失敗しました",
      restoreProtocolFeatureFailed:
        "プロトコルマッピング機能スイッチの復元に失敗しました",
      restoreProtocolRuntimeFailed:
        "プロトコル マッピングを実行状態に復元できませんでした",
      restoreVisibilityConfigFailed: "可視性を元の構成に復元できませんでした",
      restoreVisibilityRuntimeFailed:
        "可視性ランタイムの復元 CIDR が失敗しました",
      restoreGatewayVisibilityFailed:
        "ゲートウェイの可視性を実行状態に復元できませんでした",
      restoreProxyHeadersConfigFailed:
        "プロトコルヘッダーの元の設定を復元できませんでした",
      restoreProxyHeadersRuntimeFailed:
        "プロトコルヘッダーを実行状態に復元できませんでした",
      restoreGatewayProxyHeadersRuntimeFailed:
        "ゲートウェイ プロトコル ヘッダーの実行状態を復元できませんでした",
      restorePortalFailed: "ポータル表示を実行状態に復元できませんでした",
    },
    dockerPanel: {
      passwordNotNeeded:
        "現在の動作モードでは設定が必要ありません Docker 管理パネルのパスワード",
      setPasswordFailed: "管理パネルのパスワードの設定に失敗しました",
      passwordChangeUnsupported:
        "現在の動作モードは、Docker 管理パネルのパスワードの変更をサポートしていません",
      changePasswordFailed: "管理パネルのパスワードの変更に失敗しました",
      tooManyAttemptsWithRetry:
        "試行回数が多すぎます。{seconds} 秒後にもう一度お試しください",
      tooManyAttempts: "何度も試しました。後でもう一度試してください。",
      passwordSetupRequired:
        "管理パネルのパスワードが設定されていません。最初の設定を完了してください。",
      passwordIncorrectWithRetry:
        "管理パネルのパスワードが正しくありません。{seconds} 秒後にもう一度お試しください。",
    },
    runType: {
      switchFailed: "動作モードの切り替えに失敗しました",
      switchFailedRolledBack:
        "動作モードの切り替えに失敗しました。設定はロールバックされました",
    },
    firewall: {
      whitelistSynced: "と同期 {count} ホワイトリスト IP",
      exemptPorts: "、予約済みエントリーポート{ports}",
      resetSuccess:
        "{runType}を押してファイアウォールをリセットしました {whitelistMessage}{exemptPortsMessage}",
      resetFailed: "ファイアウォールのリセットに失敗しました",
      clearSuccess:
        "ファイアウォール ルールをクリアし、{port} ポートに関連する履歴リダイレクトを削除しました",
      clearFailed: "ファイアウォールのクリアに失敗しました",
    },
    protocolMapping: {
      subdomainOnly:
        "プロトコル マッピングはサブドメイン モードでのみ有効にできます",
      updateFeatureFailed:
        "プロトコルマッピング機能スイッチの更新に失敗しました",
      updateFeatureFailedRolledBack:
        "プロトコルマッピング機能スイッチの更新に失敗し、設定がロールバックされました",
    },
    smartConnect: {
      subdomainOnly:
        "スマート コネクトはサブドメイン モードでのみ有効にできます",
      updateFailed: "スマート接続の更新に失敗しました",
      updateFailedRolledBack:
        "スマート接続の更新に失敗し、構成がロールバックされました",
    },
    fnosPortIcon: {
      syncFailed:
        "Feiniu ポート アイコンの引き継ぎ設定をゲートウェイに同期できませんでした",
    },
    gateway: {
      syncAuthCacheFailed:
        "認証キャッシュ構成をゲートウェイに同期できませんでした",
      syncThrottleFailed:
        "ゲートウェイ スロットリング構成をゲートウェイに同期できませんでした",
      updateFailed: "ゲートウェイ構成の更新に失敗しました",
      updateFailedRolledBack:
        "ゲートウェイ構成の更新に失敗しました。構成はロールバックされました。",
    },
    proxyMappings: {
      syncRulesFailed: "パスプロキシルートの同期に失敗しました",
      restoreRulesFailed: "パスプロキシルートの復元に失敗しました",
      updateFailed: "パスプロキシマッピングの更新に失敗しました",
      updateFailedRolledBack:
        "パスプロキシマッピングの更新に失敗しました。設定はロールバックされました",
    },
    gatewayVisibility: {
      updateFailed: "ゲートウェイの可視性を更新できませんでした",
      updateFailedRolledBack:
        "ゲートウェイの可視性を更新できませんでした。設定はロールバックされました。",
    },
    gatewayProxyHeaders: {
      subdomainOnly:
        "プロトコルヘッダーはサブドメインマッピングモードでのみ編集できます",
      updateFailed: "ゲートウェイプロトコルヘッダーの更新に失敗しました",
      updateFailedRolledBack:
        "ゲートウェイ プロトコル ヘッダーの更新に失敗し、構成はロールバックされました",
    },
    captcha: {
      turnstileKeysRequired:
        "Cloudflare回転木戸が有効な場合、site_keyとsecret_keyの両方を入力する必要があります",
    },
    ipLocation: {
      ipLookupUrlLabel: "IP 識別ライブラリのアドレス",
      cidrUrlLabel: "CIDR アドレスライブラリのアドレス",
    },
    connectionTest: {
      httpStatus: "サービスはエラーステータスコード {status} を返します",
      invalidData: "サービスがデータを異常に返しました",
      success: "正常に接続されました",
      timeout: "接続タイムアウト",
      failed: "接続に失敗しました",
    },
    autoHttps: {
      dockerUnsupported: "Docker バージョンは自動 HTTPS をサポートしていません",
      openWrtUnsupported:
        "OpenWrt バージョンは自動 HTTPS をサポートしていません",
      startFailed: "自動 HTTPS 起動に失敗しました",
    },
    hostMappings: {
      singleAuthPortMapping:
        "は、認証サービスとして AUTH_PORT を指すホスト マッピングを 1 つだけ持つことができます",
      authMappingMustBePublic:
        "認証サービス {host} はパブリック入口を維持する必要があり、自己認証や厳格なホワイトリストを有効にすることはできません。そうしないと、ログイン入口に到達できなくなります。",
      authMappingBasicAuthForbidden:
        "認証サービス {host} 資格情報の注入を有効にできません",
      basicAuthInvalid:
        "ホスト マッピング {host} の資格情報の挿入には、ユーザー名とパスワードの入力が必要であり、ユーザー名にはコロンを含めることはできません",
      locationPathRequired:
        "ホスト マッピング {host} のパス ルールにパスを入力する必要があります",
      locationPathMustStartSlash:
        "ホスト マッピング {host} {path} のパス ルールは / で始まる必要があります",
      locationRootForbidden:
        "ホスト マッピング {host} では、パス ルールとしてルート パスの構成が許可されません",
      locationReservedPath:
        "ホストマッピング {host} のパスルール {path} は予約されたパスを使用します",
      locationDuplicate:
        "ホストマッピング {host} 重複したパスルールが存在します {path}",
      locationTargetRequired:
        "ホストマッピング {host} パスルール {path} にターゲットを入力する必要があります",
      locationStatusInvalid:
        "ホストマッピング {host} パスルール {path} 応答ステータスコードは 100 ～ 599 でなければなりません",
      locationHeaderInvalid:
        "ホスト マッピング {host} のパス ルール {path} に不正な応答ヘッダー {header} が含まれています",
      locationHeaderForbidden:
        "ホストマッピング {host} パスルール {path} 応答ヘッダーをカスタマイズできません {header}",
      syncHostRulesFailed: "ホストルートの同期に失敗しました",
      syncAuthConfigFailed: "認証ゲートウェイ構成の同期に失敗しました",
      updateFailed: "ホストマッピングの更新に失敗しました",
      updateFailedRolledBack:
        "ホスト マッピングの更新に失敗し、構成はロールバックされました",
      metadataFailed: "ターゲットアドレスタイトルの更新に失敗しました",
      bookmarkFolderForRoot: "{root} サブドメインマッピング",
      bookmarkFolderDefault: "fn-knock サブドメインマッピング",
    },
    streamMappings: {
      listenPortNotInteger:
        "リスニングポート {port} は有効な整数ではありません",
      listenPortOutOfRange: "リスニングポート {port} が有効範囲外です",
      duplicatePort:
        "{protocol} リスニングポート {port} 重複しています。プロトコルとポートを一意にしてください",
      targetMustBeHostPort:
        "ターゲット アドレス {target} は、ホスト:ポートの形式である必要があります",
      syncFailed:
        "プロトコル マッピングとゲートウェイ ポート解放ルールの同期に失敗しました",
      syncFailedRolledBack:
        "プロトコル マッピングとゲートウェイ ポート解放ルールの同期に失敗し、設定はロールバックされました",
    },
    passkeyRp: {
      parentDomainRequired:
        "親ドメイン Passkey RP を有効にする場合は、最初にルート ドメイン名を入力するか、親ドメイン RP ID を明示的に指定してください。",
      mustMatchAuthHost:
        "親ドメイン Passkey RP ID {rpId} は、認証サービス {authHost} またはその親ドメインと同じである必要があります。",
    },
    subdomainMode: {
      sslAutoSelected:
        "は、現在のサブドメイン モードにより適した証明書に自動的に切り替わりました。",
      sslAutoSelectionSyncFailed:
        "推奨証明書は見つかりましたが、ゲートウェイとの同期に失敗し、自動切り替えが行われませんでした。",
    },
    totp: {
      invalidCode: "認証コードが間違っています。もう一度お試しください。",
      notFound: "TOTP が見つかりません",
    },
    passkeys: {
      notFound: "Passkey が見つかりません",
    },
    syncRoutes: {
      partialFailedGatewayLogging:
        "同期部分が失敗しました: gateway_logging={gatewayLogging}",
      partialFailedGatewayLoggingWaf:
        "同期部分が失敗しました: gateway_logging={gatewayLogging}、waf={waf}",
      success:
        "は、現在の動作モードに応じて、{rules} パス ルート、{hostRules} ホスト ルート、{streamRules} プロトコル マッピング、リクエスト ログ構成、および WAF 構成を同期しています。",
    },
    backup: {
      readFnosDirectoryFailed:
        "Feiniu バックアップ ディレクトリの読み取りに失敗しました",
      exportFnosSuccess:
        "バックアップが Feiniu ディレクトリにエクスポートされました",
      exportFnosFailed: "Feiniu ディレクトリへのエクスポートに失敗しました",
      importSuccessWithWarnings:
        "バックアップはインポートされましたが、実行状態の同期の一部が失敗しました。",
      importSuccess:
        "バックアップがインポートされ、実行ステータスの同期が完了しました",
      importFailed: "バックアップのインポートに失敗しました",
      importFnosSuccessWithWarnings:
        "Feiniu バックアップはインポートされましたが、実行状態の同期の一部が失敗しました",
      importFnosSuccess:
        "Feiniu バックアップがインポートされ、実行ステータスの同期が完了しました",
      importFnosFailed: "Feiniu からのバックアップのインポートに失敗しました",
    },
    sessions: {
      notFound: "セッションが見つかりません",
    },
  },
  gatewayLogs: {
    configSyncFailed:
      "リクエストログ設定は保存されますが、ゲートウェイとの同期に失敗します",
    readDirectoryFailed: "ログディレクトリの読み取りに失敗しました",
    readDatesFailed: "ログ日付の読み取りに失敗しました",
    readEntriesFailed: "リクエストログの読み取りに失敗しました",
    deleteEntriesFailed: "リクエストログの削除に失敗しました",
  },
  backoffRoutes: {
    ipRequired: "ip パラメータがありません",
  },
  ipLocationRoutes: {
    batchLimit: "単一クエリの最大数 {max} IP",
  },
  gatewayPortal: {
    syncConfigFailed: "ポータル表示設定のゲートウェイへの同期に失敗しました",
    syncHostRulesFailed: "ホストルートの同期に失敗しました",
  },
  gatewayVisibility: {
    customCidrInvalid: "カスタム CIDR 間違った形式: {cidrs}",
    emptyEnabledConfig:
      "可視性をオンにした後、少なくとも 1 つの領域またはカスタム CIDR を追加する必要があります",
    syncFailed: "ゲートウェイ可視性構成の同期に失敗しました",
  },
  gatewayLogging: {
    syncConfigFailed: "同期ゲートウェイ要求ログの構成に失敗しました",
  },
  sslGateway: {
    clearFailed: "ゲートウェイ証明書のクリアに失敗しました",
    syncFailed: "ゲートウェイ証明書の同期に失敗しました",
  },
  sslRoutes: {
    gatewayStatusReadFailed: "ゲートウェイ SSL ステータスを読み取れません",
    readSharedFileFailed: "共有ディレクトリファイルの読み込みに失敗しました",
    emptyDomains: "ドメイン名リストが空です。最初にドメイン名を追加するか、IP",
    certOrKeyInvalid: "証明書または秘密鍵が無効です",
    hostRequired: "ホストを空にすることはできません",
    localCaCertificateLabel: "ローカル CA 証明書",
    success: "成功",
    certNotInstalled: "証明書がインストールされていません",
    manualCertificateLabel: "証明書を手動でアップロードします",
    certNotFound: "証明書が存在しません",
  },
  redis: {
    defaultCredential: "デフォルトの認証情報",
    certificateLabels: {
      acme: "ACME 証明書",
      ca: "自己署名証明書",
      manual: "証明書を手動でアップロードします",
      current: "現在の証明書",
    },
    ssl: {
      certFormatInvalid: "無効な証明書形式: {message}",
      keyFormatInvalid: "無効な秘密鍵形式: {message}",
      certKeyMismatch: "証明書と秘密鍵が一致しません",
      certKeyCheckFailed: "証明書と秘密キーの検証に失敗しました: {message}",
      certContentRequired: "証明書の内容を空にすることはできません",
      certNotFound: "証明書が存在しません",
      certOrKeyInvalid: "証明書または秘密鍵が無効です",
    },
    acme: {
      domainRequired: "ドメイン名を空にすることはできません",
      domainsRequired: "ドメイン名のリストを空にすることはできません",
      dnsProviderRequired: "DNS サービスプロバイダーを空にすることはできません",
      primaryDomainDuplicated:
        "プライマリ ドメイン名 {primaryDomain} はすでに他のアプリケーションに存在します",
      applicationNotFound: "申請項目が存在しません",
      noMatchingIssuedCertificate:
        "現在のアプリケーション項目には、ドメイン名構成に一致する発行済み証明書がありません。",
      jobDataInvalid: "ACME 無効なタスクデータです",
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
      "登録 ACME アカウントが失敗しました (終了コード: {code}) {brief}",
    bundledZipMissing: "組み込みの acmesh.zip リソースが見つかりません",
    extractingBundled: "組み込みの acme.sh リソースを解凍しています...",
    unzipFailed: "解凍に失敗しました。終了コード: {code}",
    extractedAcmeMissing: "解凍は成功しましたが、acme.sh が見つかりません",
    writingDataDir: "データ ディレクトリに書き込み中...",
    writtenAcmeMissing: "acme.sh を書き込んだ後に見つかりません",
    checkInstallFailed: "インストール状況の確認に失敗しました: {detail}",
    ready: "acme.sh の準備ができました",
    notInstalled: "acme.sh がインストールされていません",
    initializingBundled: "組み込みの acme.sh を初期化しています...",
    registeringAccount: "ACME アカウントを登録中...",
    savingDefaultCa: "デフォルトの認証局を保存しています...",
    installSuccess:
      "インストールが成功しました。アカウントのメールアドレス: {email}",
    installFailed: "インストールに失敗しました: {detail}",
    installFirst: "まず acme.sh をインストールしてください",
    installingCannotDelete: "acme.sh がインストール中のため削除できません",
    deleted: "acme.sh が削除されました",
    deleteFailed: "削除に失敗しました: {detail}",
    domainsRequired: "ドメイン名のリストを空にすることはできません",
    dnsTypeRequired: "DNS 検証タイプがありません",
    issueFailed: "証明書発行失敗（終了コード：{code}） {brief}",
  },
  acmeJobRunner: {
    manualStop: "ACME タスクはユーザーによって手動で停止されました",
    lockMessages: {
      manualRequest: "証明書の申請",
      autoRenew: "証明書の自動更新",
    },
    activeTaskRunning:
      "現在 ACME のタスクが実行中です。後でもう一度お試しください。",
    flowFailed: "証明書申請プロセスが失敗しました: {message}",
    stopSignalSent: "停止信号を送信し、{count} acme.sh プロセスを終了しました",
    noRunningProcess: "実行中の acme.sh プロセスが見つかりません",
    stopProcessError: "プロセスの停止中に例外が発生しました: {message}",
    processStillRunning:
      "終了していない acme.sh プロセスがまだあります: {pids}",
    lockLost:
      "ACME 実行中のロックが失われ、タスクが停止されました。アプリケーションを再度開始してください。",
    lockRefreshFailed: "ACME 実行中のロック更新例外: {message}",
    lockLeaseExpired:
      "{message};ロックイン期間が終了し、タスクが停止されました。アプリケーションを再度開始してください。",
    applicationChangedSkipped:
      "アプリケーションのドメイン名が実行中に変更され、古い証明書の書き込みがスキップされました。アプリケーションを再度開始してください。",
    issuedButApplicationChanged:
      "証明書の発行は成功しましたが、申請項目のドメイン名が変更されているため、現在の申請項目は書き込まれません。",
    issuedButCertReadFailed:
      "証明書は正常に発行されましたが、証明書ファイルの読み取りに失敗しました (後で再試行するか、acme.sh ディレクトリを確認してください)",
    clearedDomainWorkingState:
      "acme.sh ドメイン名の作業ディレクトリがクリーンアップされ、証明書のリストと更新がシステム タスクによって管理されます。",
    clearDomainWorkingStateFailed:
      "証明書は保存されましたが、acme.sh ドメイン名のステータスのクリーニングに失敗しました: {message}",
    linkedLibrarySyncedGateway:
      "関連する証明書ストア エントリが同期され、ゲートウェイ証明書リストが更新されました。",
    linkedLibraryUpdated: "関連する証明書ストア エントリを更新しました",
    addedToLibraryAndSyncedGateway:
      "証明書が正常に発行されると、証明書は自動的に証明書ライブラリに追加され、ゲートウェイ証明書リストが更新されます。",
    addedToLibrary:
      "証明書が正常に発行されると、証明書ストアに自動的に追加されます。",
    addToLibraryFailed:
      "証明書は発行され、保存されましたが、証明書ストアへの自動追加に失敗しました: {message}",
    stoppedIgnoredProcessError:
      "タスクが停止され、プロセス終了後のエラーは無視されました",
  },
  acmeRoutes: {
    domainsInvalid:
      "ドメイン名リストを空にすることはできません、または形式が無効です。",
    dnsTypeRequired: "DNS 検証タイプがありません",
    unsupportedDnsProvider: "サポートされていません DNS サービスプロバイダー",
    missingDnsCredentials:
      "DNS API 認証情報がありません。次のオプションのいずれかを入力してください: {requirements}",
    cloudflareInvalidKey:
      "Cloudflare API キーが正しくありません (X-Auth-Key 形式が無効です)",
    cloudflareInvalidEmail:
      "Cloudflare 電子メールが正しくありません (X-Auth-Email 形式が無効です)",
    cloudflareInvalidHeaders:
      "Cloudflare API リクエストヘッダーが無効です。通常、API キー/電子メールが正しくないことが原因です。",
    acmeFrequencyLimited:
      "適用頻度は制限されています（Retry-After={seconds}秒、600秒以降は再試行されません）。しばらく待ってから再試行してください。",
    dnsApiRateLimited:
      "DNS API 電流制限 (429/レート制限) をトリガーします。後でもう一度お試しください。",
    logUnknownFailure:
      "ログでエラーが検出されましたが、自動的に関連付けられませんでした",
    installingRetryLater:
      "acme.sh がインストールされています。後でもう一度試してください。",
    installFirst: "まず acme.sh をインストールしてください",
    multipleApplicationsUseNewApi:
      "現在複数の申請項目があります。ACME 申請項目を管理するには新しいインターフェースを使用してください。",
    applicationNotFound: "申請項目が存在しません",
    notFound: "見つかりません",
    installingCannotDelete: "acme.sh がインストール中のため削除できません。",
    installingCannotSwitchCa:
      "acme.sh がインストールされているため、一時的に認証局を切り替えることができません。",
    noMatchingIssuedCertificate:
      "現在のアプリケーション項目には、ドメイン名構成に一致する発行済み証明書がありません。",
    success: "成功",
    dns01Only: "はDNS-01検証方法のみをサポートします",
    certNotFound: "証明書が存在しません",
    certOrKeyInvalid: "証明書または秘密鍵が無効です",
  },
  acmeDnsProviders: {
    groups: {
      common: "よく使われる",
      domestic: "国内",
      international: "インターナショナル",
      selfHostedAdvanced: "自作/上級",
    },
    credentialSchemes: {
      default: "デフォルトの認証情報",
    },
    fields: {
      accountEmail: "アカウントのメールアドレス",
      sshPrivateKeyPath: "SSH 秘密鍵ファイルのパス",
    },
    labels: {
      aliyun: "アリババクラウド DNS",
      tencentCloudDnspod: "テンセントクラウド DNSPod (テンセントクラウド)",
      huaweiCloudDns: "ファーウェイクラウド DNS",
      jdCloudDns: "JDクラウドDNS",
      westCn: "ウェスタンデジタル",
    },
    cloudflare: {
      globalKeyDescription:
        "Cloudflare レガシー グローバル API キー方式と互換性があります。",
      apiTokenDescription:
        "おすすめです。トークンを入力するだけです。ゾーン ID またはアカウント ID がわかっている場合は、それらを一緒に入力して自動検出を減らすことができます。",
    },
    gcloud: {
      description:
        "gcloud コマンドと実行環境の承認された構成に応じて異なります。入力されていない場合は、gcloud のデフォルト設定を使用します。",
    },
    azure: {
      managedIdentityDescription:
        "AZUREDNS_MANAGEDIDENTITY trueを入力してください。",
    },
    descriptions: {
      boolean01: "0または1を入力してください。",
      optionalBoolean01: "オプション、0 または 1 を入力します。",
    },
    requirements: {
      optionalSuffix: ";オプション {keys}",
      orSeparator: ";または",
    },
  },
  acmePatches: {
    duckdns: {
      scriptMissing: "見つかりません DuckDNS DNS API スクリプト: {path}",
      proxyApplied: "は DuckDNS API を {from} から {to} に切り替えました",
    },
  },
  reverseProxyTrustedIps: {
    syncFailed:
      "同期アンチジェネレーション スロットリング免除 IP が失敗しました",
  },
  commonAuthLocations: {
    cidrLookupFailed: "CIDR クエリが失敗しました",
    syncFailed:
      "一般的に使用される除外設定をゲートウェイに同期できませんでした",
  },
  fnosDataShare: {
    invalidPath: "不正な共有ファイルパス",
    shareMissing:
      "Feiniu 共有ディレクトリが見つかりませんでした。アプリケーション リソースが正しく構成されていることを確認してください。",
    fileOnly: "は共有ディレクトリ内のファイルのみを読み取ることができます",
    fileTooLarge:
      "ファイルが大きすぎます。証明書または秘密鍵のテキスト ファイルのみを入れてください。",
  },
  autoHttps: {
    listenEacces:
      "にはポート 80 をリッスンする権限がありません。現在のデバイスまたはコンテナーでプログラムが低いポートにバインドできることを確認してください。",
    listenEaddrinuse:
      "ポート 80 が他のプログラムによって占有されているため、自動 HTTPS を開始できません。 Feiniu システム設定、セキュリティ、ポート設定を試して、編集し、ポート 80 と 443 をリダイレクトするチェックを外してください。",
    listenFailedWithMessage: "ポート 80 でのリッスンに失敗しました: {message}",
    listenFailed: "はポート 80 でのリッスンに失敗しました。",
  },
  wafCollector: {
    drainFailed: "WAF イベントの取得に失敗しました",
  },
  hostMappingBookmarks: {
    defaultFolderTitle: "fn-knock サブドメインマッピング",
  },
  whitelist: {
    addFailed: "ホワイトリスト レコードの追加に失敗しました",
    recordNotFound: "ホワイトリスト レコードが見つかりませんでした",
    domainResolveFailed: "ドメイン名解決に失敗しました",
    refreshFailed: "ホワイトリスト レコードをすぐに更新できませんでした",
  },
  whitelistManager: {
    dnsRecordQueryFailedWithCode:
      "{label} レコードのクエリが失敗しました ({code}): {message}",
    dnsRecordQueryFailed: "{label} レコードのクエリが失敗しました: {message}",
    targetFormatInvalid: "IP、CIDR、またはドメイン名の形式が正しくありません",
    autoGrantIpOnly: "ログイン自動認証は 1 つの IP のみをサポートします",
    cidrInvalid: "CIDR 形式が正しくありません",
    domainInvalid: "ドメイン名の形式が正しくありません",
    ipInvalid: "IP 形式が正しくありません",
    autoOwnerMissing: "自動ホワイトリスト属性識別子がありません",
    domainResolveFailed: "ドメイン名解決に失敗しました",
    resolvedIpCount: "解析されました {count} IP",
    noAaaaRecords: "は現在 A / AAAA レコードに解析されていません",
    syncAllowedStateFailed:
      "ドメイン名解決結果は更新されましたが、システム解放ステータスの同期に失敗しました",
  },
  terminal: {
    defaultTitle: "Webターミナル",
    defaultSessionTitlePrefix: "セッション-",
    tmuxNotDetectedInstallFirst:
      "tmux が検出されません。最初に tmux 環境をインストールしてください。",
    tmuxReadyWithVersion: "tmux の準備ができました: {version}",
    refreshingApt: "Debian ソフトウェア ソースを更新しています...",
    aptUpdateFailed: "apt-get アップデートの実行に失敗しました",
    installingTmux: "tmux をインストールしています...",
    aptInstallTmuxFailed: "apt-get tmux のインストールの実行に失敗しました",
    verifyingTmuxInstall: "tmux のインストール結果を確認しています...",
    tmuxMissingAfterInstall: "インストールが完了した後も tmux が検出されない",
    tmuxInstallCompleteWithVersion:
      "tmux のインストールが完了しました: {version}",
    tmuxInstallFailed: "tmux のインストールに失敗しました",
    cwdUnavailable:
      "作業ディレクトリが存在しないか、アクセスできません: {path}",
    webTerminalDisabled: "Webターミナル機能はまだ有効になっていません",
    tmuxInstallingWait:
      "tmux がインストールされています。インストールが完了するまでお待ちください。",
    tmuxStatusError: "tmux ステータス異常: {message}",
    tmuxMissingCannotCreate:
      "tmux が検出されず、復元可能な端末セッションを作成できません",
    rootRunRequiresDangerToggle:
      "現在のプロセスは root として実行されています。ターミナルを作成する前に、設定で高リスク操作スイッチを明示的にオンにする必要があります。",
    requestedShellUnavailable: "要求されたシェルは利用できません: {shell}",
    noShellDetected:
      "利用可能なシェルが検出されませんでした。zsh、bash、または sh がシステムにインストールされていることを確認してください",
    paneMetadataReadFailed: "ターミナルペインのメタデータを読み取れません",
    paneTtyParseFailed: "ターミナルペイン tty を解析できません",
    inputPipeCreateFailed: "ターミナル入力パイプを作成できません",
    ioRelayCreateFailed: "端末IOリレーを確立できません",
    sessionLimitReached: "ターミナルセッションの制限に達しました ({count})",
    tmuxSessionCreateFailed: "tmux セッションの作成に失敗しました",
    sessionTitleRequired: "セッション名を空にすることはできません",
    sessionMissingOrExpired: "ターミナルセッションが存在しないか、期限切れです",
    tmuxMissingCannotAttach:
      "tmux が検出されず、端末セッションを接続できません",
    inputPipeNotReady: "ターミナル入力パイプはまだ準備ができていません",
    inputWriteInterrupted: "端子入力書き込み中断",
    attachmentExpired: "端末の接続に失敗しました",
    inputSendFailed: "端末入力の送信に失敗しました",
    resizeFailed: "端子サイズ調整に失敗しました",
    sessionNotFound: "ターミナルセッションが存在しません",
  },
  waf: {
    manifestInvalid: "システムルールリストの形式が正しくありません",
    manifestMissingZipInfo:
      "システム ルール リストに zip ファイル情報がありません",
    manifestRequestFailed:
      "システム ルール リストのリクエストが失敗しました: HTTP {status}",
    manifestRefreshFailed: "システムルールリストの更新に失敗しました",
    confOnly: ".conf ルール ファイルのみをサポートします",
    ruleFilenameInvalid: "ルールファイル名が間違っています",
    fileTooLarge: "{filename}が1MBを超えています",
    fileInvalidUtf8: "{filename} は有効な UTF-8 テキストではありません",
    filesystemDirectiveBlocked:
      "{filename} には、許可されていないファイル システム ディレクティブが含まれています",
    systemRuleDescription: "システムセキュリティルール",
    customRuleDescription: "ユーザーアップロードルール",
    enableNeedsRule:
      "開く前に少なくとも 1 つの WAF ルール ファイルを有効にしてください",
    rulesLoadFailed: "WAF ルールのロードに失敗しました",
    configSyncFailed: "同期 WAF をゲートウェイに設定できませんでした",
    sourceInvalid: "ルールのソースが正しくありません",
    ruleFileNotFound: "ルールファイルが存在しません",
    zipInvalid: "システムルールのzip形式が正しくありません",
    zipDirectoryInvalid: "システム ルールの zip ディレクトリが正しくありません",
    zipUnpackedTooLarge: "解凍後のシステム ルール パッケージが大きすぎます",
    zipHeaderInvalid:
      "システム ルールの zip ファイル ヘッダーが正しくありません",
    zipMethodUnsupported:
      "は現在、zip 圧縮方式 {method} をサポートしていません。",
    zipSizeInvalid: "システムルールのzipファイルサイズが正しくありません",
    zipPathInvalid:
      "システム ルールの zip ファイル パスが正しくありません: {path}",
    downloadFailed:
      "システム ルールのダウンロードに失敗しました: HTTP {status}",
    zipTooLarge: "システム ルール パッケージが大きすぎます",
    zipHashMismatch: "システム ルール パッケージの検証に失敗しました",
    zipEmpty: "システム ルール パッケージが空です",
    zipDuplicateFile:
      "システム ルール パッケージに重複したファイルがあります: {path}",
    zipConfRootOnly:
      "システム ルール パッケージの .conf ファイルはルート ディレクトリに配置する必要があります",
    zipNoConf: "システム ルール パッケージに .conf ファイルがありません",
    systemRulePathInvalid: "システムルールファイルのパスが正しくありません",
    manifestEmpty: "システムルールリストが空です",
    keepOneEnabledRule:
      "WAF がオンの場合、有効なルール ファイルを少なくとも 1 つ保持します",
    uploadSelectConf: "アップロードする .conf ファイルを選択してください",
    reloadRulesFailed: "WAF ルールのリロードに失敗しました",
    statusReadFailed: "WAF ステータスの読み取りに失敗しました",
    configSaveOrLoadFailed: "WAF 設定の保存または読み込みに失敗しました",
    systemRulesSyncFailed: "システムルールの同期に失敗しました",
    ruleToggleFailed: "WAF ルールの開始と停止に失敗しました",
    ruleReadFailed: "WAF ルールの読み取りに失敗しました",
    customRuleUploadFailed: "カスタムルールのアップロードに失敗しました",
    customRuleDeleteFailed: "カスタムルールの削除に失敗しました",
    eventsDrainFailed: "WAF イベントの取得に失敗しました",
    logsQueryFailed: "WAF ログのクエリに失敗しました",
    logNotFound: "WAF ログが存在しません",
    logsDeleteFailed: "WAF ログの削除に失敗しました",
  },
  oidc: {
    callbackStateExpired:
      "ログインステータスの有効期限が切れています。再度ログインしてください。",
    loginFailedRetry: "外部ログインに失敗しました。もう一度お試しください。",
    reservedExtraAuthParam:
      "extra_auth_params には OIDC 予約パラメータ: {key} が含まれます",
    urlInvalid: "{label} は合法である必要があります URL",
    urlMustUseHttps: "{label} は HTTPS を使用する必要があります",
    providerUnsupported: "サポートされていない外部ログインプロバイダー",
    providerMissingRequiredConfig: "{provider} 必要な構成がありません {fields}",
    providerMissingRequiredFields:
      "外部ログインプロバイダーに必要な構成がありません {fields}",
    accessTokenMissing: "未取得 access_token",
    idTokenMissing: "未取得 id_token",
    callbackUrlBuildFailed:
      "外部ログイン コールバック アドレスを生成できません。public_auth_base_url を設定してください",
    issuerMissing: "OIDC 発行者が構成されていません",
    discoveryMissingFields: "OIDC 証拠書類に必須フィールドがありません",
    nonceCheckFailed: "OIDC ノンス検証に失敗しました",
    issuerCheckFailed: "OIDC 発行者の検証に失敗しました",
    subjectEmpty: "OIDC 件名が空です",
    githubUserIdEmpty: "GitHub ユーザー ID は空です",
    providerNotFound: "外部ログインプロバイダーが存在しません",
    connectionTestSuccess: "接続テストに成功しました",
    oauthEndpointIncomplete: "OAuth2 エンドポイントが完全に構成されていません",
    connectionTestFailed: "接続テストに失敗しました",
    totpMissing: "TOTP 認証情報が存在しません",
    selectProvider: "外部ログインプロバイダーを選択してください",
    providerUnavailable: "外部ログインプロバイダーは利用できません",
    bindingNotFound: "外部アカウント バインドが存在しません",
    inviteInvalid: "バインディング招待リンクが無効です",
    inviteExpired: "バインディング招待リンクの有効期限が切れました",
    inviteProviderNotAllowed:
      "この招待リンクはこのプロバイダーの使用を許可されていません",
    authorizationEndpointMissing: "認証エンドポイントが構成されていません",
    bindStateInvalid: "バインディング招待ステータスが無効です",
    accountNotBoundCannotLogin:
      "この外部アカウントはバインドされていないため、ログインできません。",
    tokenEndpointMissing: "トークンエンドポイントが構成されていません",
    bindProviderMismatch:
      "バインディング招待状がログインプロバイダーと一致しません",
    inviteTotpMissing:
      "バインディング招待状に関連付けられた TOTP は存在しなくなりました",
    accountAlreadyBoundOtherTotp:
      "この外部アカウントは別の TOTP にバインドされています",
    inviteUsed: "バインディング招待リンクが使用されました",
    providerErrors: {
      accessDenied:
        "外部ログイン認証をキャンセルしたか、プロバイダーによって認証リクエストが拒否されました。",
      temporarilyUnavailable:
        "外部ログインサービスは一時的に利用できません。しばらくしてからもう一度お試しください。",
      serverError:
        "外部ログインプロバイダーがサービスエラーを返しました。後でもう一度試してください。",
      invalidScope:
        "外部ログイン許可範囲が正しく構成されていません。管理者に連絡してプロバイダーの構成を確認してください。",
      rejected:
        "外部ログイン要求はプロバイダーによって拒否されました。外部ログイン設定を確認して、もう一度お試しください。",
      incomplete:
        "外部ログインが完了していません。ログインをやり直してください。",
    },
    bindWithProvider: "{provider}を使用してバインドします",
    selectProviderTitle: "外部アカウントプロバイダーを選択してください",
    bindToTotp: "外部アカウントを {totp} にバインドします。",
    linkMissingToken: "リンクにトークンがありません。",
    inviteMissingExpiredUsed:
      "招待状が存在しないか、有効期限が切れているか、すでに使用されています。",
    noProvidersTitle: "利用可能な外部ログインプロバイダーはありません",
    noProvidersBody:
      "現在、この招待にはバインドできる外部アカウント プロバイダーがありません。",
    bindFailedTitle: "外部アカウントのバインドに失敗しました",
    bindStartFailed: "外部アカウントのバインドを開始できません。",
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
    deleteBindingFailed: "外部アカウント バインドを削除できませんでした",
    createInviteFailed: "バインディング招待状の作成に失敗しました",
    catalog: {
      googleDescription: "Google アカウントでサインインします。",
      microsoftDescription: "Microsoft / Azure AD アカウントでログインします。",
      githubDescription: "GitHub OAuth を使用してログインします。",
      customLabel: "カスタム OIDC",
      customDescription:
        "標準の OpenID Connect Discovery を使用するカスタムプロバイダー。",
    },
  },
  subdomainMode: {
    recommendationMissingBase:
      "ルート ドメイン名または認証サービスが設定されていないため、現時点では推奨される証明書ドメイン名を生成できません。",
    recommendationWildcardSummary:
      "推奨アプリケーション {rootDomain} および *.{rootDomain} は、同じ親ドメイン内のルート ドメイン名、認証サービス、およびビジネス サブドメインをカバーするために使用されます。",
    authOutOfRootWarning:
      "現在の認証サービス {authHost} はルート ドメイン名 {rootDomain} の下になく、追加の正確なドメイン名が追加されています。選択した DNS サービス プロバイダーがこれらのドメイン名を管理できることを確認してください。",
    recommendationSingleHostSummary:
      "ルートドメイン名が設定されていません。現在、認証サービス {authHost} として推奨できるのは、単一のドメイン名証明書を申請する場合のみです。",
    wildcardSuggestion:
      "将来的に複数のビジネス サブドメインをカバーしたい場合は、最初にルート ドメイン名を補足してから、ワイルドカード証明書を申請することをお勧めします。",
    configureRootOrAuth:
      "最初にサブドメインモードでルートドメイン名を設定するか、ホストマッピングで認証サービスを指定してください。",
    authMissingWarning:
      "認証サービスは指定されておらず、現在の推奨結果はルート ドメイン名に基づいてのみ導出されています。",
    uncoveredHostMappingsWarning:
      "現在、推奨される証明書でカバーされていないホスト マッピングが {count} あります。これらを外部に公開する必要がある場合でも、追加の証明書またはドメイン名の計画が必要になります。",
    coverageNoSsl:
      "SSL 証明書は現在有効になっておらず、認証サービスとビジネス サブドメインは HTTPS の対象になっていません。",
    coverageReadyConcrete:
      "現在展開されている証明書は、認証サービスと構成されているすべてのホスト マッピングをカバーしています。",
    coverageReadyRecommended:
      "現在展開されている証明書は、サブドメイン モードで現在推奨されている範囲を満たしています。",
    coveragePartialConcrete:
      "現在の証明書は、サブドメイン モードで必要なドメイン名の一部のみをカバーしており、認証サービスまたは一部のビジネス ホストで証明書の不一致が依然として存在する可能性があります。",
    coveragePartialRecommended:
      "現在の証明書は一部の推奨ドメイン名のみをカバーしており、後でサブドメイン モードを有効にした場合でも証明書の不一致が発生する可能性があります。",
    coverageMismatchConcrete:
      "現在展開されている証明書はサブドメイン モードと一致せず、認証サービスとビジネス ホストが正しくカバーされていません。",
    coverageMismatchRecommended:
      "現在展開されている証明書は、サブドメイン モードで推奨されるドメイン名の範囲をカバーしていません。",
    coverageMissingRequiredWarning:
      "現在の証明書には、{count} 必要な補償項目がまだ不足しています。証明書を再適用または置き換えることをお勧めします。",
    coverageMissingRecommendedWarning:
      "現在の証明書には、{count} 推奨されるドメイン名適用項目がまだ不足しています。将来これらのドメイン名を使用する必要がある場合は、証明書を再適用または置き換えることをお勧めします。",
    coverageAuthHostMissingWarning:
      "現在の証明書は認証サービス {authHost} をカバーしていません。",
    inventoryEmpty:
      "証明書ストアにはサブドメイン モードで使用できる証明書がまだありません。",
    inventoryActiveReady:
      "現在アクティブな証明書は、サブドメイン モードで必要なドメイン名を完全にカバーしています。",
    inventoryOneReady:
      "証明書ストアには、サブドメイン モードを完全にカバーでき、アクティブな証明書に直接切り替えることができる証明書が 1 つあります。",
    inventoryMultipleReady:
      "証明書ストアには {count} の証明書があり、それぞれが現在のサブドメイン モードを完全にカバーできます。",
    inventoryCombinedReady:
      "証明書ライブラリは、結合後に完全なカバレッジ機能を備えています。",
    inventoryCandidateReady:
      "現在のサブドメイン モードをカバーできる候補証明書が証明書ストアにすでに存在します。",
    inventoryCombinedNeedsMultiSni:
      "証明書ライブラリの組み合わせで現在のサブドメイン モードをすでにカバーできますが、現在のゲートウェイはまだ単一アクティブ証明書モードのままであり、同時に有効にすることはできません。",
    inventoryPartialCandidates:
      "証明書ストアにはすでに候補証明書がいくつかありますが、認証サービスとすべてのホスト マッピングを完全にカバーすることはできません。",
    inventoryNoCertificateCoversRecommendation:
      "現在、サブドメイン モードで推奨されるドメイン名をカバーできる証明書はありません。",
    inventoryMultiCertRequiresSniWarning:
      "現在の証明書ライブラリでは、複数の証明書を共同でカバーする必要がありますが、ゲートウェイは依然として単一アクティブ証明書モードであり、一度にすべてを有効にすることはできません。",
    inventorySwitchRecommendedWarning:
      "現在アクティブな証明書はサブドメイン モードと正確に一致しません。推奨される証明書に切り替えることをお勧めします。",
    inventoryBetterForSniWarning:
      "既存の証明書ストアは、後続のマルチ証明書/SNI の展開により適しています。",
  },
  cloudflared: {
    missingToken: "最初にCloudflareトークンを設定してください",
    startFailedWithDetail: "Cloudflared の起動に失敗しました: {detail}",
    processExited: "クラウドフレアプロセスが終了しました",
    processExitedWithCode:
      "Cloudflared プロセスが終了しました (終了コード {code})",
    processCrashed: "クラウドフレアプロセスが異常終了しました: {message}",
    resumeOnBoot:
      "再開: 前回 Cloudflared がオンであったことが検出され、自動的に回復中です...",
    unknownError: "不明なエラー",
    notInitialized: "Cloudflared が初期化されていません",
    startFailed: "起動に失敗しました",
  },
  dnsmasq: {
    notDetectedInstallFirst:
      "dnsmasq が検出されません。最初にインストールを完了してください。",
    dnsPortUnavailable:
      "DNS 53 ポートは使用できません。ポートを解放して再試行してください。",
    dnsPortUnavailableWithDetail:
      "DNS 53 ポートが利用できません。ポートを解放して再試行してください: {detail}",
    detectedWithVersion:
      "dnsmasq が検出されました: {version}、初期化またはサービスの起動を待機しています",
    detected:
      "dnsmasq が検出されました。サービスの初期化または開始を待機しています。",
    missingServiceAutoComplete:
      "システムサービスが欠落しているため、初期化中に自動的に完了します。",
    servicePackageMissing:
      "dnsmasq 実行可能ファイルは検出されましたが、システム サービスがインストールされていません。最初に dnsmasq パッケージをインストールしてください",
    completingService: "dnsmasq システム サービスを完了しています...",
    completeServiceFailed: "dnsmasq システム サービスの完全な障害",
    serviceDefinitionMissingAfterInstall:
      "dnsmasq サービスのインストール後に、利用可能なシステム サービス定義が検出されません。",
    executableMissing: "dnsmasq 実行可能ファイルが検出されない",
    configTestFailed: "dnsmasq 構成の検証に失敗しました",
    restartFailed: "dnsmasq の再起動に失敗しました",
    serviceDefinitionMissing:
      "dnsmasq システムサービス定義が検出されません。サービス環境を完成させるために、まず初期化を完了してください。",
    readyWithVersion: "dnsmasq の準備が完了しました: {version}",
    ready: "dnsmasq の準備ができました",
    refreshingApt: "Debian ソフトウェア ソースを更新しています...",
    aptUpdateFailed: "apt-get アップデートの実行に失敗しました",
    installing: "dnsmasq をインストールしています...",
    aptInstallFailed: "apt-get dnsmasq のインストールの実行に失敗しました",
    enablingService: "dnsmasq サービスを有効にしています...",
    verifyingService: "dnsmasq サービスを確認しています...",
    installMissingAfterComplete: "dnsmasq が検出されません",
    installFailed: "dnsmasq のインストールに失敗しました",
    checkingEnvironment: "dnsmasq 環境を確認しています...",
    validatingConfig: "dnsmasq 構成を確認しています...",
    startingService: "dnsmasq サービスを開始しています...",
    initializeFailed: "dnsmasq の初期化に失敗しました",
  },
  firewall: {
    goBackendCallFailed:
      "Go バックエンド インターフェイス呼び出しが失敗しました: {message}",
    clearLegacyTcpRedirectFailed:
      "履歴クリア TCP リダイレクト {listenPort} -> {targetPort} 失敗",
    initDefaultRulesFailed:
      "デフォルトのファイアウォール ルールの初期化に失敗しました",
    syncWhitelistTargetFailed:
      "ホワイトリスト ターゲット {target} の同期に失敗しました",
    cleanRulesFailed: "ファイアウォール ルールをクリアできませんでした",
    syncAuthGatewayConfigFailed: "認証ゲートウェイ構成の同期に失敗しました",
    syncReverseProxyThrottleFailed:
      "同期アンチジェネレーションスロットル構成が失敗しました",
    syncGatewayVisibilityConfigFailed:
      "ゲートウェイ可視性構成の同期に失敗しました",
    syncGatewayProxyHeadersConfigFailed:
      "同期ゲートウェイプロトコルヘッダーの構成に失敗しました",
    syncGatewayHostResponseConfigFailed:
      "同期ゲートウェイ ホスト応答設定に失敗しました",
    enableProxyProtocolForceFailed:
      "プロキシプロトコル強制モードを有効にできませんでした",
    disableProxyProtocolForceFailed:
      "プロキシ プロトコル強制モードを終了できませんでした",
    disableStreamRulesFailed:
      "プロトコル マッピングの監視をオフにできませんでした",
    flushPathRoutesFailed: "パスルーティングのクリアに失敗しました",
    syncHostRoutesFailed: "ホストルートの同期に失敗しました",
    syncDefaultRouteFailed: "デフォルトルートの同期に失敗しました",
    flushHostRoutesFailed: "ホストルートのクリアに失敗しました",
    syncPathRoutesFailed: "パスルーティングの同期に失敗しました",
    syncStreamRulesFailed: "同期プロトコルのマッピングに失敗しました",
    syncAuthEntryRouteFailed: "認証エントリルートの同期に失敗しました",
    syncAuthDefaultRouteFailed: "同期認証のデフォルトルートに失敗しました",
  },
  updateManager: {
    manifestFieldInvalid: "更新情報 {field} 無効",
    manifestFormatInvalid: "更新情報フォーマットエラー",
    manifestMissingVersion: "アップデート情報にバージョンがありません",
    manifestMissingUpdateAvailable: "更新情報がありません update_available",
    manifestMissingForceUpdate: "更新情報がありません force_update",
    manifestMissingDownloadUrl: "更新情報がありません download_url",
    manifestArm64FieldsIncomplete:
      "アップデート情報 ARM64 ダウンロード欄が不完全です",
    architectureUnsupported:
      "現在のシステム アーキテクチャは自動更新をサポートしていません: {arch}",
    manifestMissingArm64DownloadUrl:
      "アップデート情報がありません ARM64 ダウンロードアドレス",
    manifestMissingArm64Checksum: "更新情報がありません ARM64 チェック値",
    checkHttpFailed: "更新チェックに失敗しました: HTTP {status}",
    checkFailed: "アップデートチェックに失敗しました",
    noUpdateInfo: "アップデート情報はまだ取得されていません",
    featureDisabled: "アップデート機能は現在有効になっていません",
    alreadyLatest: "が現在の最新バージョンです",
    downloadHttpFailed: "ダウンロードに失敗しました: HTTP {status}",
    responseBodyUnreadable: "ダウンロード失敗: 応答ストリームを読み取れません",
    checksumFailed: "検証失敗: 期待値 {expected}、実際の値 {actual}",
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
        "MAC プラットフォームは現在、アプリケーションの自動ダウンロードをサポートしていません。 brew install Cloudflared を通じて手動でインストールしてください。",
      platformUnsupported: "現在のプラットフォームはサポートされていません",
      responseBodyUnreadable: "ダウンロード応答本文が読めません",
      downloadCancelled: "ダウンロードがキャンセルされました",
      unknownError: "不明なエラー",
      macManualRemove: "MAC プラットフォームを手動で削除してください",
      notInstalledBrew:
        "Cloudflared がインストールされていません。最初に brew install Cloudflared を通じてインストールしてください。",
      notInitialized:
        "Cloudflared は初期化されていません。最初にダウンロードしてください",
    },
    frp: {
      platformUnsupported: "現在のプラットフォームはサポートされていません",
      packageMissing: "FRP インストールパッケージがありません",
      extractFailed: "解凍に失敗しました。終了コード {code}",
      responseBodyUnreadable: "ダウンロード応答本文が読めません",
      connectionFailed: "接続に失敗しました",
      downloadFailed: "ダウンロードに失敗しました: {detail}",
      unknownError: "不明なエラー",
      downloadCancelled: "ダウンロードがキャンセルされました",
      notInitialized:
        "FRP 初期化されていません。最初にダウンロードしてください",
    },
  },
  frpc: {
    instanceNotFound: "FRP インスタンスが存在しません: {id}",
    instanceLimitExceeded:
      "追加の FRP インスタンスが {limit} までサポートされます",
    primaryName: "メインFRP",
    instanceName: "FRP 例",
    verifyFailedWithDetail: "frpc verify 検証失敗: {detail}",
    verifyFailedWithCode: "frpc verify 検証失敗、終了コード {code}",
    verifyFrpNotInitialized:
      "FRPは初期化されていないため検証できません。 frpc.toml、まずシステム設定でFRPリソースをダウンロードしてください。",
    pidInvalidForInstance:
      "PID は有効期限が切れているか、このインスタンスに属していません",
    processExited: "frpc プロセスは終了しました",
    processExitedWithCode: "frpc プロセスが終了しました (終了コード {code})",
    processCrashed: "frpc プロセスが異常終了しました: {message}",
    processStillRunning: "FRP プロセスはまだ終了していません pid={pid}",
    primaryDeleteDenied: "マスター FRP インスタンスは削除できません",
    notInitialized: "FRP 初期化されていません",
    startFailedWithDetail: "起動 frpc が失敗しました: {detail}",
    pidCleanedForInstance:
      "PID はこのインスタンスに属しません。このインスタンスの実行記録はクリアされました。",
    resumeOnBoot:
      "再開: FRP インスタンスが前回オープン状態であったことが検出され、自動的に回復中です...",
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
    passwordTooShort:
      "管理パネルのパスワードには少なくとも 6 桁の数字が必要です",
    passwordTooLong:
      "管理パネルのパスワードは 128 文字を超えることはできません",
    passwordWhitespace:
      "管理パネルのパスワードには空白文字を含めることはできません",
    passwordNeedsLettersAndNumbers:
      "管理パネルのパスワードには文字と数字の両方を含める必要があります",
    passwordAlreadyConfigured: "管理パネルのパスワードが設定されました",
    passwordNotConfigured: "現在、管理パネルのパスワードは設定されていません。",
    newPasswordSameAsCurrent:
      "新しいパスワードは現在のパスワードと同じにすることはできません",
    resetHelp:
      "fn-knock 管理パネルパスワードリセットツール\n\n使用法:\n  fn-knock-リセットパネルパスワード\n\n機能:\n  - 管理パネルのパスワードをクリア\n  - すべての管理パネルのログインセッションをクリアします\n  - ログイン失敗退避ステータスのクリア\n\n実行完了後、次回管理ポータルにアクセスした際に、再度「初回パスワード設定」の作業が行われます。",
    resetCleared:
      "[fn-knock] 管理パネルのパスワードステータスがクリアされました",
    resetNextVisit:
      "[fn-knock] 次回管理ポータルにアクセスするときに、管理パネルのパスワードをリセットする必要があります。",
    resetFailed: "[fn-knock] 管理パネルのパスワードをクリアできませんでした:",
  },
  passkeyRoutes: {
    notFoundWithRetry:
      "Passkey が見つかりません。{seconds} 秒後にもう一度お試しください",
    verifyFailedWithRetry:
      "認証に失敗しました。{seconds} 秒後にもう一度お試しください",
    bindTokenExpired: "結合証明書の有効期限が切れています",
  },
  maintenanceBackup: {
    commandMissing: "システム環境に {command} コマンドがありません",
    commandFailed: "{command} コマンドの実行に失敗しました",
    commandCheckFailed: "検出 {command} コマンドが失敗しました",
    commandsMissingNoApt:
      "システム環境に {commands} コマンドが不足しており、Debian apt-get が見つからず、自動的にインストールできません。",
    commandsMissingNoPackageManager:
      "システム環境に {commands} コマンドがなく、opkg または Debian apt-get が見つからず、自動インストールできません。",
    opkgUpdateFailed: "opkg 更新の実行に失敗しました",
    aptUpdateFailed: "apt-get アップデートの実行に失敗しました",
    packageInstallFailed: "インストール {packages} が失敗しました",
    commandsStillMissingAfterInstall:
      "自動インストールが完了しても、{commands} コマンドが検出されません。",
    commandErrorWithDetail: "{message} (終了コード: {code}): {detail}",
    commandError: "{message} (終了コード: {code})",
    shareDirectoryMissing:
      "Feiniu 共有ディレクトリが見つかりませんでした。アプリケーション リソースが正しく構成されていることを確認してください。",
    invalidBackupPath: "バックアップファイルのパスが不正です",
    invalidRedisStreamData:
      "Redis ストリーム データ形式が無効です: {key} ({id})",
    unsupportedRedisExportType:
      "は、エクスポートされた Redis データ型: {type} ({key}) をサポートしていません。",
    createArchiveFailed: "バックアップ アーカイブの生成に失敗しました",
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
    entryTypeUnsupported: "エントリ[{index}].type はサポートされていません",
    entryTtlInvalid:
      "エントリ[{index}].ttl_ms は正の整数または null でなければなりません",
    entryValueStringRequired:
      "エントリ[{index}].value は文字列である必要があります",
    jsonParseFailed: "バックアップ ファイル JSON を解析できません",
    payloadObjectInvalid:
      "バックアップ ファイルの内容は有効なオブジェクトではありません",
    unsupportedSchemaVersion:
      "は、バージョン = {version} のバックアップ ファイルのみをサポートします",
    unsupportedPrefix:
      "は、{prefix} プレフィックスが付いたバックアップ ファイルのみをサポートします",
    missingAppVersion: "バックアップ ファイルがありません app_version",
    appVersionUnsupported:
      "現在のバージョン {currentVersion} では、{range} の範囲内でエクスポートされ、{appVersion} を受信したバックアップのインポートのみが許可されます",
    missingExportedAt: "バックアップファイルがありません exported_at",
    missingEntries: "バックアップ ファイルにエントリ配列がありません",
    duplicateRedisKey: "バックアップ ファイルに重複した Redis キーがあります",
    archiveMissingPayload: "バックアップ アーカイブに {filename} がありません",
    archivePasswordInvalid:
      "バックアップ アーカイブのパスワード検証に失敗しました",
    readArchiveFailed: ".knock バックアップ アーカイブの読み取りに失敗しました",
    writeRedisFailed: "Redis バックアップ データの書き込みに失敗しました",
    unknownError: "不明なエラー",
    syncSteps: {
      runModeGatewayRoutes: "動作モードとゲートウェイルーティング",
      directModeWhitelist: "ダイレクトモードのホワイトリスト",
      gatewayLogging: "ログ構成のリクエスト",
      sslDeployment: "SSL 証明書の展開",
      legacyAuthLogCleanup: "放棄されたログインログのクリーニング",
      systemResourceMonitorReset: "システムリソース監視ステータスリセット",
    },
    archiveEmpty: "バックアップ アーカイブのコンテンツが空です",
    directoryImportFileOnly:
      "はバックアップ ディレクトリ内のファイルのみをインポートできます",
    directoryImportExtensionOnly:
      "は、{extension} バックアップ ファイルのインポートのみをサポートします",
    directoryImportTooLarge:
      "バックアップ ファイルが大きすぎるため、Feiniu ディレクトリからインポートできません。",
    archiveContentMissing: "バックアップ アーカイブ コンテンツが欠落しています",
    archiveBase64Invalid:
      "バックアップ アーカイブは有効な Base64 データではありません",
  },
  captcha: {
    powServerNotConfigured:
      "PoW 検証コードはサーバー設定をまだ完了していません。",
    providerMismatch: "認証コードの種類が一致しません",
    turnstileNotConfigured:
      "現在の回転式改札口の構成はまだ完了していません。パラメータを完了するには管理者に問い合わせてください。",
    turnstileSecretMissing: "Cloudflare 改札口 secret_key 未設定",
    turnstileTokenRequired: "改札口トークンを空にすることはできません",
    turnstileServiceUnavailable:
      "回転式改札口認証サービスは一時的に利用できません",
    turnstileVerifyFailedWithReason:
      "回転式改札口の検証に失敗しました: {reason}",
    turnstileVerifyFailed: "回転式改札口の検証に失敗しました",
    providerUnavailable:
      "利用可能な確認コードプロバイダーが見つかりませんでした",
    powNotEnabled: "PoW 確認コードは現在有効になっていません",
    powUnavailable: "現在の PoW 検証コードは利用できません",
    providerConfigMismatch: "確認コードプロバイダーが現在の構成と一致しません",
  },
  cidr: {
    serviceError: "CIDR サービス異常",
    emptyResponse: "<空の応答>",
    upstreamUrl: "上流アドレス: {url}",
    status: "ステータス: {status}{statusText}",
    contentType: "タイプ: {contentType}",
    upstreamCode: "アップストリームコード: {code}",
    upstreamMessage: "アップストリームニュース: {message}",
    requestId: "リクエストID: {requestId}",
    responsePreview: "回答概要: {preview}",
    provinceRequired: "州を空にすることはできません",
    upstreamTimeout: "CIDR アップストリームリクエストのタイムアウト",
    upstreamRequestFailed:
      "CIDR アップストリームリクエストが失敗しました ({status})",
    invalidJson: "CIDR アップストリームが無効な JSON を返しました",
    upstreamUnexpected: "CIDR アップストリームは例外を返します",
    provinceWideLabel: "{province}県",
  },
  dashboard: {
    inbound: "インバウンド",
    outbound: "下り",
    upstreamUnavailable: "上りサービスが利用できません",
    hostRequired: "ホストを空にすることはできません",
  },
  acme: {
    alreadyInstalled: "acme.shがインストールされました",
    installInProgress: "インストールタスクが進行中です",
    installSubmitted: "インストールタスクが送信されました",
    issueSucceeded: "証明書が正常に発行されました",
  },
  ddns: {
    ipv6OnlyUnavailable:
      "現在の更新スコープは IPv6 のみを更新していますが、使用可能な IPv6 アドレスが検出されません",
    ipv4OnlyUnavailable:
      "現在の更新スコープは IPv4 のみを更新していますが、使用可能な IPv4 アドレスが検出されませんでした",
    dualStackUnavailable:
      "現在の更新範囲内に使用可能な IPv4 または IPv6 アドレスはありません",
    domainConfigIncomplete: "ドメイン名の設定が不完全です",
    domainNotInZone:
      "ドメイン名 {fqdn} はルート ドメイン {zone} に属していません",
    invalidJsonResponse: "応答が無効です JSON: {text}",
    aRecordFailed: "レコード処理に失敗しました",
    aaaaRecordFailed: "AAAA レコード処理に失敗しました",
    providerDnsUpdateSuccess: "{provider} DNS 正常に更新されました",
    aliyunParamKeyMissing:
      "Alibaba Cloud リクエストパラメータにキー名がありません",
    requestFailed: "リクエストが失敗しました",
    tencentMissingResponse:
      "HTTP {status}: Tencent Cloud API 応答がありません 応答",
    invalidHeaderFormat: "無効なヘッダー形式: {header}",
    interfaceSourceLabel: "ネットワークカード {name}",
    selectedInterfaceSourceLabel: "選択されたネットワークカード",
    publicSourceLabel: "パブリックネットワーク",
    staticSourceLabel: "静的 IP",
    domainSourceLabel: "ドメイン名 {domain}",
    domainSourceLabelEmpty: "ソースドメイン名",
    staticIpv4Invalid: "静的 IPv4 無効なアドレス: {value}",
    staticIpv6Invalid: "静的 IPv6 無効なアドレス: {value}",
    sourceDomainRequired: "解決するソースドメイン名を入力してください",
    sourceDomainInvalid: "ソース ドメイン名の形式が無効です: {domain}",
    sourceDomainResolveFailed: "ソースドメイン名 {domain} 解決失敗: {error}",
    singleAddressProviderUnsupported:
      "{provider} 一度に更新できるアドレスは 1 つだけです。更新範囲を IPv4 または IPv6 のみに設定してください",
    interfaceIpv6Unavailable:
      "現在の取得方法はネットワーク カードから直接取得することですが、選択したネットワーク カードには利用可能な IPv6 アドレスがありません",
    interfaceIpv4Unavailable:
      "現在の取得方法はネットワーク カードから直接取得することですが、選択したネットワーク カードには利用可能な IPv4 アドレスがありません",
    interfaceDualStackUnavailable:
      "現在の取得方法はネットワーク カードから直接取得することですが、選択したネットワーク カードには利用可能な IPv4 または IPv6 アドレスがありません。",
    publicIpv6Unavailable:
      "現在の取得方法は公衆ネットワークからの取得ですが、利用可能なIPv6アドレスが取得できません",
    publicIpv4Unavailable:
      "現在の取得方法は公衆ネットワークからの取得ですが、利用可能なIPv4アドレスが取得できません",
    publicDualStackUnavailable:
      "現在の取得方法は公衆ネットワークからの取得ですが、利用可能なIPv4またはIPv6のアドレスは取得できておりません。",
    staticIpv6Unavailable:
      "現在の取得方法は静的 IP ですが、使用可能な IPv6 アドレスが入力されていません",
    staticIpv4Unavailable:
      "現在の取得方法は静的 IP ですが、使用可能な IPv4 アドレスが入力されていません",
    staticDualStackUnavailable:
      "現在の取得方法は静的 IP ですが、使用可能な IPv4 または IPv6 アドレスが入力されていません",
    domainIpv6Unavailable:
      "現在の取得方法はドメイン名解決ですが、利用可能なIPv6アドレスは解決されていません。",
    domainIpv4Unavailable:
      "現在の取得方法はドメイン名解決ですが、利用可能なIPv4アドレスは解決されていません。",
    domainDualStackUnavailable:
      "現在の取得方法はドメイン名を解決することですが、利用可能な IPv4 または IPv6 アドレスが解決されていません",
    selectInterfaceAddress:
      "をネットワーク カードから直接取得する場合は、最初に {family} アドレスを選択してください",
    selectedInterfaceAddressUnavailable:
      "選択したネットワーク カードの {index} 番目 {family} のアドレスは使用できなくなりました。再度選択してください。",
    ipv4FailedContinueIpv6:
      "IPv4 取得に失敗しました。引き続き使用します IPv6 ({error})",
    ipv4Failed: "IPv4 取得に失敗しました ({error})",
    ipv6FailedContinueIpv4:
      "IPv6 取得に失敗しました。引き続き使用します IPv4 ({error})",
    ipv6Failed: "IPv6 取得に失敗しました ({error})",
    publicIpv6NotSelectable:
      "パブリック ネットワークによって検出された IPv6 ({ip}) は、ローカル マシンまたは Docker ホストのオプションのネットワーク カード アドレスにありません。外部ネットワークがアドレスにアクセスできない場合は、代わりに「ネットワーク カードから直接取得」を使用し、ホストのパブリック ネットワークを選択してください IPv6",
    interfaceRequired:
      "ネットワーク カードから直接取得する場合は、まず送信ネットワーク カードを明示的に選択する必要があります",
    interfaceNotFound: "利用可能なネットワーク カードが見つかりません: {name}",
    dockerHostInterfaceLabel: "ホスト {name} ({summary})",
    curlStatusLineParseFailed: "CURL 応答ステータス行を解析できません: {line}",
    curlNoHeaders: "curl は応答ヘッダーを返しませんでした",
    requestCanceled: "リクエストがキャンセルされました",
    curlRequestFailed: "curl リクエストが失敗しました: {detail}",
    triggerCron: "定期点検",
    triggerEnable: "自動アップデートを有効にした後、今すぐ確認してください",
    triggerMessage: "{trigger}: {message}",
    notConfigured: "未設定",
    skippedNoProvider:
      "DDNS プロバイダーが選択されていないため、スキップされました",
    skippedIncompleteConfig: "現在の構成は不完全であるためスキップされました",
    skippedPublicIpUnavailable:
      "パブリックネットワークIPを取得できません、スキップされました",
    skippedReason: "{reason}、スキップ",
    targetIpNoChange: "ターゲットIP 変更なし、更新不要",
    none: "なし",
    ipChange: "{family}: {before} -> {after}",
    targetIpChanged: "ターゲットを検出しました IP 変更: {changes}",
    dnsUpdateSuccess: "DNS が正常に更新されました [{provider}]: {message}",
    dnsUpdateFailed: "DNS 更新に失敗しました [{provider}]: {message}",
    taskError: "タスク例外: {message}",
    intervalOutOfRange:
      "自動同期頻度は、{min} ～ {max} の間の整数の分数である必要があります。",
    primaryDomainName: "メインドメイン",
    noProviderSelected: "プロバイダーが選択されていません",
    duplicateTarget:
      "同じプロバイダーとドメイン ダイジェストにはすでに DDNS のエントリがあります",
    primaryInitFailed: "プライマリドメイン DDNS エントリの初期化に失敗しました",
    primaryDomainScope: "メインドメイン",
    additionalDomainScope: "追加フィールド",
    targetNotFound: "DDNS エントリが見つかりませんでした",
    unknownProvider: "不明 DDNS プロバイダー: {provider}",
    primaryDeleteForbidden: "メインドメインエントリは削除できません",
    primaryDisableForbidden:
      "メインドメインエントリを個別に無効化することはできません",
    unknownProviderShort: "不明なプロバイダー: {provider}",
    selectProviderFirst: "最初にDDNSプロバイダーを選択してください",
    primaryConfigIncomplete:
      "現在のプライマリ ドメイン構成は不完全です。すべての必須フィールドに入力してください。",
    targetConfigIncomplete:
      "現在のエントリ構成は不完全です。すべての必須フィールドに入力してください。",
    manualTestStart:
      "手動テストが開始され、現在のターゲット IP を解析しています...",
    manualTestPrefix: "手動テスト",
    currentTargetIp:
      "現在のターゲット IP ({source}) — IPv4: {ipv4}, IPv6: {ipv6}",
    testAborted: "{message}、テストは中止されました",
    updateSuccess: "アップデート成功: {message}",
    updateFailed: "更新に失敗しました: {message}",
    testError: "テスト例外: {message}",
    settingsSaveFailed: "保存DDNS 自動同期設定に失敗しました",
    providerSetFailed: "プロバイダーの設定に失敗しました",
    configSaveFailed: "DDNS の保存に失敗しました",
    createTargetFailed: "DDNS エントリの作成に失敗しました",
    updateTargetFailed: "DDNS エントリの更新に失敗しました",
    deleteTargetFailed: "DDNS エントリの削除に失敗しました",
    updateTargetEnabledFailed: "更新 DDNS エントリ有効ステータスに失敗しました",
    providers: {
      common: {
        fields: {
          root_domain: {
            label: "ルートドメイン名",
            description:
              "は、example.com などのゾーンを決定するために使用されます。",
          },
          domain: {
            label: "完全なドメイン名",
            shortLabel: "ドメイン名",
            description: "更新される完全なドメイン名",
            hostDescription: "更新する完全なホスト名",
          },
          ttl: {
            description: "デフォルト {seconds} 秒",
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
            description: "オプション、透過的に dynv6 API に渡されます",
          },
        },
        configIncomplete: "dynv6 構成が不完全です",
        empty: "(空)",
        success: "dynv6: {detail} (送信: {params})",
        updateFailed: "dynv6 アップデートに失敗しました [{status}]: {detail}",
        requestError: "dynv6 リクエスト例外: {detail}",
      },
      duckdns: {
        fields: {
          domains: {
            label: "サブドメイン",
            description:
              ".duckdns.org サフィックスを付けずに、DuckDNS サブドメイン名のみを入力します。カンマ区切りがサポートされています",
          },
          token: {
            description:
              "DuckDNS コンソールのホームページでアカウント トークンを確認できます",
          },
        },
        configIncomplete: "DuckDNS 構成が不完全です",
        noIpAvailable:
          "DuckDNS 更新に失敗しました: IPv4 または IPv6 アドレスが利用できません",
        updateFailedWithStatus:
          "DuckDNS 更新に失敗しました [{status}]: {detail}",
        requestFailed: "リクエストが失敗しました",
        updateFailed: "DuckDNS 更新に失敗しました: {detail}",
        nonOkResponse: "が OK 以外の応答を返しました",
        success: "DuckDNS 正常に更新されました{detail}",
        requestError: "DuckDNS 例外リクエスト: {detail}",
      },
      dnspod: {
        fields: {
          record_line: {
            label: "線",
            description: "デフォルトで「デフォルト」行を使用します",
          },
        },
        defaultLine: "デフォルト",
        configIncomplete: "DNSPod 構成が不完全です",
        queryRecordFailed: "レコードのクエリに失敗しました",
        updateRecordFailed: "レコードの更新に失敗しました",
        createRecordFailed: "レコードの作成に失敗しました",
      },
      cloudflare: {
        fields: {
          api_token: {
            label: "API トークン",
            description: "ゾーン。DNS 編集権限が必要です",
          },
          zone_id: {
            description:
              "Cloudflare ドメイン名ページで、3 つの点をクリックし、コピー領域 ID を選択します",
          },
          proxied: {
            label: "Cloudflare エージェント",
            description:
              "Cloudflareプロキシ（オレンジ色の雲）を有効にするかどうか",
            options: {
              dnsOnly: "解析のみ",
              orangeCloud: "オレンジ色の雲",
            },
          },
        },
        configIncomplete: "Cloudflare 構成が不完全です",
        searchRecordFailed: "クエリ {type} レコードが失敗しました: {detail}",
        updateRecordFailed: "更新 {type} 記録に失敗しました: {detail}",
        createRecordFailed: "{type} レコードの作成に失敗しました: {detail}",
        recordOperationError: "{type} 録画動作例外: {detail}",
        success: "Cloudflare DNS 正常に更新されました",
      },
      godaddy: {
        configIncomplete: "GoDaddy の設定が不完全です",
        updateFailed: "アップデートに失敗しました",
        updateFailedWithStatus: "[{status}] {detail}",
      },
      porkbun: {
        configIncomplete: "豚まんの設定が不完全です",
        queryRecordFailed: "レコードのクエリに失敗しました",
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
            label: "線",
            description:
              "デフォルトで Alibaba Cloud の「デフォルト」行を使用する",
          },
        },
        configIncomplete: "Alibaba Cloud DNS 構成が不完全",
        requestFailed: "リクエストが失敗しました",
        updateFailed: "アップデートに失敗しました",
        createFailed: "作成に失敗しました",
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
        configIncomplete: "Baidu Cloud DNS 設定が不完全",
        queryFailed: "クエリが失敗しました",
        updateFailed: "アップデートに失敗しました",
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
          "現在のオペレーティング環境は Web Crypto をサポートしていないため、Huawei Cloud AK/SK 署名を生成できません。",
        configIncomplete: "Huawei Cloud DNS 構成が不完全",
        requestFailed:
          "Huawei Cloud DNS リクエストが失敗しました: HTTP {status} {statusText}、{detail}",
        zoneNotFound: "Huawei クラウドゾーンが見つかりません: {zone}",
      },
      tencentcloud: {
        label: "テンセントクラウド DNS",
        fields: {
          secret_key: {
            placeholder: "テンセントクラウド秘密鍵",
          },
          record_line: {
            label: "線",
            description: "デフォルトで「デフォルト」行を使用します",
          },
          record_line_id: {
            label: "ライン ID",
            description: "オプション;入力すると、行 ID が最初に使用されます",
          },
        },
        defaultLine: "デフォルト",
        configIncomplete: "Tencent Cloud DNS 構成が不完全",
        missingUpdatedRecordId:
          "Tencent Cloud が更新された RecordId を返さなかった",
        missingCreatedRecordId:
          "Tencent Cloud が作成後に RecordId を返さなかった",
      },
      noip: {
        fields: {
          hostname: {
            description:
              "完全なホスト名を入力します。カンマ区切りの複数のホスト名をサポートします。",
          },
          username: {
            label: "ユーザー名",
            description:
              "コンソールで生成された NO-IP DDNS キーのユーザー名を使用することをお勧めします",
          },
          password: {
            label: "パスワード",
            description:
              "メインアカウントのパスワードの代わりに、DDNSキーと一致するパスワードを使用することをお勧めします",
          },
        },
        statusMessages: {
          "911":
            "NO-IP サーバーで一時的な障害が発生しました。公式では、少なくとも 30 分後に再試行することを推奨しています。",
          nohost:
            "指定されたホスト名が存在しないか、現在の DDNS キーに属していません",
          badauth: "ユーザー名またはパスワードが間違っています",
          badagent:
            "クライアントは NO-IP によって無効になっています。ユーザー エージェントまたはクライアントのステータスを確認してください",
          "!donator":
            "現在のアカウントは要求された機能強化をサポートしていません",
          abuse: "DDNS キーは、悪用のため NO-IP によって禁止されました。",
        },
        unknownStatus: "不明なステータスを返します: {code}",
        updateFailed: "NO-IP 更新に失敗しました: {detail}",
        updateSuccess: "NO-IP 正常に更新されました {detail}",
        ipUnchanged: "NO-IP IP 不変 {detail}",
        configIncomplete: "NO-IP 構成が不完全です",
        noIpAvailable:
          "NO-IP 更新に失敗しました: 利用可能なIPv4 または IPv6 アドレスがありません",
        updateFailedWithStatus: "NO-IP 更新に失敗しました [{status}]: {detail}",
        requestFailed: "リクエストが失敗しました",
        emptyResponse: "NO-IP 更新失敗: 空の応答が返されました",
        requestError: "NO-IP 例外リクエスト: {detail}",
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
              "ESA サイト名、通常はルート ドメイン名。サイト ID が入力されている場合、この項目は一般的なクエリ用です",
          },
          site_id: {
            description:
              "オプション。入力後、サイトは直接操作されるため、毎回最初にサイト リストをクエリする必要がなくなります。",
          },
          proxied: {
            label: "ESA エージェント",
            description:
              "デフォルトでは、解析のみを行います。プロキシが有効になっている場合、ビジネスタイプは自動的に含まれます",
            options: {
              dnsOnly: "解析のみ",
              enabled: "プロキシを有効にする",
            },
          },
          biz_name: {
            label: "業種",
            description:
              "ESA プロキシが有効な場合にのみ有効で、デフォルトの Web",
            options: {
              web: "ウェブサイト",
              api: "インターフェース",
              imageVideo: "オーディオとビデオ",
            },
          },
        },
        configIncomplete: "Alibaba Cloud ESA DNS 構成が不完全",
        siteNameMissing: "Alibaba Cloud ESA DNS サイト名がありません",
        siteNotFound: "見つかりません ESA サイト: {site}",
        noIpAvailable:
          "Alibaba Cloud ESA DNS には更新可能な IP アドレスがありません",
        createRecordFailed: "CreateFailed: レコードの作成に失敗しました",
        success: "Alibaba Cloud ESA DNS 正常に更新されました",
        recordIdMissing: "更新失敗: レコードに RecordId がありません",
      },
      dynu: {
        fields: {
          api_key: {
            description: "API-Dynu で生成されたキー API 認証情報",
          },
          domain: {
            description: "更新する完全な Dynu ホスト名",
          },
          group: {
            description:
              "オプション;グループを Dynu DNS レコードに書き込みます",
          },
        },
        actionFailed: "{action} 失敗しました",
        actions: {
          resolveRoot: "Dynu ルートゾーンの解析",
          readDnsService: "Dynu DNS サービスを読み取ります",
          updateWildcardAlias: "Dynu ワイルドカード エイリアスの更新",
          queryRecord: "Dynu {type} レコードのクエリ",
          updateRecord: "Dynu {type} 記録を更新",
          createRecord: "Dynu {type} レコードを作成",
        },
        invalidRootInfo: "Dynu が有効なルート ドメイン情報を返しませんでした",
        wildcardUnsupported:
          "Dynu REST は、nodeName を *.{domain} として DNS として記録することをサポートしていません。まず {domain} を独立したサービスとして追加し、Dynu DDNS サービスでワイルドカード エイリアスを有効にするか、DDNS 設定を {domain} に変更してください。",
        wildcardUnchanged: "Dynu ワイルドカード エイリアス IP 変更なし",
        wildcardSuccess: "Dynu ワイルドカード エイリアスが正常に更新されました",
        configIncomplete: "Dynu 設定が不完全です",
        noIpAvailable:
          "Dynu アップデートに失敗しました: 利用可能な IPv4 または IPv6 アドレスがありません",
        recordIdMissing:
          "Dynu から返された DNS レコードには RecordId がありません",
        requestError: "Dynu リクエスト例外: {detail}",
      },
      edgeone: {
        label: "テンセントクラウド EdgeOne",
        fields: {
          secret_key: {
            placeholder: "テンセントクラウド秘密鍵",
          },
          zone_id: {
            description:
              "EdgeOne サイト ID、ホストゾーンを見つけるために使用されます",
          },
          domain: {
            description:
              "更新する完全なホスト名。まず中国語のドメイン名を Punycode に変換してください",
          },
          location: {
            label: "ラインを分析します",
            placeholder: "デフォルトまたは CN.BJ",
            description:
              "オプション;デフォルトでは空白のままにして、デフォルトのグローバル行を示します",
          },
          ttl: {
            description:
              "デフォルトは 300 秒、EdgeOne では 60 ～ 86400 が許可されます",
          },
          overseas_access: {
            label: "海外アクセス制御",
            description:
              "オンにすると、EdgeOneセキュリティポリシーAPIが海外IPからのアクセスをブロックします。香港、マカオ、台湾は海外ではありません。この設定は、構成が変更されたときに 1 回だけ同期され、DDNS の更新ごとに繰り返されることはありません。",
            options: {
              off: "未使用",
              blockOverseas: "海外ブロックIP",
            },
          },
          endpoint: {
            description:
              "デフォルトの国内バージョン、https://teo.intl.tencentcloudapi.com または地域のアクセス ドメイン名に変更可能",
          },
          region: {
            placeholder: "空白のままにしてください",
            description:
              "オプション;ほとんどのシナリオでは空白のままにすることができます",
          },
        },
        configIncomplete: "Tencent Cloud EdgeOne 構成が不完全",
        configTargetIncomplete:
          "Tencent Cloud EdgeOne の構成が不完全で、ゾーン ID またはドメイン名がありません",
        missingRecordId: "EdgeOne 返されたレコードに RecordId がありません",
        missingCreatedRecordId: "EdgeOne 作成した RecordId は返されません",
        overseasAccess: {
          describeRulesFailed:
            "EdgeOne 海外アクセス制御は既存のカスタム ルールの読み取りに失敗しました (provider_target={target}、zone_id={zoneId}、endpoint_host={endpointHost}、リージョン={region}、エンティティ={entity}、スコープ={scope}): {message}",
          syncFailedWithAttempt:
            "EdgeOne 海外アクセス制御同期失敗({attempt}、submitted_rule_count={count}): {message}",
          syncAllScopesFailed:
            "EdgeOne 海外アクセス制御の同期に失敗しました: すべてのルール スコープの試行が失敗しました",
          cleanupAllScopesFailed:
            "EdgeOne 海外アクセス制御のクリーンアップに失敗しました: すべてのルール スコープの試行が失敗しました",
          syncSuccess:
            "同期 EdgeOne 海外 IP ブロックポリシー、中国本土、香港、マカオ、台湾からのアクセスのみ許可",
          cleanupSuccess: "クリーニング済み EdgeOne 海外 IP ブロックポリシー",
        },
      },
      edgeone_cname: {
        label: "Tencent Cloud EdgeOne (CNAME アクセス)",
        fields: {
          secret_key: {
            placeholder: "テンセントクラウド秘密鍵",
          },
          zone_id: {
            description:
              "EdgeOne サイト ID、アクセラレーションされたドメイン名が属するサイトを見つけるために使用されます",
          },
          domain: {
            label: "高速化されたドメイン名",
            description:
              "EdgeOneで作成された高速化されたドメイン名。現在のオリジン サイト タイプ IP_DOMAIN のみをサポートし、一度に 1 つのオリジン サイト アドレスのみを更新できます。",
          },
          overseas_access: {
            label: "海外アクセス制御",
            description:
              "オンにすると、EdgeOneセキュリティポリシーAPIが海外IPからのアクセスをブロックします。香港、マカオ、台湾は海外ではありません。この設定は、構成が変更されたときに 1 回だけ同期され、DDNS の更新ごとに繰り返されることはありません。",
            options: {
              off: "未使用",
              blockOverseas: "海外ブロックIP",
            },
          },
          endpoint: {
            description:
              "デフォルトの国内バージョン、https://teo.intl.tencentcloudapi.com または地域のアクセス ドメイン名に変更可能",
          },
          region: {
            placeholder: "空白のままにしてください",
            description:
              "オプション;ほとんどのシナリオでは空白のままにすることができます",
          },
        },
        configIncomplete:
          "Tencent Cloud EdgeOne (CNAME アクセス) の設定が不完全です",
        singleAddressOnly:
          "Tencent Cloud EdgeOne (CNAME アクセス) は一度に 1 つのオリジンサイトアドレスのみ更新できます。DDNS 更新範囲を「IPv4 のみ更新」または「IPv6 のみ更新」に設定してください。",
        noIpAvailable:
          "Tencent Cloud EdgeOne (CNAME アクセス) には更新可能な IP アドレスがありません",
        domainNotFound:
          "見つかりません EdgeOne 高速化されたドメイン名: {domain}",
        unsupportedOriginType:
          "現在の高速化されたドメイン名オリジン サイト タイプは {originType} で、DDNS 更新ではタイプ IP_DOMAIN の高速化されたドメイン名のみがサポートされます。",
        originUnchanged:
          "Tencent Cloud EdgeOne (CNAME アクセス) ソース サイトはすでに最新であるため、更新する必要はありません。",
        successWithInvalidHostHeaderIgnored:
          "Tencent Cloud EdgeOne (CNAME アクセス) オリジン サイトが正常に更新されました (無効なホスト ヘッダーは無視されました)",
        success:
          "Tencent Cloud EdgeOne (CNAME アクセス) オリジンサイトが正常に更新されました",
      },
    },
  },
  smartConnect: {
    runTypes: {
      direct: "ダイレクト接続モード",
      reverseProxy: "アンチジェネレーションモード",
      subdomain: "サブドメインモード",
    },
    currentMode: "現在のモード",
    unavailableReason:
      "サブドメイン モードのみが利用可能で、現在は {mode} です。",
    selectLocalIp: "ローカルエリアネットワークIPを選択してください",
    selectValidLocalIpv4: "有効なローカル LAN IPv4 アドレスを選択してください",
    dnsmasqNotInstalled:
      "dnsmasq が検出されません。最初にインストールを完了してください。",
    dnsmasqNotInitialized:
      "dnsmasq はまだ初期化されていません。まず環境の初期化を完了してください。",
    syncFailed: "スマート接続の同期に失敗しました",
  },
  scanDiscovery: {
    localIpv4CidrOnly:
      "スキャン ネットワーク セグメントはローカル IPv4 CIDR: {cidrs} のみをサポートします",
    maxCidrsExceeded:
      "一度にネットワークセグメントをスキャンする最大{max}を選択します",
    maxHostsExceededWithCurrent:
      "は一度に最大 {max} のホストをスキャンできますが、現在は {current} です",
    maxHostsExceeded: "一度に最大 {max} のホストをスキャンします",
    selectAtLeastOneCidr:
      "ローカル IPv4 スキャン ネットワーク セグメントを少なくとも 1 つ選択してください",
    targetLabels: {
      docker: "{cidr} (Docker ホスト LAN)",
      loopback: "{cidr} (ネイティブ ループバック)",
      interface: "{cidr}({name})",
      mapping: "{cidr} (マッピング済みターゲット)",
      custom: "{cidr} (カスタム)",
      saved: "{cidr} (保存)",
    },
    serviceLabels: {
      lottery: "宝くじアシスタント",
      dlymusic: "Liyu Music Management",
      kuake: "Quark自動転送",
      xunlei: "サンダー",
      nowen: "ネビュラポータル",
      fnos: "フェイニウOS",
      fnys: "フェイニウフィルム",
      xiaoyaAlist: "シャオヤ・アリスト",
    },
  },
  gatewayProxyHeaders: {
    runTypes: {
      direct: "ダイレクト接続モード",
      reverseProxy: "アンチジェネレーションモード",
      subdomain: "サブドメインモード",
    },
    unavailableReason:
      "サブドメイン モードのみが利用可能で、現在は {mode} です。",
    syncFailed: "同期ゲートウェイプロトコルヘッダーの構成に失敗しました",
  },
  sshSecurity: {
    logSourceUnavailable:
      "現在のシステムはjournalctlまたは/var/log/auth.logを見つけられません",
    openWrtUnsupported:
      "OpenWrt バージョンはまだサポートされていません SSH 安全",
    enableUnavailable: "現在の環境では有効化できません SSH セキュリティ",
    syncFirewallUnavailable: "現在の環境は同期できません SSH ファイアウォール",
    clearFirewallUnavailable:
      "現在の環境SSH ファイアウォールをクリアできません",
    logSourceUnavailableShort: "SSH ログソースが利用できません",
    customCidrInvalid: "カスタム CIDR 間違った形式: {cidrs}",
    syncSshPolicyFailed: "同期 SSH 専用ファイアウォール ルールが失敗しました",
    clearSshPolicyFailed:
      "クリア SSH 専用ファイアウォール ルールが失敗しました",
    blockRecordInvalid: "ブロックレコード形式が正しくありません",
    routes: {
      updateConfigFailed: "アップデート SSH セキュリティ設定に失敗しました",
      syncFirewallSuccess:
        "同期 {allowedCidrs} は許可 CIDR および {synced} SSH IP から {ports} ポートはブロック",
      syncFirewallFailed: "同期 SSH ファイアウォールが失敗しました",
      clearFirewallSuccess:
        "SSH プライベート ファイアウォール ルールをクリアしました",
      clearFirewallFailed: "クリア SSH ファイアウォールが失敗しました",
      readLoginLogsFailed: "SSH ログインログの読み取りに失敗しました",
      blockNotFound: "ブロッキングレコードは存在しません",
      removeBlockFailed: "ブロックを解除できませんでした",
      selectIps: "ブロックを解除するには IP を選択してください",
      removeBlocksFailed: "一括ブロック解除に失敗しました",
    },
  },
  notifications: {
    brand: {
      prefix: "ノックノック",
      defaultTitle: "ノック通知",
    },
    templates: {
      events: {
        authLoginSuccess: "ログイン成功",
        authLogout: "ログアウト",
        authLoginFailure: "ログインに失敗しました",
        authSessionIpDrift: "セッション IP ドリフト",
        securityScannerBlocked: "スキャナー傍受",
        ddnsUpdateCompleted: "DDNS アップデート",
        gatewayThrottleBlocked: "ゲートウェイスロットルブロック",
        wafBlocked: "WAF ブロック",
        sshLoginSuccess: "SSH ログイン成功",
        sshLoginFailure: "SSH ログインに失敗しました",
        sshIpBlocked: "SSH IP ブロックされました",
        appUpdateAvailable: "アプリアップデートのヒント",
        cpuAlert: "CPU アラーム",
        cpuRecovered: "CPU 回復",
        memoryAlert: "メモリーアラーム",
        memoryRecovered: "記憶回復",
        frpConnected: "FRP 接続されました",
        frpDisconnected: "FRP 切断されました",
        cloudflaredConnected: "Cloudflared が接続されています",
        cloudflaredDisconnected: "Cloudflared 切断されました",
      },
      ruleName: "{event} お知らせ",
      levels: {
        info: "お知らせ",
        warn: "注意",
        error: "エラー",
        critical: "重度",
      },
      sources: {
        serverAdmin: "管理バックエンド",
        goReauthProxy: "認定代理店",
        systemMonitor: "システム監視",
      },
      authMethods: {
        oidc: "外部アカウント",
      },
      grantTypes: {
        browserSession: "ブラウザセッション",
        loginIpGrant: "ログイン IP 認証",
      },
      wafModes: {
        detection: "検出",
        blocking: "ブロック",
        off: "閉じる",
      },
      wafActions: {
        block: "ブロック",
        deny: "拒否",
        detect: "検出",
        log: "記録",
        pass: "リリース",
      },
      logoutSources: {
        userLogout: "ユーザーが積極的に終了",
        adminSessionDelete: "管理者はオフラインです",
      },
      driftSources: {
        proxySession: "エージェントセッション",
        fnosToken: "フェイニウトークン",
        sessionRefresh: "セッション更新",
        browserSession: "ブラウザセッション",
      },
      ddnsTriggers: {
        cron: "スケジュールされたタスク",
        enable: "有効化後の最初の実行",
        manualTest: "手動テスト",
      },
      ddnsUpdateScopes: {
        ipv4Only: "のみIPv4",
        ipv6Only: "IPv6のみ",
      },
      ddnsIpSources: {
        public: "パブリックネットワーク検出",
        interface: "ネットワークカード読み取り",
        static: "静的 IP",
        domain: "ドメイン名解決",
      },
      updateCheckReasons: {
        cron: "定期点検",
        manual: "手動検査",
        manualCheckAndDownload: "手動で確認してダウンロード",
        downloadBootstrap: "ダウンロード前にご確認ください",
      },
      credential: "認証情報",
      unknownCredential: "不明な認証情報",
      credentialLinkedTotp: "{authMethod}「{credential}」関連 TOTP「{totp}」",
      credentialName: "証明書「{credential}」",
      sessionCommentCompact: "備考: {comment}",
      appendSessionComment: "{text} (備考:{comment})",
      yes: "はい",
      no: "いいえ",
      wafOutcomeBlocked: "ブロック",
      wafOutcomeLogged: "記録",
      sections: {
        overview: "イベント概要",
        aggregation: "集計ステータス",
        advice: "取り扱いに関する提案",
      },
      aggregationText:
        "この通知には、{seconds} 番目のウィンドウ内の{count} の同様のイベントが集約されています。",
      details: {
        units: {
          seconds: "{count}秒",
          minutes: "{count}分",
          times: "{count}回",
          ratePerSecond: "{count}回/秒",
        },
        listSeparator: "、",
        unknown: "不明",
        unknownIp: "不明 IP",
        unknownMethod: "未知の方法",
        unknownProvider: "不明なプロバイダー",
        unknownUser: "不明なユーザー",
        unknownHost: "不明なホスト",
        currentSession: "現在のセッション",
        memoryMetric: "メモリ",
        connected: "接続されました",
        disconnected: "切断されました",
        parenthesized: "({value})",
        sessionCommentSentence: "現在のセッションコメントは「{comment}」です。",
        aggregationStatsValue:
          "{count} アイテム / {seconds} 2 番目のウィンドウ",
        facts: {
          credentialName: "証明書名",
          linkedTotp: "協会TOTP",
          sessionComment: "セッションメモ",
          loginIp: "ログインIP",
          ipLocation: "IP 場所",
          authMethod: "認証方式",
          loginProvider: "ログインプロバイダー",
          grantType: "認可方法",
          rememberLogin: "忘れずにログインしてください",
          sessionExpiresAt: "セッションが期限切れになりました",
          sessionId: "セッション ID",
          logoutSource: "終了方法",
          loginTime: "ログイン時間",
          sourceIp: "ソース IP",
          failureAttempts: "失敗数",
          retryWait: "再試行してお待ちください",
          limitUntil: "リミットカットオフ",
          originalIp: "オリジナル IP",
          originalLocation: "元の場所",
          currentIp: "現在 IP",
          currentLocation: "現在地",
          driftSource: "変化の根源",
          hitCount: "ヒット数",
          observationWindow: "観察窓",
          triggerThreshold: "トリガー閾値",
          blockedAt: "インターセプト時間",
          recentPaths: "最近のパス",
          target: "エントリー",
          provider: "プロバイダー",
          targetType: "エントリータイプ",
          trigger: "実行方法",
          updateScope: "更新範囲",
          ipSource: "IP 出典",
          ipv4Change: "IPv4 変更",
          ipv6Change: "IPv6 変更",
          result: "実行結果",
          blockDuration: "ブロック期間",
          blockedUntil: "封鎖終了",
          rateLimit: "電流制限閾値",
          burstCapacity: "バースト容量",
          targetHost: "ターゲットホスト",
          requestPath: "リクエストパス",
          routeType: "ルーティングタイプ",
          authRoute: "認定ルーティング",
          traceId: "トレース ID",
          requestAddress: "リクエストアドレス",
          outcome: "処理結果",
          wafAction: "WAF アクション",
          wafMode: "WAF モード",
          ruleIds: "ルール ID",
          ruleBundle: "ルールパック",
          statusCode: "ステータスコード",
          user: "ユーザー",
          port: "ポート",
          logTime: "ログタイム",
          invalidUser: "無効なユーザーです",
          threshold: "しきい値",
          window: "ウィンドウ",
          blockedReason: "ブロックの理由",
          relatedUser: "関連ユーザー",
          currentVersion: "現在のバージョン",
          latestVersion: "最新バージョン",
          checkReason: "確認方法",
          forceUpdate: "強制アップデート",
          releaseNotes: "アップデート手順",
          hostname: "ホスト名",
          currentUsage: "現在の使用状況",
          alertThreshold: "アラーム閾値",
          recoverThreshold: "回復閾値",
          sampleInterval: "サンプリング間隔",
          sustainDuration: "期間",
          tunnelType: "トンネルタイプ",
          connectionStatus: "接続状態",
          processPid: "プロセスPID",
          runtimeFeedback: "操作フィードバック",
          eventType: "イベントタイプ",
          riskLevel: "リスクレベル",
          eventSource: "イベントソース",
          happenedAt: "発生時刻",
          aggregationStats: "集計統計",
        },
        authLoginSuccess: {
          loginViaProvider: "{provider}からログイン",
          loginWithMethod: "は {method} を使用します",
          authViaProvider: "合格 {provider}",
          authWithMethod: "は {method} を使用します",
          summaryOidc: "{credential} {method}成功、ソース IP {ip}{totpPart}",
          linkedTotpPart: "、 TOTP『{totp}』 に関連付けられています",
          summaryTotp:
            "{method}『{credential}』 TOTP『{totp}』 に関連付けられています {ip}からログイン成功",
          summaryCredential:
            '認証情報 "{credential}" が {ip} から正常にログインしました',
          overview:
            "今回のログイン{auth}で認証が完了し、認可方法は{grantType}{locationPart}となります。 {commentPart}",
          locationPart: "、ログイン場所は{location}です",
          advice:
            "ご自身でログインされていない場合は、できるだけ早くセッションをキャンセルし、アクセスポリシーを確認することをお勧めします。",
        },
        authLogout: {
          summaryTotp:
            "{method}「{credential}」関連 TOTP「{totp}」がログアウトしました",
          summaryCredential: "資格情報「{credential}」がログアウトしました",
          overview:
            "セッションは、終了モード {source} で {ip}{locationPart} から終了しました。 {commentPart}",
          advice:
            "期待どおりに終了しない場合は、管理者がオフラインになっているか、異常なセッション クリーンアップが発生していないかを確認してください。",
        },
        authLoginFailure: {
          summary: "{ip}からのログイン失敗が{attempts}回累積しました",
          overview:
            "連続ログイン認証の失敗を検出しました。現在のソース IP は {ip}{retryPart}{blockedPart} です。",
          retryPart: "、再試行する前に {seconds} 秒待つ必要があります",
          blockedPart: "、制限は {time} まで続きます",
          advice:
            "自分で実行していない場合は、すぐに認証情報の安全性を確認し、ソース IP をブロックするか、ログイン保護レベルを向上させることを検討することをお勧めします。",
        },
        authSessionIpDrift: {
          summary: "{session} IP {fromIp} から {toIp} に切り替えます",
          overview:
            "は、{session}のアクセス元IPが変化したことを検出し、アクセス元が{source}であると判明しました。 {commentPart}これは通常、ネットワークの切り替え、プロキシの変更、またはセッションの異常に関連しています。",
          advice:
            "この IP の変更が予想どおりでない場合は、現在のセッションが乗っ取られる危険性があるかどうかをできるだけ早く確認してください。",
        },
        securityScannerBlocked: {
          summary: "{ip} はスキャン動作によりブロックされました",
          overview:
            "このソースは、{minutes} 分以内に合計 {hits} のスキャン動作をトリガーし、しきい値を {threshold} 回 {pathsPart} 超えました。",
          pathsPart: ";最近のヒット パスには {paths} が含まれます",
          advice:
            "ゲートウェイのログをチェックして、悪意のある検出かどうかを確認することをお勧めします。偽陽性であることが確認された場合は、スキャンしきい値をさらに調整できます。",
        },
        ddnsUpdateCompleted: {
          defaultTarget: "DDNS エントリー",
          summarySuccess: "{target} DDNS 正常に更新されました",
          summaryFailure: "{target} DDNS アップデートに失敗しました",
          currentTask: "今回のミッション",
          overview:
            "{trigger}は範囲{scope}、IPソース{ipSource}でDDNS更新を実行しました。 {resultPart}",
          resultPart: "結果の説明: {message}",
          adviceSuccess:
            "解析がまだ有効になっていない場合は、外部アクセスを検証する前に、DNS キャッシュが更新されるのを待ち続けることができます。",
          adviceFailure:
            "プロバイダーの資格情報、レコード構成の解析、およびパブリック ネットワーク IP の取得ステータスが正常かどうかを確認することをお勧めします。",
          primaryDomain: "メインドメイン",
          additionalDomain: "追加フィールド",
        },
        gatewayThrottleBlocked: {
          summary:
            "{ip} リクエストが速すぎるためブロックされました {seconds} 秒",
          overview:
            "このソースはゲートウェイ スロットリング保護をトリガーしました。現在の制限しきい値は {rate} 回/秒、バースト容量は {burst}{targetPart} です。",
          targetPart: "、ターゲットリクエストは{target}です",
          advice:
            "アクセスログをチェックして、突然のトラフィック、偶発的な損傷、または悪意のあるリクエストであるかどうかを確認し、必要に応じて現在の制限ポリシーを調整してください。",
        },
        wafBlocked: {
          summary: "{ip} のリクエストは WAF {outcome} に置き換えられました",
          overview:
            "WAF は {outcome} {ip}{hostPart}{pathPart}{actionPart}{modePart} から供給されています。 {rulesPart}",
          hostPart: "訪問しました {host}",
          pathPart: "{path}",
          actionPart: "、アクションは {action}",
          modePart: "、現在のモードは {mode}",
          rulesPart: "ヒットルール: {rules}。",
          adviceBlocked:
            "WAF ログの Trace ID を押して、ヒットの詳細を表示してください。誤警報であることが確認された場合は、BUGまでにプロジェクトチームに報告してください。",
          adviceLogged:
            "WAF ログの Trace ID を押してヒットの詳細を表示し、ルールとリクエストのコンテキストに基づいてポリシーを調整する必要があるかどうかを判断してください。",
        },
        sshLoginSuccess: {
          summary: "SSH ユーザー「{username}」が{ip}から正常にログインしました",
          overview:
            "は、{ip}{locationPart}{authPart} からの SSH の 1 つの成功したログインを検出しました。",
          authPart: "、認証方法は{authMethod}です",
          advice:
            "このログインが予期されていない場合は、SSH アカウント、キー、およびソース アクセス ポリシーを確認してください。",
        },
        sshLoginFailure: {
          summary:
            "SSH ユーザー「{username}」は{ip}からのログインに失敗しました",
          overview:
            "このソースは、{minutes} 分間に {attempts}/{threshold} {locationPart} 回ログインに失敗しました。",
          locationPart: "、位置{location}",
          advice:
            "失敗の数がブロックしきい値に近いかどうかに注意し、必要に応じて SSH の公開範囲を強化するか、資格情報を調整してください。",
        },
        sshIpBlocked: {
          reasonCidrNotAllowed: "は許可エリア内にありません",
          reasonFailedThreshold: "失敗数がしきい値に達しました",
          summary: "{ip} は SSH によって安全にブロックされました",
          overview:
            "SSH {reason} により、セキュリティがソース {ip}{locationPart} をブロックしました。",
          advice:
            "ソースが信頼できるかどうかを確認してください。誤ってブロックした場合は、SSH 安全なブロック リストでブロックを解除できます。",
        },
        appUpdateAvailable: {
          currentVersionUnknown: "現在のバージョンは不明",
          targetVersionUnknown: "対象バージョン不明",
          summary: "新しいバージョン{version}が見つかりました",
          currentCheck: "今回の検査",
          overview:
            "{reason}は、fn-knockを{localVersion}から{latestVersion}{forcePart}にアップグレードできることを発見しました。",
          forcePart: "、できるだけ早くアップデートを手配することをお勧めします",
          releaseNotesAdvice: "アップデートの説明: {releaseNotes}",
          advice:
            "適切なメンテナンス期間中にアップデートを完了し、インストール前に現在の構成とサービスのステータスを確認することをお勧めします。",
        },
        systemMetric: {
          recoveredSummary:
            "{hostname} {metric} 使用率が {usage}% に戻りました",
          alertSummary: "{hostname} {metric} 使用率が {usage}% に増加しました",
          recoveredOverview:
            "{hostname}の{metric}使用率は{usage}%まで戻り、回復ラインは{recover}%です。以前のアラームしきい値は {threshold}% でした。",
          alertOverview:
            "{hostname}の{metric}使用率は現在{usage}%で、警報閾値{threshold}%を超えており、回復ラインは{recover}%に設定されています。",
          recoveredAdvice:
            "現在の資源は比較的安全な範囲に戻りました。今後も変動を繰り返すかどうか引き続き観察することをお勧めします。",
          alertAdvice:
            "継続的なリソースの満杯を避けるために、高負荷のプロセス、バックグラウンドタスク、または外部トラフィックの変化をできるだけ早く確認することをお勧めします。",
        },
        tunnel: {
          connectedSummary: "{tunnel} 接続されました",
          disconnectedSummary: "{tunnel} 切断されました",
          connectedOverview:
            "{tunnel} トンネル接続が復旧しました {messagePart}。",
          connectedMessagePart: "、実行中のフィードバックは: {message}",
          disconnectedOverview:
            "{tunnel} トンネル接続が切断されました {messagePart}。",
          disconnectedMessagePart: "、現在のフィードバックは: {message}",
          connectedAdvice:
            "以前にアクセスの問題のトラブルシューティングを行っていた場合は、外部入口が復旧したかどうかを再確認できます。",
          disconnectedAdvice:
            "トンネル構成、上流ネットワークのステータス、およびリモート サービスが到達可能かどうかを確認することをお勧めします。",
        },
        short: {
          loginFailureAttempts: "{count} 失敗しました",
          scanHits: "{count} スキャン",
          scanBlocked: "スキャンと傍受",
          success: "成功",
          failure: "失敗しました",
          blockSeconds: "{seconds}s がブロックされました",
          blockTriggered: "トリガーブロック",
          rules: "ルール {rules}",
          sshLoginSuccess: "SSH ログイン成功",
          sshLoginFailure: "SSH ログインに失敗しました",
          regionNotAllowed: "この地域では許可されていません",
          failureThreshold: "失敗しきい値",
          currentVersion: "現在 {version}",
        },
        titles: {
          ddnsUpdateSuccess: "{target} が正常に更新されました",
          ddnsUpdateFailure: "{target} アップデートに失敗しました",
          credentialIpDrift: "証明書「{credential}」IP ドリフト",
          appUpdateAvailable: "新しいバージョン{version}が見つかりました",
        },
      },
    },
    providers: {
      catalog: {
        email: {
          label: "メール",
          description:
            "は、SMTP を介して電子メール通知を送信し、メールボックス接続情報を一元管理するための IMAP 設定項目の保存をサポートします。",
          fields: {
            smtp_host: {
              label: "SMTP ホスト",
              description:
                "電子メール送信サーバーのアドレス (smtp.example.com など)。",
            },
            smtp_port: {
              label: "SMTPポート",
              description:
                "共通ポートは 465 (SSL/TLS) または 587 (STARTTLS) です。",
            },
            smtp_security: {
              label: "SMTP 暗号化方式",
              options: {
                none: "暗号化されていません",
              },
            },
            smtp_auth_mode: {
              label: "SMTP 認証方式",
              description:
                "は自動的に AUTH PLAIN を優先し、サポートされていない場合は AUTH LOGIN に戻ります。",
              options: {
                auto: "オートネゴシエーション",
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
              label: "メール送信中",
              description:
                "は、メールヘッダーの MAIL FROM および From アドレスとして使用されます。",
            },
            from_name: {
              label: "送信者名",
            },
            to_addresses: {
              label: "デフォルトの受信者",
              description:
                "は、複数のメールボックスを区切るためのカンマまたは改行をサポートしています。テスト送信ではここで受信者が使用され、ルールはターゲットでオーバーライドすることもできます。",
              targetLabel: "受信者のオーバーライド",
              targetDescription:
                "オプション。プロバイダーのデフォルト受信者を使用する場合は、空白のままにします。",
              addressLabel: "受信者",
            },
            cc_addresses: {
              label: "デフォルトCC",
              targetLabel: "CCの範囲",
              addressLabel: "CC",
            },
            bcc_addresses: {
              label: "デフォルトの BCC",
              targetLabel: "BCC カバレッジ",
              addressLabel: "BCC",
            },
            reply_to: {
              label: "デフォルトの返信アドレス",
              targetLabel: "返信アドレスの上書き",
              addressLabel: "返信アドレス",
            },
            allow_invalid_tls: {
              label: "証明書を検証しないことを許可します",
              description:
                "独自に構築したメール サーバーまたは自己署名証明書をデバッグする場合にのみオンにすることをお勧めします。実稼働環境では、これをオフにしておく必要があります。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            imap_host: {
              label: "IMAP ホスト",
              description:
                "オプション。受信設定を保存するために使用されます。現在の通知送信プロセスは SMTP のみを使用し、IMAP を積極的に読み取りません。",
            },
            imap_port: {
              label: "IMAPポート",
            },
            imap_security: {
              label: "IMAP 暗号化方式",
              options: {
                none: "暗号化されていません",
              },
            },
            imap_username: {
              label: "IMAP ユーザー名",
            },
            imap_password: {
              label: "IMAP パスワード",
            },
            imap_mailbox: {
              label: "IMAP メールディレクトリ",
            },
            subject_prefix: {
              label: "トピックの接頭辞",
              description: "オプション、例: [運用環境]。",
              placeholder: "[制作環境]",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
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
            smtpReaderDisposed: "SMTP リーダーがリリースされました",
            invalidSmtpResponse: "SMTP を解析できません 応答: {line}",
            smtpConnectionTimeout: "SMTP 接続タイムアウト",
            smtpTlsHandshakeTimeout: "SMTP TLS ハンドシェイクタイムアウト",
            smtpCommandFailed: "{message}: {code} {response}",
            unknownResponse: "不明な応答",
            authPlainUnsupported:
              "SMTP サーバーは AUTH PLAIN をサポートしていません",
            authLoginUnsupported:
              "SMTP サーバーは AUTH LOGIN をサポートしていません",
            unsupportedAuthMechanisms:
              "SMTP サポートされていない認証方法: {mechanisms}",
            authFailed: "SMTP 認証に失敗しました",
            usernameAuthFailed: "SMTP ユーザー名認証に失敗しました",
            passwordAuthFailed: "SMTP パスワード認証に失敗しました",
            dataStartFailed: "SMTP DATA ステージ起動に失敗しました",
            submitFailed: "SMTP メール送信に失敗しました",
            invalidFromAddress: "送信メールの形式が間違っています",
            recipientRequired:
              "少なくとも 1 つの受信メール アドレスを設定する必要があります",
            handshakeFailed: "SMTP サーバーハンドシェイクが失敗しました",
            ehloFailed: "SMTP EHLO 失敗しました",
            startTlsUnsupported:
              "SMTP サーバーは STARTTLS 機能を宣言していませんでした",
            startTlsFailed: "SMTP STARTTLS 失敗しました",
            ehloAfterTlsFailed: "SMTP TLS アップグレード後 EHLO が失敗しました",
            credentialsRequired:
              "SMTP ユーザー名とパスワードを空にすることはできません",
            noAuthMechanism:
              "SMTP サーバーは利用可能な認証方法を提供していません。",
            mailFromFailed: "SMTP 送信者の設定に失敗しました",
            recipientSetFailed:
              "SMTP 受信者 {recipient} を設定できませんでした",
            quitFailed: "SMTP 終了に失敗しました",
            missingSmtpHost: "SMTP ホストが見つかりません",
            deliveryFailed: "メール配信に失敗しました",
          },
        },
        pushplus: {
          description:
            "プッシュ通知はPushPlus標準の送信インターフェースを通じて送信され、公式アカウント、アプリ、メールなどのチャネルをルールに従って選択できます。",
          fields: {
            server_url: {
              label: "サービスアドレス",
              description:
                "公式インターフェースのデフォルト値をそのままにしておきます。",
            },
            token: {
              description:
                "PushPlusのユーザートークンまたはメッセージトークンは大切に保管してください。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            topic: {
              label: "グループエンコーディング",
              description:
                "オプション。入力後、メッセージは指定されたグループに送信されます。入力されていない場合、メッセージはトークン自体に送信されます。",
            },
            template: {
              label: "メッセージテンプレート",
              description:
                "はデフォルトで Markdown を使用します。ターゲットチャンネルがプレーンテキストまたはHTMLに適している場合は、個別に切り替えることもできます。",
              options: {
                txt: "プレーンテキスト",
              },
            },
            channel: {
              label: "送信チャンネル",
              description:
                "はデフォルトで WeChat 公式アカウントに送信されます。他のチャンネルがPushPlusで設定されている場合は、ここで切り替えることができます。",
              options: {
                wechat: "WeChat 公開アカウント",
                webhook: "サードパーティ Webhook",
                cp: "エンタープライズ WeChat アプリケーション",
                mail: "メール",
                sms: "SMS",
                voice: "音声",
                extension: "プラグイン/デスクトップ プログラム",
                clawbot: "WeChat クローボット",
              },
            },
            option: {
              label: "チャンネル設定パラメータ",
              description:
                "オプション。 cp、Webhook、メールなどのチャネルは通常、PushPlus パーソナル センターで事前に設定されたチャネル コードを入力する必要があります。",
            },
            to: {
              label: "フレンドトークン / ユーザー ID",
              description:
                "オプション。 WeChat 公式アカウント チャネルの友達トークンを入力し、エンタープライズ WeChat アプリケーション チャネルのユーザー ID を入力します。 PushPlusドキュメント形式で複数人で入力できます。",
              placeholder: "friend_token または user1、user2",
            },
            callback_url: {
              label: "コールバック URL",
              description:
                "オプション。 PushPlus 非同期配信が完了すると、結果がこのアドレスにコールバックされます。",
            },
            pre: {
              label: "前処理エンコーディング",
              description:
                "オプション。これは、PushPlus アカウントが対応する前処理ロジックで構成されている場合にのみ入力されます。このロジックは、メッセージの内容をサーバーに送信する前に処理するために使用されます。",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingToken: "PushPlus トークンがありません",
            requestFailed: "PushPlus リクエストは失敗しました",
          },
        },
        wxpusher: {
          description:
            "は、WxPusher 標準プッシュ インターフェイスを通じて、指定された UID またはトピックにメッセージ通知を送信します。ルールのターゲットを空白のままにすると、プロバイダーのデフォルトのターゲット構成が継承されます。",
          fields: {
            server_url: {
              label: "サービスアドレス",
              description: "正式サービスではデフォルト値のままにしておきます。",
            },
            app_token: {
              description:
                "WxPusher バックグラウンドアプリケーションのAppTokenを適切に保管してください。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            uids: {
              label: "デフォルト UID リスト",
              targetLabel: "UID リスト",
              description:
                "オプション。テスト送信では、ここでは UID の使用が優先されます。ルールターゲットが空白のままの場合、ここのデフォルト値も使用されます。",
              targetDescription:
                "オプション。プロバイダーのデフォルトの UID リストを上書きするには、入力します。デフォルト値を使用する場合は空白のままにします。",
            },
            topic_ids: {
              label: "デフォルトのトピック",
              description:
                "オプション。ここではテスト送信によりトピックが優先されます。チャンネルの直接検証を容易にするために、少なくとも 1 つのデフォルト UID またはトピックを入力することをお勧めします。",
              targetDescription:
                "オプション。プロバイダーのデフォルトのトピックを上書きするには、これを入力します。デフォルト値を使用する場合は空白のままにします。",
            },
            url: {
              label: "デフォルトメッセージジャンプ URL",
              targetLabel: "メッセージジャンプ URL",
              description:
                "オプション。ルールターゲットが入力されていない場合は、ここのジャンプリンクが使用されます。テスト送信にも使用されます。",
              targetDescription:
                "オプション。入力後、プロバイダーのデフォルトのジャンプ リンクを上書きします。空白のままにすると、デフォルト値が使用されます。",
            },
            verify_pay_type: {
              label: "デフォルトのサブスクリプション検証",
              targetLabel: "サブスクリプションの検証",
              description:
                "オプション。ルールターゲットが入力されていない場合、ここでのサブスクリプション検証戦略が使用されます。",
              targetDescription:
                "オプション。入力後、プロバイダーのデフォルトのサブスクリプション検証ポリシーが上書きされます。 「プロバイダーのデフォルトを継承」が選択されている場合、個別にオーバーライドされることはありません。",
              options: {
                "0": "未検証",
                "1": "有料会員限定",
                "2": "退会または期限切れのユーザーのみ",
                __inherit__: "プロバイダーのデフォルトを使用",
              },
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingAppToken: "WxPusher AppToken がありません",
            invalidTopicIds: "トピック ID 形式が正しくありません: {values}",
            recipientRequired:
              "WxPusher 少なくとも 1 つの UID またはトピック ID を設定する必要があります。これはプロバイダーのデフォルト設定に入力するか、ルール ターゲットで個別にオーバーライドできます。",
            targetsFailed:
              "{failed}/{total} WxPusher ターゲットの送信に失敗しました",
            requestFailed: "WxPusher リクエストが失敗しました",
          },
        },
        bark: {
          description:
            "は、Bark 公式オンライン バージョンまたは自社構築の Bark サーバーを通じて APN プッシュ通知を iPhone に送信します。",
          fields: {
            server_url: {
              label: "サービスアドレス",
              description:
                "公式オンライン バージョンのデフォルト値をそのまま使用します。自社構築の Bark サーバーを使用する場合は、サービス ルート アドレスを入力します。",
            },
            device_key: {
              description:
                "Barkアプリにデバイスキーがコピーされました。複数のキーを入力し、カンマで区切ることができます。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            level: {
              label: "通知レベル",
              description:
                "アクティブはデフォルトのインスタント リマインダーです。 timeSensitive はフォーカス モードを通過できます。クリティカルは重要な注意事項です。",
            },
            group: {
              label: "メッセージのグループ化",
              description:
                "オプション。 Barkクライアントでは同じグループが集約されて表示されます。",
            },
            sound: {
              label: "警報音",
              description:
                "オプション。サポートされているシステムまたはカスタム通知サウンドの名前を Bark に入力します。",
            },
            url: {
              label: "クリックしてジャンプ URL",
              description:
                "オプション。通知をクリックするとリンクが開きました。入力されていない場合は、メッセージ アクションの最初のリンクが最初に使用されます。",
            },
            icon: {
              label: "アイコン URL",
              description:
                "オプション。 iOS 15以降ではカスタムアイコンを表示できます。",
            },
            badge: {
              label: "下付き数字",
              description: "オプション。 Barkアプリアイコンに表示される数字。",
            },
            call: {
              label: "繰り返し鳴る",
              description: "有効にすると、Bark が約 30 秒間鳴り続けます。",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingDeviceKey: "Bark デバイスキーがありません",
            requestFailed: "Bark リクエストが失敗しました",
            pushFailed: "Bark プッシュに失敗しました",
            targetsFailed:
              "{failed}/{total} Bark ターゲットの送信に失敗しました",
          },
        },
        serverchan: {
          label: "サーバーソース",
          description:
            "Server Turbo・Turbo を通じて Markdown 通知を送信すると、Web サイトで設定されたデフォルトの受信チャネルを再利用できます。",
          fields: {
            server_url: {
              label: "サービスアドレス",
              description:
                "公式インターフェースのデフォルト値をそのままにしておきます。",
            },
            sendkey: {
              description:
                "サーバー Jiang·Turbo が SendKey を提供します。安全に保管してください。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            channel: {
              label: "メッセージチャンネル",
              description:
                "オプション。このプッシュのチャネルを動的に指定します。9|66 のように、| で区切られた最大 2 つの値を使用します。",
            },
            openid: {
              description:
                "オプション。テスト アカウントは openid を使用し、エンタープライズ WeChat アプリケーション メッセージは受信者の UID を使用します。サーバーソースのドキュメント形式に従って複数の値を入力してください。",
              placeholder: "openid1、openid2 または uid1|uid2",
            },
            short: {
              label: "カード概要",
              description:
                "オプション。メッセージ カードの短い概要 (最大 64 文字)。空白のままにすると、サーバーは自動的にテキストをインターセプトします。",
              placeholder: "ログイン異常、早めに対処してください",
            },
            noip: {
              label: "隠し通話 IP",
              description:
                "が有効になった後、このプッシュでは呼び出し元 IP は表示されません。",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingSendKey: "サーバーソース SendKey がありません",
            requestReturned: "サーバーソースリターン HTTP {status}",
            requestFailed: "サーバーソースリクエストが失敗しました",
          },
        },
        dingtalk: {
          label: "ディントークロボット",
          description:
            "DingTalk ロボット Webhook を使用して、Markdown 通知をグループ チャットに送信し、署名検証をサポートします。",
          fields: {
            webhook_url: {
              description:
                "DingTalk ロボットによって生成された完全な Webhook アドレス。",
            },
            secret: {
              label: "署名キー",
              description:
                "オプション。ロボットの「署名」が有効になっている場合は、セキュリティ設定ページに表示されるSECから始まるキーを入力してください。",
            },
            keyword_prefix: {
              label: "キーワードプレフィックス",
              description:
                "オプション。ロボットがカスタム キーワード検証を有効にしている場合は、固定キーワードを入力することをお勧めします。送信時にタイトルに自動的に付加されます。",
              placeholder: "監視アラーム",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            at_mobiles: {
              label: "@携帯番号",
              description:
                "オプション。複数の値はカンマまたは改行で区切る必要があり、グループのメンバーの携帯電話番号である必要があります。",
            },
            at_user_ids: {
              label: "@ ユーザー ID",
              description:
                "オプション。カンマまたは改行を使用して複数の値を区切ると、@userId がテキストに自動的に追加されます。",
            },
            is_at_all: {
              label: "@みんな",
              description:
                "有効にすると、isAtAll がリクエストに含まれ、@Everyone がテキストに追加されます。",
            },
          },
          mentionAll: "@みんな",
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingWebhookUrl: "がありません DingTalk Webhook URL",
            requestReturned: "DingTalk 戻る HTTP {status}",
            requestFailed: "DingTalk リクエストが失敗しました",
          },
        },
        feishu: {
          label: "フェイシュロボット",
          description:
            "は、Feishu ロボット Webhook を使用して、投稿リッチ テキスト通知をグループ チャットに送信し、署名検証をサポートします。",
          fields: {
            webhook_url: {
              description:
                "Feishu ロボットによって生成された完全な Webhook アドレス。",
            },
            secret: {
              label: "署名キー",
              description:
                "オプション。ロボットの「署名検証」が有効になっている場合は、セキュリティ設定からコピーしたキーを入力してください。",
            },
            keyword_prefix: {
              label: "キーワードプレフィックス",
              description:
                "オプション。ロボットがカスタム キーワード検証を有効にしている場合は、固定キーワードを入力することをお勧めします。送信時にタイトルに自動的に付加されます。",
              placeholder: "アプリケーションアラーム",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            mention_user_ids: {
              label: "@ ユーザー ID",
              description:
                "オプション。複数の値を区切るには、カンマまたは改行を使用します。すべての入力をサポートします。外部グループの @ 個人ユーザーに対しては、Open ID のみがサポートされます。",
            },
          },
          mentionAll: "皆さん",
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingWebhookUrl: "フェイシュがいなくなった Webhook URL",
            requestReturned: "フェイシュ帰還 HTTP {status}",
            requestFailed: "フェイシュのリクエストが失敗しました",
          },
        },
        webhook: {
          description:
            "HTTP JSON をサポートするアドレスに標準通知メッセージを送信します。",
          fields: {
            url: {
              description: "標準通知を受信する宛先アドレス JSON。",
            },
            method: {
              label: "リクエスト方法",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            shared_secret: {
              label: "共有キー",
              description:
                "オプション。入力すると、X-Fn-Knock-Signature リクエスト ヘッダーを介して送信されます。",
            },
            endpoint_path: {
              label: "追加パス",
              description: "オプション。ベースWebhookURLに接合して発送します。",
            },
            extra_headers_json: {
              label: "追加リクエストヘッダー JSON",
              description: 'オプション。例: {"X-Env":"prod"}。',
            },
            extra_body_json: {
              label: "追加リクエストボディJSON",
              description:
                "オプション。ペイロードにリンクされます。extra_body。",
            },
          },
          errors: {
            missingUrl: "がありません Webhook URL",
            requestReturned: "Webhook 戻る HTTP {status}",
            requestFailed: "Webhook リクエストが失敗しました",
          },
        },
        magicpush: {
          label: "MagicPushマジックプッシュ",
          description:
            "MagicPush の自社構築サービスを通じて構成済みのチャネルにプッシュ通知を送信し、標準プッシュ構成と MagicPush インバウンド構成をサポートします。",
          fields: {
            server_url: {
              label: "基本 API アドレス",
              description:
                "http://192.168.31.98:3000 などの MagicPush サービス ルート アドレスを入力します。 /api/pushまたは/api/inboundに入力されている場合は、そのまま使用されます。",
            },
            delivery_mode: {
              label: "配信モード",
              description:
                "標準プッシュは /api/push に送信されます。受信設定は /api/inbound/:token に送信され、MagicPush の受信ルールがフィールド マッピングを担当します。",
              options: {
                push: "標準プッシュ",
                inbound: "インバウンド構成",
              },
            },
            token: {
              description:
                "MagicPush インターフェーストークン。標準プッシュは、Authorization: Bearer で送信されます。受信設定は /api/inbound/:token に接続されます。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingBaseUrl: "MagicPush ベース API アドレスがありません",
            missingToken: "MagicPush トークンがありません",
            invalidBaseUrl: "MagicPush 基本 API 無効なアドレス",
            requestReturned: "MagicPush 戻る HTTP {status}",
            requestFailed: "MagicPush リクエストが失敗しました",
          },
        },
        telegram: {
          description:
            "インライン アクション ボタンを使用して、Telegram ボット API を介して、指定されたチャットまたはチャネルにテキスト通知を送信します。",
          fields: {
            server_url: {
              label: "ボット API アドレス",
              description:
                "公式ボット API デフォルト値のままにしてください。ネットワーク要因により公式アドレスにアクセスできない場合は、https://tgapi.fnknock.cnを入力して代わりにアドレスを転送できます。独自に構築したローカル ボット API サーバーを使用する場合は、そのルート アドレスを入力することもできます。",
            },
            bot_token: {
              description:
                "@BotFather を通じてロボットを作成した後に取得したボット トークン。",
            },
            chat_id: {
              description:
                "ターゲットチャット ID、またはチャンネルユーザー名 (例: @channelusername)。最初に @UserIdzhBot にメッセージを送信すると、チャット ID を取得できます。テスト送信でもこのターゲットが使用されます。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            message_thread_id: {
              description:
                "オプション。グループトピックに送信する場合は、該当のトピックID（message_thread_id）を記入してください。",
            },
            disable_notification: {
              label: "サイレント送信",
              description:
                "が有効な場合、Telegram はプロンプト音を再生せずにサイレントで配信されます。",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingBotToken: "Telegram ボットトークンがありません",
            missingChatId: "行方不明 Telegram チャット ID",
            requestReturned: "Telegram 戻る HTTP {status}",
            requestFailed: "Telegram リクエストが失敗しました",
          },
        },
        wecom: {
          label: "エンタープライズ WeChat メッセージ プッシュ",
          description:
            "エンタープライズ WeChat メッセージ プッシュ (グループ Webhook) を通じて、指定されたグループ チャットにテキストまたはマークダウン通知を送信します。",
          fields: {
            webhook_url: {
              description:
                "企業 WeChat メッセージ プッシュ ページによって生成された完全な Webhook アドレスを保管してください。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
            mentioned_list: {
              label: "アラートメンバーのユーザーID",
              description:
                "オプション。複数の値を区切るには、カンマまたは改行を使用します。 @all がサポートされています。",
            },
            mentioned_mobile_list: {
              label: "リマインダー携帯番号",
              description:
                "オプション。複数の値を区切るには、カンマまたは改行を使用します。 @all がサポートされています。",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingWebhookUrl: "がありません WeCom Webhook URL",
            requestReturned: "WeCom 戻る HTTP {status}",
            requestFailed: "WeCom リクエストが失敗しました",
          },
        },
        pushdeer: {
          description:
            "PushDeer 公式オンライン バージョンまたは自社構築サービスを通じて、バインドされたデバイスに Markdown 通知を送信します。",
          fields: {
            server_url: {
              label: "サービスアドレス",
              description:
                "公式オンライン バージョンのデフォルト値をそのまま使用してください。自己構築 PushDeer を使用する場合は、自己構築したサービスのルート アドレスを入力します。",
            },
            pushkey: {
              description:
                "PushDeer クライアントで生成された PushKey。複数のキーを入力し、カンマで区切ることができます。",
            },
            timeout_seconds: {
              label: "タイムアウト秒数",
            },
          },
          message: {
            fallbackTitle: "fn-knock お知らせ",
          },
          errors: {
            missingPushKey: "PushDeer プッシュキーがありません",
            requestReturned: "PushDeer 戻る HTTP {status}",
            apiReturnedCode: "PushDeer API リターンコード {code}",
            requestFailed: "PushDeer リクエストは失敗しました",
          },
        },
      },
    },
    routes: {
      createProviderFailed: "通知プロバイダーの作成に失敗しました",
      testProviderFailed: "通知プロバイダーのテストが失敗しました",
      getProviderFailed: "通知プロバイダーの取得に失敗しました",
      updateProviderFailed: "通知プロバイダーの更新に失敗しました",
      deleteProviderFailed: "通知プロバイダーの削除に失敗しました",
      createRuleFailed: "通知ルールの作成に失敗しました",
      updateRuleFailed: "通知ルールの更新に失敗しました",
      deleteRuleFailed: "通知ルールの削除に失敗しました",
      unsupportedDeliveryStatus: "サポートされていない配信ステータス",
      clearDeliveriesFailed: "配送記録をクリアできませんでした",
    },
    service: {
      unnamed: "無名",
      invalidJson: "{field} は合法である必要があります JSON",
      invalidSelectValue: "{field} 値が不正です",
      fieldRequired: "{field} を空にすることはできません",
      testMessage: {
        title: "テストのお知らせ",
        summary:
          "通知チャネル構成は正常であり、テスト メッセージが正常にトリガーされました。",
        bodyText:
          "これは、現在のプロバイダーの接続、構造化コピー、プレゼンテーションを検証するために Knock によって事前に送信されるテスト通知です。",
        bodyMarkdown:
          "**接続チェックに合格しました。 **\n\nこれは、現在のプロバイダーの接続、構造化コピー、および表示を確認するために、ノックノックによって送信される一方的なテスト通知です。",
        sendType: "送信タイプ",
        providerTest: "プロバイダーテスト",
        sentAt: "送信時間",
      },
      providerNotFound: "通知プロバイダーが存在しません",
      unsupportedProviderType: "サポートされていない通知プロバイダーの種類です",
      providerDefinitionMissing: "通知プロバイダー定義が存在しません",
      providerReferencedByRule:
        "このプロバイダーはまだルール「{rule}」によって参照されています",
      testSendFailed: "テスト送信に失敗しました",
      testSendSuccess: "テストは正常に送信されました",
      providerTypeMismatch: "プロバイダーのタイプが既存の構成と一致しません",
      providerTestName: "{provider} テスト",
      ruleProviderMissing: "ルールは存在しない通知プロバイダーを参照しています",
      invalidTemplateOverrideMode:
        "ターゲット テンプレート カバレッジ モードが不正です",
      unsupportedEventType: "サポートされていないシステム イベント タイプ",
      invalidGroupBy: "集計ディメンションが不正です",
      invalidMessageTemplateMode: "メッセージ テンプレート モードが不正です",
      invalidEventLevelFilter: "イベントレベルのフィルタ条件が不正です",
      invalidEventSourceFilter: "イベントソースのフィルター条件が不正です",
      targetRequired:
        "少なくとも 1 つの通知ターゲットをバインドする必要があります",
      duplicateEventRule:
        "このイベントにはすでに通知ルールが存在します。まず元のルールを削除してください。",
      ruleNotFound: "通知ルールが存在しません",
      deletedProvider: "プロバイダーが削除されました",
    },
  },
};
