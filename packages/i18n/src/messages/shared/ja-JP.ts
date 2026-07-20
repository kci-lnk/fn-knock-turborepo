export const jaJPShared = {
  binaryDownload: {
    currentPlatform: "現在のプラットフォーム",
    supported: "対応",
    unsupported: "未対応",
    resourceStatus: "リソースの状態",
    readyLabel: "準備完了",
    pendingLabel: "未準備",
    downloadProgress: "ダウンロードの進捗",
    errorPrefix: "エラー: ",
    downloadButton: "リソースをダウンロード",
    redownload: "再ダウンロード",
    redownloadConfirmTitle: "このリソースを再ダウンロードしますか？",
    redownloadConfirmDescription: "既存のファイルは上書きされます。",
    confirmRedownload: "再ダウンロード",
    delete: "削除",
    deleteConfirmTitle: "このリソースを削除しますか？",
    deleteConfirmDescription: "再度使用するには、ダウンロードが必要です。",
    confirmDelete: "削除",
    downloading: "ダウンロード中です。しばらくお待ちください...",
    cancelTask: "ダウンロードをキャンセル",
  },
  dataShareFilePicker: {
    title: "FNOS からファイルを選択",
    description:
      "アプリケーションのルートディレクトリにある、読み取り可能なファイルを選択してください。",
    directoryLabel: "アプリケーションのファイル",
    alertTitle: "ディレクトリの読み取りに失敗しました",
    unavailableDescription:
      "このディレクトリにはアクセスできません。アプリがインストール済みで、共有ディレクトリが作成されていることを確認してください。",
    confirmText: "このファイルを使用",
    availableDescription: "利用可能なファイルが {count} 件見つかりました",
    noMatchedFiles: "一致するファイルがありません",
    noMatchedDescription:
      "キーワードを変更するか、ディレクトリリストを更新してください。",
    emptyTitle: "共有ディレクトリに利用可能なファイルがありません",
    emptyDescription:
      "このディレクトリに対応形式のファイルがありません。対応するファイルを追加してから、もう一度選択してください。",
    noExtension: "拡張子なし",
  },
  certSourceField: {
    uploadFromPhone: "スマートフォンからアップロード",
    uploadFromComputer: "パソコンからアップロード",
    uploadFile: "ファイルをアップロード",
    chooseSourceTitle: "ファイルの取得元を選択",
    chooseSourceDescription:
      "インポート方法を選択して、{label} ファイルを読み込みます。",
    localFileDescription:
      "このデバイスから {types} ファイルを選択して、自動的に読み込みます",
    chooseFromFnos: "FNOS から選択",
    sharedFileDescription:
      "{shareName} のルートディレクトリから 3 階層以内にあるファイルを読み込みます",
    pickerTitle: "FNOS から {label} を選択",
    pickerDescription:
      "先に証明書ファイルを「アプリケーションデータ → fn-knock」へ移動してください",
    readFile: "このファイルを読み込む",
  },
  logViewer: {
    title: "動作ログ",
    emptyText: "ログはまだありません",
    lineCount: "{count} 行",
  },
  detailDialog: {
    close: "閉じる",
    copyLog: "ログをコピー",
    copySuccess: "ログをコピーしました",
    copyUnverified: "ログのコピーを試行しました",
    copyUnverifiedDescription:
      "コピー結果を確認できませんでした。クリップボードを確認してください。",
    copyFailed: "ログのコピーに失敗しました",
    manualCopyHint:
      "このページは制限された環境で動作している可能性があります。手動でコピーしてください。",
  },
  inlineCommentEditor: {
    placeholder: "コメントを入力...",
    edit: "コメントを編集",
    save: "コメントを保存",
    cancel: "編集をキャンセル",
    required: "コメントを入力してください",
    updateFailed: "コメントの更新に失敗しました",
  },
  defaultRouteConfirm: {
    clearTitle: "デフォルトルートを解除しますか？",
    setTitle: "デフォルトルートに設定しますか？",
    clearFnosDescription:
      "ポート {port} のサービスをデフォルトルートから解除します。FNOS のデフォルトのアクセス先に影響する場合があります。",
    clearDescription:
      "解除するとデフォルトルートがなくなるため、どのパスにも一致しないリクエストが正しく転送されない場合があります。",
    setDescription:
      "現在のデフォルトルートはポート {port} のサービスです。別のルートに切り替えると、FNOS のデフォルトのアクセス先に影響する場合があります。",
  },
  certForm: {
    sslCert: "SSL 証明書",
    privateKey: "秘密鍵",
  },
  pagedTableFooter: {
    total: "合計 {total} {itemText}",
    records: "件",
    pageSizeOption: "{count} 件",
  },
  dnsCredentialBridge: {
    providers: {
      cloudflare: "Cloudflare",
      alidns: "Alibaba Cloud DNS",
      dnspod: "DNSPod",
      tencentcloud: "Tencent Cloud DNS",
      edgeone: "Tencent Cloud EdgeOne",
      edgeoneCname: "Tencent Cloud EdgeOne（CNAME 接続）",
      godaddy: "GoDaddy",
      porkbun: "Porkbun",
      dynv6: "dynv6",
      duckdns: "DuckDNS",
    },
  },
  proxyTargetInputField: {
    hint: "左側でプロトコルを選び、右側に IP アドレスとポートを入力します。ポートを省略すると、フォーカスを外したときにプロトコルのデフォルト値が補完されます。",
    suggestionsLabel: "転送先アドレスの候補",
  },
  configCollapsibleCard: {
    editConfig: "設定の編集",
  },
  streamProtocolMultiSelect: {
    ariaLabel: "トランスポートプロトコル",
  },
};
