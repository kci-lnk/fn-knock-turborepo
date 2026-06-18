export const enShared = {
  binaryDownload: {
    currentPlatform: "Current platform",
    supported: "Supported",
    unsupported: "Unsupported",
    resourceStatus: "Resource status",
    readyLabel: "Ready",
    pendingLabel: "Not ready",
    downloadProgress: "Download progress",
    errorPrefix: "Error: ",
    downloadButton: "Download resource",
    redownload: "Download again",
    redownloadConfirmTitle: "Download this resource again?",
    redownloadConfirmDescription: "This overwrites the existing file.",
    confirmRedownload: "Download again",
    delete: "Delete",
    deleteConfirmTitle: "Delete this resource?",
    deleteConfirmDescription: "You must download it again before use.",
    confirmDelete: "Delete",
    downloading: "Downloading, please wait...",
    cancelTask: "Cancel task",
  },
  dataShareFilePicker: {
    title: "Choose a file from FNOS",
    description: "Choose a readable file from the application root directory.",
    directoryLabel: "Application files",
    alertTitle: "Failed to read directory",
    unavailableDescription:
      "The directory is not accessible yet. Confirm the app is installed and the shared directory has been created.",
    confirmText: "Use this file",
    availableDescription: "{count} available files found",
    noMatchedFiles: "No matching files",
    noMatchedDescription: "Try another keyword or refresh the directory list.",
    emptyTitle: "No available files in the shared directory",
    emptyDescription:
      "No matching files are in this directory. Add supported file types, then choose again.",
    noExtension: "No extension",
  },
  certSourceField: {
    uploadFromPhone: "Upload from phone",
    uploadFromComputer: "Upload from computer",
    uploadFile: "Upload file",
    chooseSourceTitle: "Choose file source",
    chooseSourceDescription:
      "Choose an import method, then read the {label} file.",
    localFileDescription:
      "Choose a {types} file from this device and read it automatically",
    chooseFromFnos: "Choose from FNOS",
    sharedFileDescription:
      "Read existing files from the {shareName} root directory, up to three levels deep",
    pickerTitle: "Choose {label} from FNOS",
    pickerDescription:
      "Move the certificate file into Application data -> fn-knock first",
    readFile: "Read this file",
  },
  logViewer: {
    title: "Runtime logs",
    emptyText: "No logs yet",
    lineCount: "{count} lines",
  },
  detailDialog: {
    close: "Close",
    copyLog: "Copy logs",
    copySuccess: "Logs copied",
    copyUnverified: "Tried to copy logs",
    copyUnverifiedDescription: "Logs copied",
    copyFailed: "Failed to copy logs",
    manualCopyHint:
      "This page may be running in a restricted environment. Copy manually.",
  },
  inlineCommentEditor: {
    placeholder: "Enter a comment...",
    edit: "Edit comment",
    save: "Save comment",
    cancel: "Cancel editing",
    required: "Comment name cannot be empty",
    updateFailed: "Failed to update comment",
  },
  defaultRouteConfirm: {
    clearTitle: "Clear the default route?",
    setTitle: "Set the default route?",
    clearFnosDescription:
      "You are clearing the default route for the service on port {port}. This may affect the default FNOS entry.",
    clearDescription:
      "After clearing it, there will be no default route. Requests that do not match a path may not forward as expected.",
    setDescription:
      "The current default route points to the service on port {port}. Switching to another route may affect the default FNOS entry.",
  },
  certForm: {
    sslCert: "SSL certificate",
    privateKey: "Private key",
  },
  pagedTableFooter: {
    total: "{total} {itemText}",
    records: "records",
  },
  dnsCredentialBridge: {
    providers: {
      cloudflare: "Cloudflare",
      alidns: "Alibaba Cloud DNS",
      dnspod: "DNSPod",
      tencentcloud: "Tencent Cloud DNS",
      edgeone: "Tencent Cloud EdgeOne",
      edgeoneCname: "Tencent Cloud EdgeOne (CNAME access)",
      godaddy: "GoDaddy",
      porkbun: "Porkbun",
      dynv6: "dynv6",
      duckdns: "DuckDNS",
    },
  },
  proxyTargetInputField: {
    hint: "Choose the protocol on the left, then enter the IP and port on the right. If the port is omitted, it is filled from the protocol default on blur.",
  },
  configCollapsibleCard: {
    editConfig: "Edit config",
  },
  streamProtocolMultiSelect: {
    ariaLabel: "Transport protocol",
  },
};
