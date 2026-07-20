export const koKRShared = {
  binaryDownload: {
    currentPlatform: "현재 플랫폼",
    supported: "지원",
    unsupported: "미지원",
    resourceStatus: "리소스 상태",
    readyLabel: "사용 가능",
    pendingLabel: "사용 불가",
    downloadProgress: "다운로드 진행률",
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
    title: "FNOS에서 파일 선택",
    description:
      "애플리케이션 루트 디렉터리에서 읽을 수 있는 파일을 선택하세요.",
    directoryLabel: "애플리케이션 파일",
    alertTitle: "디렉터리를 읽지 못했습니다.",
    unavailableDescription:
      "디렉터리에 아직 접근할 수 없습니다. 앱이 설치되어 있고 공유 디렉터리가 생성되었는지 확인하세요.",
    confirmText: "이 파일 사용",
    availableDescription: "사용 가능한 파일 {count}개",
    noMatchedFiles: "일치하는 파일이 없습니다.",
    noMatchedDescription:
      "다른 키워드를 사용하거나 디렉터리 목록을 새로 고치세요.",
    emptyTitle: "공유 디렉터리에 사용 가능한 파일이 없습니다.",
    emptyDescription:
      "이 디렉터리에는 일치하는 파일이 없습니다. 지원되는 파일 형식을 추가한 후 다시 선택하세요.",
    noExtension: "확장자 없음",
  },
  certSourceField: {
    uploadFromPhone: "휴대전화에서 업로드",
    uploadFromComputer: "컴퓨터에서 업로드",
    uploadFile: "파일 업로드",
    chooseSourceTitle: "파일 가져올 위치 선택",
    chooseSourceDescription:
      "가져올 위치를 선택한 다음 {label} 파일을 불러오세요.",
    localFileDescription:
      "이 기기에서 {types} 파일을 선택하면 자동으로 불러옵니다.",
    chooseFromFnos: "FNOS에서 선택",
    sharedFileDescription:
      "{shareName} 루트 디렉터리에서 최대 3단계 아래의 기존 파일을 불러옵니다.",
    pickerTitle: "FNOS에서 {label} 선택",
    pickerDescription:
      "먼저 인증서 파일을 애플리케이션 데이터 > fn-knock으로 옮기세요.",
    readFile: "이 파일 불러오기",
  },
  logViewer: {
    title: "런타임 로그",
    emptyText: "아직 로그가 없습니다.",
    lineCount: "{count}줄",
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
    placeholder: "메모 입력...",
    edit: "메모 수정",
    save: "메모 저장",
    cancel: "편집 취소",
    required: "메모를 입력하세요.",
    updateFailed: "메모를 수정하지 못했습니다.",
  },
  defaultRouteConfirm: {
    clearTitle: "기본 라우트를 해제하시겠습니까?",
    setTitle: "기본 라우트로 설정하시겠습니까?",
    clearFnosDescription:
      "{port} 포트 서비스의 기본 라우트를 해제하면 FNOS 기본 항목에 영향을 줄 수 있습니다.",
    clearDescription:
      "해제 후에는 기본 라우트가 없습니다. 경로가 일치하지 않는 요청이 예상대로 전달되지 않을 수 있습니다.",
    setDescription:
      "현재 기본 라우트는 {port} 포트 서비스를 가리킵니다. 다른 라우트로 바꾸면 FNOS 기본 항목에 영향을 줄 수 있습니다.",
  },
  certForm: {
    sslCert: "SSL 인증서",
    privateKey: "개인 키",
  },
  pagedTableFooter: {
    total: "{total} {itemText}",
    records: "개",
    pageSizeOption: "{count} 행",
  },
  dnsCredentialBridge: {
    providers: {
      cloudflare: "Cloudflare",
      alidns: "Alibaba Cloud DNS",
      dnspod: "DNSPod",
      tencentcloud: "Tencent Cloud DNS",
      edgeone: "Tencent Cloud EdgeOne",
      edgeoneCname: "Tencent Cloud EdgeOne(CNAME 연결)",
      godaddy: "GoDaddy",
      porkbun: "Porkbun",
      dynv6: "dynv6",
      duckdns: "DuckDNS",
    },
  },
  proxyTargetInputField: {
    hint: "왼쪽에서 프로토콜을 선택하고 오른쪽에서 IP와 포트를 입력하세요. 포트를 생략하면 프로토콜 기본 포트로 자동 채워집니다.",
    suggestionsLabel: "대상 주소 제안",
  },
  configCollapsibleCard: {
    editConfig: "설정 수정",
  },
  streamProtocolMultiSelect: {
    ariaLabel: "전송 프로토콜",
  },
};
