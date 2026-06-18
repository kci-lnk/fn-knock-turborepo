export const jaJPShared = {
  binaryDownload: {
    currentPlatform: "現在のプラットフォーム",
    supported: "対応",
    unsupported: "サポートされていません",
    resourceStatus: "リソースステータス",
    readyLabel: "準備完了",
    pendingLabel: "準備ができていません",
    downloadProgress: "ダウンロードの進行状況",
    errorPrefix: "エラー:",
    downloadButton: "リソースをダウンロードする",
    redownload: "再ダウンロード",
    redownloadConfirmTitle: "リソースを再ダウンロードしますか?",
    redownloadConfirmDescription:
      "この操作により、既存のファイルが上書きされます。",
    confirmRedownload: "再ダウンロードの確認",
    delete: "削除",
    deleteConfirmTitle: "リソースの削除を確認しますか?",
    deleteConfirmDescription:
      "削除後、使用する前に再ダウンロードする必要があります。",
    confirmDelete: "削除を確認",
    downloading: "ダウンロード中です。お待ちください...",
    cancelTask: "タスクをキャンセル",
  },
  dataShareFilePicker: {
    title: "Feiniu からファイルを選択",
    description:
      "アプリケーションのルートディレクトリから読み取り可能なファイルを選択します。",
    directoryLabel: "アプリケーションファイル",
    alertTitle: "ディレクトリの読み取りに失敗しました",
    unavailableDescription:
      "このディレクトリは現在アクセスできません。アプリケーションがインストールされ、共有ディレクトリが生成されていることを確認してください。",
    confirmText: "このファイルを使用します",
    availableDescription: "{count} 利用可能なファイルが見つかりました",
    noMatchedFiles: "一致するファイルがありません",
    noMatchedDescription:
      "キーワードを変更するか、ディレクトリリストを更新してください。",
    emptyTitle: "共有ディレクトリにはまだファイルがありません",
    emptyDescription:
      "カレントディレクトリに条件に該当するファイルがありません。サポートされているファイルの種類を入力して選択してください。",
    noExtension: "サフィックスなし",
  },
  certSourceField: {
    uploadFromPhone: "携帯電話からアップロード",
    uploadFromComputer: "パソコンからアップロード",
    uploadFile: "ファイルをアップロード",
    chooseSourceTitle: "ファイルソースの選択",
    chooseSourceDescription:
      "まずインポート方法を選択してから、{label} ファイルを読み込みます。",
    localFileDescription:
      "デバイスから{types} ファイルを選択し、自動的に読み取ります",
    chooseFromFnos: "フェイニウからお選びください",
    sharedFileDescription:
      "{shareName} ルート ディレクトリおよび 3 レベル以内の既存のファイルを読み取ります",
    pickerTitle: "フェイニウから{label}を選択",
    pickerDescription:
      "まず証明書ファイルをアプリケーションデータ -> fn-knock ディレクトリに移動してください",
    readFile: "このファイルを読み込みます",
  },
  logViewer: {
    title: "動作ログ",
    emptyText: "ログはまだありません",
    lineCount: "{count} 行",
  },
  detailDialog: {
    close: "閉じる",
    copyLog: "ログのコピー",
    copySuccess: "ログがコピーされました",
    copyUnverified: "ログをコピーしようとしました",
    copyUnverifiedDescription: "ログがコピーされました",
    copyFailed: "ログのコピーに失敗しました",
    manualCopyHint:
      "現在のページは制限された環境で実行されている可能性があります。手動でコピーしてください。",
  },
  inlineCommentEditor: {
    placeholder: "備考を入力してください...",
    edit: "編集者メモ",
    save: "メモを保存",
    cancel: "編集をキャンセル",
    required: "コメント名を空にすることはできません",
    updateFailed: "メモを更新できませんでした",
  },
  defaultRouteConfirm: {
    clearTitle: "デフォルトルートのクリアを確認しますか?",
    setTitle: "デフォルトルートの設定を確認しますか?",
    clearFnosDescription:
      "{port} ポート サービスのデフォルト ルートをクリアしています。これは、Feiniu OS のデフォルトの入口アクセスに影響を与える可能性があります。",
    clearDescription:
      "クリア後はデフォルトルートがなくなり、失われたパスのリクエストが期待どおりに転送されなくなる可能性があります。",
    setDescription:
      "現在のデフォルト ルートは {port} ポート サービスです。他のルートに切り替えると、Feiniu OS のデフォルト エントリに影響が出る可能性があります。",
  },
  certForm: {
    sslCert: "SSL 証明書",
    privateKey: "秘密鍵",
  },
  pagedTableFooter: {
    total: "合計 {total} {itemText}",
    records: "レコード",
  },
  dnsCredentialBridge: {
    providers: {
      cloudflare: "Cloudflare",
      alidns: "アリババクラウド DNS",
      dnspod: "DNSPod",
      tencentcloud: "テンセントクラウド DNS",
      edgeone: "テンセントクラウド EdgeOne",
      edgeoneCname: "Tencent Cloud EdgeOne (CNAME アクセス)",
      godaddy: "ゴーダディ",
      porkbun: "豚まん",
      dynv6: "dynv6",
      duckdns: "DuckDNS",
    },
  },
  proxyTargetInputField: {
    hint: "左側でプロトコルを選択し、右側に IP とポートを入力します。ポートが未入力の場合は、プロトコルのデフォルトポートで自動補完されます。",
  },
  configCollapsibleCard: {
    editConfig: "設定の編集",
  },
  streamProtocolMultiSelect: {
    ariaLabel: "トランスポートプロトコル",
  },
};
