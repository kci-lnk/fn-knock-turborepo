export const koKRShared = {
  binaryDownload: {
    currentPlatform: "현재 플랫폼",
    supported: "지원됨",
    unsupported: "지원되지 않음",
    resourceStatus: "자원현황",
    readyLabel: "준비",
    pendingLabel: "준비되지 않음",
    downloadProgress: "다운로드 진행 상황",
    errorPrefix: "오류: ",
    downloadButton: "리소스 다운로드",
    redownload: "다시 다운로드",
    redownloadConfirmTitle: "이 리소스를 다시 다운로드하시겠습니까?",
    redownloadConfirmDescription: "기존 파일을 덮어씁니다.",
    confirmRedownload: "다시 다운로드",
    delete: "삭제",
    deleteConfirmTitle: "이 리소스를 삭제하시겠습니까?",
    deleteConfirmDescription: "사용하기 전에 다시 다운로드해야 합니다.",
    confirmDelete: "삭제",
    downloading: "다운로드 중입니다. 잠시 기다려 주세요...",
    cancelTask: "작업 취소",
  },
  dataShareFilePicker: {
    title: "FNOS에서 파일을 선택하세요",
    description:
      "애플리케이션 루트 디렉터리에서 읽을 수 있는 파일을 선택합니다.",
    directoryLabel: "애플리케이션 파일",
    alertTitle: "디렉터리를 읽지 못했습니다.",
    unavailableDescription:
      "디렉터리에 아직 액세스할 수 없습니다. 앱이 설치되어 있고 공유 디렉터리가 생성되었는지 확인하세요.",
    confirmText: "이 파일을 사용하세요",
    availableDescription: "{count} 사용 가능한 파일을 찾았습니다.",
    noMatchedFiles: "일치하는 파일이 없습니다.",
    noMatchedDescription:
      "다른 키워드를 사용하거나 디렉터리 목록을 새로 고치세요.",
    emptyTitle: "공유 디렉터리에 사용 가능한 파일이 없습니다.",
    emptyDescription:
      "이 디렉터리에는 일치하는 파일이 없습니다. 지원되는 파일 형식을 추가한 후 다시 선택하세요.",
    noExtension: "확장 없음",
  },
  certSourceField: {
    uploadFromPhone: "휴대전화에서 업로드",
    uploadFromComputer: "컴퓨터에서 업로드",
    uploadFile: "파일 업로드",
    chooseSourceTitle: "파일 소스 선택",
    chooseSourceDescription:
      "가져오기 방법을 선택한 다음 {label} 파일을 읽으세요.",
    localFileDescription:
      "이 장치에서 {types} 파일을 선택하고 자동으로 읽습니다.",
    chooseFromFnos: "FNOS에서 선택하세요",
    sharedFileDescription:
      "최대 3개 수준까지 {shareName} 루트 디렉터리에서 기존 파일을 읽습니다.",
    pickerTitle: "FNOS에서 {label}을 선택하세요",
    pickerDescription:
      "인증서 파일을 애플리케이션 데이터로 이동 -> fn-knock 먼저",
    readFile: "이 파일 읽기",
  },
  logViewer: {
    title: "런타임 로그",
    emptyText: "아직 로그가 없습니다.",
    lineCount: "{count} 라인",
  },
  detailDialog: {
    close: "닫기",
    copyLog: "로그 복사",
    copySuccess: "로그가 복사되었습니다.",
    copyUnverified: "로그 복사를 시도했습니다.",
    copyUnverifiedDescription: "로그가 복사되었습니다.",
    copyFailed: "로그를 복사하지 못했습니다.",
    manualCopyHint:
      "이 페이지는 제한된 환경에서 실행 중일 수 있습니다. 수동으로 복사하세요.",
  },
  inlineCommentEditor: {
    placeholder: "댓글을 입력하세요...",
    edit: "댓글 수정",
    save: "댓글 저장",
    cancel: "편집 취소",
    required: "댓글 이름은 비워둘 수 없습니다.",
    updateFailed: "댓글을 업데이트하지 못했습니다.",
  },
  defaultRouteConfirm: {
    clearTitle: "기본 경로를 삭제하시겠습니까?",
    setTitle: "기본 경로를 설정하시겠습니까?",
    clearFnosDescription:
      "{port} 포트에서 서비스의 기본 경로를 지우고 있습니다. 이는 기본 FNOS 항목에 영향을 미칠 수 있습니다.",
    clearDescription:
      "이를 지우면 기본 경로가 더 이상 존재하지 않습니다. 경로와 일치하지 않는 요청은 예상대로 전달되지 않을 수 있습니다.",
    setDescription:
      "현재 기본 경로는 {port} 포트의 서비스를 가리킵니다. 다른 경로로 전환하면 기본 FNOS 항목에 영향을 미칠 수 있습니다.",
  },
  certForm: {
    sslCert: "SSL 인증서",
    privateKey: "개인 키",
  },
  pagedTableFooter: {
    total: "{total} {itemText}",
    records: "기록",
  },
  dnsCredentialBridge: {
    providers: {
      cloudflare: "Cloudflare",
      alidns: "Alibaba Cloud DNS",
      dnspod: "DNSPod",
      tencentcloud: "Tencent Cloud DNS",
      edgeone: "Tencent Cloud EdgeOne",
      edgeoneCname: "Tencent Cloud EdgeOne(CNAME 액세스)",
      godaddy: "고대디",
      porkbun: "돼지고기",
      dynv6: "dynv6",
      duckdns: "덕DNS",
    },
  },
  proxyTargetInputField: {
    hint: "왼쪽에서 프로토콜을 선택하고 오른쪽에서 IP와 포트를 입력하세요. 포트를 생략하면 블러시 {port}으로 채워집니다.",
  },
  configCollapsibleCard: {
    editConfig: "구성 수정",
  },
  streamProtocolMultiSelect: {
    ariaLabel: "전송 프로토콜",
  },
};
