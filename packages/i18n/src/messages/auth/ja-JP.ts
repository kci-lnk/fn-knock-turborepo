export const jaJPAuth = {
  autoIpGrantComment: "ログイン後自動認証",
  title: "セキュリティ検証",
  captchaFirst: "まず、以下の人間とマシンの検証を完了してください。",
  otpPrompt: "ログインを完了するには、6 桁の動的パスワードを入力してください",
  passwordPrompt: "ユーザー名とパスワードを入力してログインしてください",
  notRobot: "私はロボットではありません",
  verified: "検証に合格しました",
  verifying: "確認中...",
  wait: "お待ちください...",
  verifyError: "検証エラー",
  turnstileMissing:
    "現在の回転式改札口の構成は完了していません。管理者に連絡してサイト キーを入力してください。",
  turnstileScriptLoadFailed: "回転木戸スクリプトの読み込みに失敗しました",
  turnstileRenderFailed:
    "回転木戸のレンダリングに失敗しました。後でもう一度試してください。",
  turnstileTimeout:
    "改札口の検証がタイムアウトしました。もう一度お試しください。",
  powUnsupportedAlgorithm: "サポートされていない PoW アルゴリズム",
  powInvalidChallenge: "PoWチャレンジデータが無効です",
  powSolveFailed:
    "PoW ソリューションが失敗しました。ページを更新してもう一度お試しください。",
  locationResolving: "テリトリー分析中...",
  locationUnavailable: "領土はまだ取得されていません",
  openGithub: "GitHubプロジェクトページを開きます",
  menu: "メニュー",
  or: "OR",
  loginWithProvider: "{provider}を使用してログインします",
  retryAfterSeconds: "{seconds} 秒後に再試行してください",
  verifyNow: "今すぐ確認",
  passwordLogin: "ユーザー名とパスワードでログイン",
  totpLogin: "TOTP ログイン",
  username: "ユーザー名",
  password: "パスワード",
  showPassword: "パスワードを表示",
  hidePassword: "パスワードを非表示",
  usernamePasswordRequired: "ユーザー名とパスワードを入力してください",
  passkeyLogin: "Passkey ワンクリックログイン",
  tip: "ヒント",
  ok: "OK",
  rememberMe: "私を覚えていてください",
  passkeyBindTitle: "Passkeyをオンにする ワンクリックログイン",
  passkeyBindDescription:
    "は現在のデバイスの Passkey にバインドされていますか?バインド後は、ワンクリックで直接ログインできます。",
  passkeyBindSkipPrompt: "もう思い出さないでください",
  passkeyBindLater: "詳細は後ほど",
  passkeyBindNow: "現在営業中",
  captchaConfigLoadFailed:
    "確認コード設定の読み込みに失敗しました。ページを更新して、もう一度お試しください。",
  captchaFailed: "人間とマシンの検証に失敗しました。もう一度お試しください。",
  loggedOutLoginIpGrant:
    "現在のブラウザ セッションはログアウトされ、ログイン時に付与された現在の IP アクセスは取り消されました。",
  loggedOutManualWhitelist:
    "現在のブラウザセッションが終了しました。管理者のホワイトリストは引き続き有効です。",
  loggedOutLocalExempt:
    "現在のブラウザセッションが終了しました。現在のネットワークはまだホワイトリストから除外されています。",
  loggedOutDefault:
    "現在のブラウザセッションが終了しました。再確認してください。",
  redirectLoopBlocked:
    "この確認ページと対象サービスの間で繰り返しリダイレクトが検出されたため、自動リダイレクトを一時停止しました。続行するには、このページでもう一度確認してください。",
  redirectTargetBlocked:
    "ログイン先が無効であるか、この確認ページ自身を指しているため、繰り返しリダイレクトを停止しました。元のサービスを開き直すか、管理者にログイン先の設定を確認してください。",
  retrySuffix: "、{seconds} 秒後にもう一度お試しください",
  invalidOtpLength: "完全な 6 桁の確認コードを入力してください",
  loginFailed: "認証に失敗しました。もう一度お試しください。",
  passkeyNoResponse: "Passkey 応答が得られませんでした",
  passkeyVerifyFailed: "Passkey 検証に失敗しました",
  passkeyLoginFailed:
    "Passkey ログインに失敗しました。もう一度お試しください。",
  oidcStartFailed: "外部ログインを開始できません",
  oidcLoginFailed: "外部ログインに失敗しました。もう一度お試しください。",
  passkeyBindInvalid:
    "バインディング認証情報が無効です。再度ログインしてください。",
  passkeyBindFailed: "Passkey バインドに失敗しました",
  passkeyCreateCancelled:
    "Passkey の作成が完了しませんでした。キャンセルまたはタイムアウトした可能性があります。",
  passkeyCreateUnavailable:
    "システムで Passkey を作成できませんでした。画面ロックとパスワード マネージャーが有効になっていることを確認して、もう一度お試しください。",
  passkeyAlreadyRegistered:
    "このデバイスまたはパスワードマネージャーにはすでに Passkey があります。そのまま使用できます。",
  home: {
    statusTitles: {
      browserSession: "現在のブラウザセッションが認証されました",
      sessionMigration: "ブラウザセッションが復元されました",
      fnosFingerprintSession: "デバイスの指紋セッションが復元されました",
      manualWhitelist: "ホワイトリストアクセスが許可されました",
      localExempt: "現在のネットワークが解放されました",
      fnosShare: "共有アクセスが許可されました",
      loginIpGrant: "セキュリティ検証に合格しました",
    },
    statusDescriptions: {
      browserSession:
        "現在のブラウザ セッションではアクセスがすでに許可されています",
      sessionMigration:
        "ネットワーク切り替えにより、現在のブラウザセッションが復元されました。",
      fnosFingerprintSession:
        "Feiniu デバイスの指紋セッションにより現在のアクセスが復元されました",
      manualWhitelist: "現在、IP が管理者のホワイトリストに含まれています",
      localExempt:
        "現在のネットワークアドレスはホワイトリストのない範囲に属しています",
      fnosShare: "現在のアクセスは Feiniu 共有リンクによって許可されています",
      loginIpGrant: "あなたの IP はアクセスを許可されました",
    },
    logoutHints: {
      browserSession:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了してください。終了後、再度入力する前に現在のブラウザを再認証する必要があります。",
      sessionMigration:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了してください。終了後、現在のブラウザを再認証する必要があり、このセッション移行に関連付けられた承認は取り消されます。",
      fnosFingerprintSession:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了してください。終了すると、現在復元されているデバイスの指紋セッションが終了し、関連する認証が取り消されます。",
      loginIpGrant:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了してください。終了すると、現在のブラウザ セッションが終了し、ログイン時に付与された現在の IP アクセスが取り消されます。",
      manualWhitelist:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了してください。終了すると、現在のブラウザ セッションが終了するだけで、管理者のホワイトリストは削除されません。",
      localExempt:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了してください。終了すると、現在のブラウザ セッションが終了するだけであり、ホワイトリストなしのネットワーク アクセスの範囲は変更されません。",
      fnosShare:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了してください。終了すると、現在の共有アクセス セッションが終了するため、共有リンクに再入力する必要があります。",
      default:
        "アクセスする必要がなくなった場合は、下のボタンをクリックして終了し、認証を取り消してください。",
    },
    logoutDialogDescriptions: {
      browserSession:
        "終了すると現在のブラウザセッションが終了し、再度入る前に再認証が必要になります。",
      sessionMigration:
        "終了すると、現在のブラウザ セッションが終了し、このセッションの移行に関連付けられた認証が取り消されます。",
      fnosFingerprintSession:
        "終了後、現在復元されているデバイスの指紋セッションが終了し、関連する認証が取り消されます。",
      loginIpGrant:
        "サインアウトすると、現在のブラウザ セッションが終了し、このログインによって付与された現在の IP アクセスが取り消されます。",
      manualWhitelist:
        "終了すると現在のブラウザセッションが終了するだけで、管理者が設定したホワイトリストは削除されません。",
      localExempt:
        "終了すると現在のブラウザセッションが終了するだけで、現在のネットワークのホワイトリストフリー属性は変更されません。",
      fnosShare:
        "終了後、現在の共有アクセスセッションは終了します。再度アクセスする必要がある場合は、共有リンクを再入力してください。",
      default:
        "退出後は現在のアクセス権限が取り消され、再度入場する前に再認証が必要となります。",
    },
    enablePasskey: "Passkeyをオンにする ワンクリックログイン",
    passkeySupportedUnbound:
      "現在のブラウザは Passkey をサポートしていますが、まだバインドされていません",
    addPasskey: "別の Passkey を追加",
    passkeyAvailableAddDevice:
      "このアカウントにはすでに Passkey があります。このデバイスに同期されていない場合は追加できます。",
    logoutDelay: "{seconds}秒後にログアウトボタンが表示されます",
    logout: "ログアウト",
    logoutConfirmTitle: "ログアウトを確認します",
    confirmLogout: "終了を確認してください",
    passkeyTokenMissing: "バインディング認証情報を取得できません",
  },
  oidcBind: {
    title: "外部アカウントをバインドする",
    checkingInvite: "招待リンクを確認中...",
    bindTo: "バインド先",
    useProvider: "{provider}を使用してバインドします",
    invalidInvite: "招待リンクは利用できません",
    wait: "お待ちください",
    selectProvider: "プロバイダーを選択してログインとバインドを完了します",
    missingToken: "招待リンクにトークンがありません",
    noProviders: "現在、利用可能な外部ログインプロバイダーはありません",
    inviteExpired: "招待リンクの有効期限が切れました",
    startFailed: "外部アカウントのバインドを開始できません",
    bindFailed:
      "外部アカウントのバインドに失敗しました。もう一度お試しください。",
  },
};
