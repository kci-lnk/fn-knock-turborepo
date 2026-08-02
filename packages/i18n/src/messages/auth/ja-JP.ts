export const jaJPAuth = {
  autoIpGrantComment: "ログイン時に自動で許可",
  title: "セキュリティ認証",
  captchaFirst: "最初に、以下のボット対策認証を完了してください",
  otpPrompt: "6 桁のワンタイムパスワードを入力してください",
  passwordPrompt: "ユーザー名とパスワードを入力してログインしてください",
  ldapPrompt: "LDAP アカウントとパスワードを入力してください",
  notRobot: "私はロボットではありません",
  verified: "認証済み",
  verifying: "確認中...",
  wait: "お待ちください...",
  verifyError: "認証エラー",
  turnstileMissing:
    "Turnstile が設定されていません。管理者にサイトキーの設定を依頼してください。",
  turnstileScriptLoadFailed: "Turnstile スクリプトの読み込みに失敗しました",
  turnstileRenderFailed:
    "Turnstile を表示できませんでした。しばらくしてから、もう一度お試しください。",
  turnstileTimeout:
    "Turnstile の認証がタイムアウトしました。もう一度お試しください。",
  powUnsupportedAlgorithm: "未対応の PoW アルゴリズムです",
  powInvalidChallenge: "PoW チャレンジデータが無効です",
  powSolveFailed:
    "PoW の計算に失敗しました。ページを更新して、もう一度お試しください。",
  locationResolving: "位置情報を取得中...",
  locationUnavailable: "位置情報を取得できません",
  openGithub: "GitHub プロジェクトページを開く",
  menu: "メニュー",
  or: "または",
  loginWithProvider: "{provider}でログイン",
  retryAfterSeconds: "{seconds}秒後に再試行",
  verifyNow: "今すぐ認証",
  passwordLogin: "パスワードでログイン",
  totpLogin: "TOTP でログイン",
  ldapLogin: "LDAP でログイン",
  ldapProvider: "ディレクトリプロバイダー",
  ldapProviderRequired: "ディレクトリプロバイダーを選択してください",
  ldapUsername: "LDAP ユーザー名",
  ldapPassword: "LDAP パスワード",
  username: "ユーザー名",
  password: "パスワード",
  showPassword: "パスワードを表示",
  hidePassword: "パスワードを非表示",
  usernamePasswordRequired: "ユーザー名とパスワードを入力してください",
  passkeyLogin: "パスキーでログイン",
  tip: "お知らせ",
  ok: "OK",
  rememberMe: "ログイン状態を保持",
  passkeyBindTitle: "パスキーログインを有効化",
  passkeyBindDescription:
    "このデバイスにパスキーを登録すると、次回から簡単にログインできます。",
  passkeyBindSkipPrompt: "今後は表示しない",
  passkeyBindLater: "後で",
  passkeyBindNow: "今すぐ有効化",
  captchaConfigLoadFailed:
    "ボット対策認証の設定を読み込めませんでした。ページを更新して、もう一度お試しください。",
  captchaFailed: "ボット対策認証に失敗しました。もう一度お試しください。",
  loggedOutLoginIpGrant:
    "ブラウザセッションからログアウトし、ログイン時に付与された IP アクセス許可も取り消しました。",
  loggedOutManualWhitelist:
    "ブラウザセッションからログアウトしました。管理者のホワイトリストは引き続き有効です。",
  loggedOutLocalExempt:
    "ブラウザセッションからログアウトしました。このネットワークは引き続きホワイトリストチェックの対象外です。",
  loggedOutDefault:
    "ブラウザセッションからログアウトしました。もう一度認証してください。",
  redirectLoopBlocked:
    "この認証ページと対象サービスとの間でリダイレクトが繰り返されたため、自動転送を停止しました。続行するには、このページでもう一度認証してください。",
  redirectTargetBlocked:
    "ログイン先が無効か、この認証ページ自体を指しているため、繰り返しリダイレクトを停止しました。元のサービスを開き直すか、管理者にお問い合わせください。",
  retrySuffix: " {seconds}秒後にもう一度お試しください。",
  invalidOtpLength: "6 桁の認証コードをすべて入力してください",
  loginFailed: "認証に失敗しました。もう一度お試しください。",
  passkeyNoResponse: "パスキーから応答がありませんでした",
  passkeyVerifyFailed: "パスキーの検証に失敗しました",
  passkeyLoginFailed:
    "パスキーでログインできませんでした。もう一度お試しください。",
  oidcStartFailed: "外部サービスでのログインを開始できませんでした",
  oidcLoginFailed: "外部ログインに失敗しました。もう一度お試しください。",
  passkeyBindInvalid:
    "パスキー登録用の認証情報が無効です。もう一度ログインしてください。",
  passkeyBindFailed: "パスキーの登録に失敗しました",
  passkeyCreateCancelled:
    "パスキーの作成がキャンセルされたか、タイムアウトしました",
  passkeyCreateUnavailable:
    "パスキーを作成できませんでした。画面ロックとパスワードマネージャーが有効になっていることを確認して、もう一度お試しください。",
  passkeyAlreadyRegistered:
    "このデバイスまたはパスワードマネージャーには、すでにパスキーが登録されています。そのまま利用できます。",
  home: {
    statusTitles: {
      browserSession: "このブラウザは認証済みです",
      sessionMigration: "ブラウザセッションを復元しました",
      fnosFingerprintSession: "デバイス指紋セッションを復元しました",
      manualWhitelist: "ホワイトリストによるアクセス",
      localExempt: "このネットワークからアクセスできます",
      fnosShare: "共有リンクによるアクセス",
      loginIpGrant: "セキュリティ認証が完了しました",
    },
    statusDescriptions: {
      browserSession: "このブラウザセッションにはアクセスが許可されています",
      sessionMigration:
        "ネットワークの変更後にブラウザセッションを復元しました",
      fnosFingerprintSession:
        "FNOS のデバイス指紋セッションによりアクセスを復元しました",
      manualWhitelist: "現在の IP は管理者のホワイトリストに登録されています",
      localExempt:
        "このネットワークアドレスはホワイトリストチェックの対象外です",
      fnosShare: "FNOS の共有リンクによりアクセスが許可されています",
      loginIpGrant: "現在の IP にアクセスが許可されています",
    },
    logoutHints: {
      browserSession:
        "アクセスが不要になったら、下のボタンからログアウトしてください。次回アクセスするときは、このブラウザでもう一度認証が必要です。",
      sessionMigration:
        "アクセスが不要になったら、下のボタンからログアウトしてください。次回アクセスするときは再認証が必要になり、セッション移行に伴うアクセス許可も取り消されます。",
      fnosFingerprintSession:
        "アクセスが不要になったら、下のボタンからログアウトしてください。復元したデバイス指紋セッションが終了し、関連するアクセス許可も取り消されます。",
      loginIpGrant:
        "アクセスが不要になったら、下のボタンからログアウトしてください。ブラウザセッションが終了し、ログイン時に付与された IP アクセス許可も取り消されます。",
      manualWhitelist:
        "アクセスが不要になったら、下のボタンからログアウトしてください。ブラウザセッションだけが終了し、管理者のホワイトリストは維持されます。",
      localExempt:
        "アクセスが不要になったら、下のボタンからログアウトしてください。ブラウザセッションだけが終了し、このネットワークのホワイトリスト除外設定は変更されません。",
      fnosShare:
        "アクセスが不要になったら、下のボタンからログアウトしてください。共有アクセスセッションが終了するため、次回は共有リンクを開き直す必要があります。",
      default:
        "アクセスが不要になったら、下のボタンからログアウトしてアクセス許可を取り消してください。",
    },
    logoutDialogDescriptions: {
      browserSession:
        "ログアウトするとブラウザセッションが終了します。次回アクセスするときは、もう一度認証が必要です。",
      sessionMigration:
        "ログアウトするとブラウザセッションが終了し、セッション移行に伴うアクセス許可も取り消されます。",
      fnosFingerprintSession:
        "ログアウトすると、復元したデバイス指紋セッションが終了し、関連するアクセス許可も取り消されます。",
      loginIpGrant:
        "ログアウトするとブラウザセッションが終了し、今回のログインで付与された IP アクセス許可も取り消されます。",
      manualWhitelist:
        "ログアウトしてもブラウザセッションだけが終了し、管理者のホワイトリストは維持されます。",
      localExempt:
        "ログアウトしてもブラウザセッションだけが終了し、このネットワークのホワイトリスト除外設定は変更されません。",
      fnosShare:
        "ログアウトすると共有アクセスセッションが終了します。次回アクセスするときは、共有リンクを開き直してください。",
      default:
        "ログアウトすると現在のアクセス許可が取り消されます。次回アクセスするときは、もう一度認証が必要です。",
    },
    enablePasskey: "パスキーログインを有効化",
    passkeySupportedUnbound:
      "このブラウザはパスキーに対応していますが、まだ登録されていません",
    addPasskey: "別のパスキーを追加",
    passkeyAvailableAddDevice:
      "このアカウントにはパスキーが登録されています。このデバイスに同期されていない場合は、別のパスキーを追加できます。",
    logoutDelay: "ログアウトボタンは{seconds}秒後に表示されます",
    logout: "ログアウト",
    logoutConfirmTitle: "ログアウトの確認",
    confirmLogout: "ログアウト",
    passkeyTokenMissing: "パスキー登録用の認証情報を取得できません",
  },
  ldapBind: {
    title: "LDAP アカウントを連携",
    description: "LDAP の本人確認を行い、既存の TOTP 認証情報に連携します",
    checkingInvite: "招待リンクを確認中...",
    bindTo: "連携先",
    missingToken: "招待リンクにトークンが含まれていません",
    inviteExpired: "招待リンクの有効期限が切れています",
    bindNow: "確認して連携",
    bindFailed:
      "LDAP アカウントを連携できませんでした。もう一度お試しください。",
  },
  oidcBind: {
    title: "外部アカウントを連携",
    checkingInvite: "招待リンクを確認中...",
    bindTo: "連携先",
    useProvider: "{provider}で連携",
    invalidInvite: "招待リンクを利用できません",
    wait: "しばらくお待ちください",
    selectProvider: "プロバイダーを選択してログインし、アカウントを連携します",
    missingToken: "招待リンクにトークンが含まれていません",
    noProviders: "利用可能な外部ログインプロバイダーがありません",
    inviteExpired: "招待リンクの有効期限が切れています",
    startFailed: "外部アカウントの連携を開始できませんでした",
    bindFailed:
      "外部アカウントを連携できませんでした。もう一度お試しください。",
  },
};
