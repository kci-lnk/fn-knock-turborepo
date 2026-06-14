export const enServer = {
  success: "Success",
  notFound: "Not found",
  invalidLocale: "Unsupported locale",
  dockerAdminDenied:
    "Docker admin panel only allows private network or trusted proxy access",
  dockerAdminDeniedTitle: "Access denied",
  dockerAdminDeniedDescription:
    "The Docker admin panel only allows access from the host, LAN, VPN, or configured trusted reverse proxies by default. Direct public access is denied.",
  dockerAdminCurrentIp: "Detected source IP: {ip}",
  dockerAdminProxyRequired: "Access admin APIs through the {port} admin entry.",
  dockerAdminLoginRequired: "Sign in to the Docker admin panel first",
  captchaUnavailable: "Captcha service is temporarily unavailable",
  tooManyAttempts: "Too many attempts. Please try again later.",
  tooManyAttemptsWithRetry:
    "Too many attempts. Please try again in {seconds} seconds.",
  loginCredentialMissing: "No login credentials are configured on the server",
  invalidOtpWithRetry:
    "The verification code is incorrect. Please retry in {seconds} seconds.",
  runtimeProfile: {
    capabilities: {
      default: "The current runtime does not support this capability",
      direct_mode_available: {
        docker: "Docker deployments do not support host direct firewall mode",
        platform:
          "The current runtime does not support host direct firewall mode",
        permission:
          "The current process does not have host direct firewall capability",
      },
      host_firewall_available: {
        docker: "Docker deployments do not support host firewall management",
        platform:
          "The current runtime does not support host firewall management",
        permission:
          "The current process does not have host firewall management capability",
      },
      smart_connect_available: {
        docker:
          "Docker deployments do not support Smart Connect yet. It depends on host dnsmasq and port 53",
        platform: "The current runtime does not support Smart Connect yet",
        permission:
          "The current process does not have the host management capability required by Smart Connect",
      },
      system_clock_sync_available: {
        docker: "Docker deployments do not support host system time sync",
        platform: "The current runtime does not support system time sync",
        permission:
          "The current process does not have the host permission required for system time sync",
      },
      self_update_available: {
        docker:
          "Docker deployments do not support in-app FPK updates. Upgrade by pulling a new image",
        openwrt:
          "OpenWrt deployments do not support in-app FPK updates. Upgrade by installing a matching IPK with opkg",
        deployment:
          "The current deployment type does not support in-app updates",
      },
      terminal_available: {
        docker: "Docker deployments do not support Web terminal",
        openwrt: "OpenWrt deployments do not support Web terminal yet",
        platform: "The current runtime does not support Web terminal",
      },
      shared_root_available: {
        missing:
          "No shared directory mount is available in the current runtime",
      },
    },
  },
  systemClock: {
    unknown: "unknown",
    actionSeparator: "; ",
    listSeparator: ", ",
    duration: {
      seconds: "{seconds}s",
      minutes: "{minutes} min",
      minutesSeconds: "{minutes} min {seconds}s",
    },
    networkCheckFailed: "Failed to check system time online",
    issues: {
      timezone: {
        title: "System timezone is not Beijing time",
        message:
          "Current system timezone is {timezone}; it should be {expected}.",
      },
      timeMismatch: {
        title: "System time differs from the online check",
        message: "System time differs from the online check by about {drift}.",
      },
    },
    statusRefreshed: "System time status refreshed",
    syncFailed: "System time sync failed",
    networkTimeUnavailable: "Could not get standard time from the network",
    sourceFetchFailed: "Failed to get time from {source}",
    missingDateHeader: "{source} did not return a usable Date header",
    invalidDateHeader: "{source} returned an unparseable time",
    commandFailed: "Failed to run {command}",
    timezoneSet: "Set system timezone to {timezone}",
    missingZoneinfoFile: "System timezone file is missing: {path}",
    timezoneWritten: "Wrote system timezone {timezone}",
    clockAdjusted: "System time adjusted",
    ntpEnabled: "Enabled automatic NTP time sync",
    serviceRestarted: "Restarted {service}",
  },
  updateRoutes: {
    downloadStarted: "Update package download started",
    downloadStartFailed: "Failed to start download",
    installStarted: "Update installation started",
    installStartFailed: "Failed to start installation",
    checkAndDownloadStarted: "Update check started and download queued",
    startFailed: "Failed to start",
  },
  gatewayHostResponse: {
    runTypes: {
      direct: "direct mode",
      reverseProxy: "reverse proxy mode",
      subdomain: "subdomain mode",
    },
    unavailableReason:
      "Only subdomain mode is available. Current mode: {mode}.",
    editSubdomainOnly:
      "Host response can only be edited in subdomain mapping mode",
    syncFailed: "Failed to sync gateway Host response configuration",
    hostRoutesSyncFailed: "Failed to sync Host routes",
    updateFailed: "Failed to update gateway Host response",
    updateFailedRolledBack:
      "Failed to update gateway Host response; configuration was rolled back",
    updateFailedRollbackFailed: "{error}; rollback failed: {rollbackError}",
    restoreConfigFailed: "Failed to restore Host response configuration",
    restoreRuntimeFailed: "Failed to restore Host response runtime state",
    restoreGatewayRuntimeFailed:
      "Failed to restore gateway Host response runtime state",
  },
  admin: {
    runTypes: {
      direct: "direct mode",
      reverseProxy: "reverse proxy mode",
      subdomain: "subdomain mode",
    },
    validation: {
      required: "{label} is required",
      httpUrlRequired: "{label} must start with http:// or https://",
      invalidFormat: "{label} format is invalid",
    },
    rollback: {
      failed: "{message}; rollback failed: {rollbackError}",
      restoreConfigFailed: "Failed to restore the previous configuration",
      restoreSmartConnectFailed:
        "Failed to restore the previous Smart Connect runtime state",
      restoreRuntimeFailed: "Failed to restore the previous runtime state",
      restoreProtocolConfigFailed:
        "Failed to restore protocol mapping configuration",
      restoreProtocolFeatureFailed:
        "Failed to restore protocol mapping feature switch",
      restoreProtocolRuntimeFailed:
        "Failed to restore protocol mapping runtime state",
      restoreVisibilityConfigFailed:
        "Failed to restore visibility configuration",
      restoreVisibilityRuntimeFailed:
        "Failed to restore visibility runtime CIDRs",
      restoreGatewayVisibilityFailed:
        "Failed to restore gateway visibility runtime state",
      restoreProxyHeadersConfigFailed:
        "Failed to restore proxy header configuration",
      restoreProxyHeadersRuntimeFailed:
        "Failed to restore proxy header runtime state",
      restoreGatewayProxyHeadersRuntimeFailed:
        "Failed to restore gateway proxy header runtime state",
      restorePortalFailed: "Failed to restore portal display runtime state",
    },
    dockerPanel: {
      passwordNotNeeded:
        "The current run mode does not need a Docker admin panel password",
      setPasswordFailed: "Failed to set admin panel password",
      passwordChangeUnsupported:
        "The current run mode does not support changing the Docker admin panel password",
      changePasswordFailed: "Failed to change admin panel password",
      tooManyAttemptsWithRetry:
        "Too many attempts. Retry in {seconds} seconds.",
      tooManyAttempts: "Too many attempts. Try again later.",
      passwordSetupRequired:
        "The admin panel password has not been set. Complete the first-time setup first.",
      passwordIncorrectWithRetry:
        "Admin panel password is incorrect. Retry in {seconds} seconds.",
    },
    runType: {
      switchFailed: "Failed to switch run mode",
      switchFailedRolledBack:
        "Failed to switch run mode; configuration was rolled back",
    },
    firewall: {
      whitelistSynced: ", synced {count} whitelist IPs",
      exemptPorts: ", kept entry ports {ports}",
      resetSuccess:
        "Firewall reset for {runType}{whitelistMessage}{exemptPortsMessage}",
      resetFailed: "Failed to reset firewall",
      clearSuccess:
        "Firewall rules cleared and historical redirects for port {port} removed",
      clearFailed: "Failed to clear firewall",
    },
    protocolMapping: {
      subdomainOnly: "Protocol mapping can only be enabled in subdomain mode",
      updateFeatureFailed: "Failed to update protocol mapping feature switch",
      updateFeatureFailedRolledBack:
        "Failed to update protocol mapping feature switch; configuration was rolled back",
    },
    smartConnect: {
      subdomainOnly: "Smart Connect can only be enabled in subdomain mode",
      updateFailed: "Failed to update Smart Connect",
      updateFailedRolledBack:
        "Failed to update Smart Connect; configuration was rolled back",
    },
    fnosPortIcon: {
      syncFailed:
        "Failed to sync FNOS port icon hijack configuration to the gateway",
    },
    gateway: {
      syncAuthCacheFailed:
        "Failed to sync auth cache configuration to the gateway",
      syncThrottleFailed:
        "Failed to sync gateway throttle configuration to the gateway",
      updateFailed: "Failed to update gateway configuration",
      updateFailedRolledBack:
        "Failed to update gateway configuration; configuration was rolled back",
    },
    gatewayVisibility: {
      updateFailed: "Failed to update gateway visibility",
      updateFailedRolledBack:
        "Failed to update gateway visibility; configuration was rolled back",
    },
    gatewayProxyHeaders: {
      subdomainOnly:
        "Proxy headers can only be edited in subdomain mapping mode",
      updateFailed: "Failed to update gateway proxy headers",
      updateFailedRolledBack:
        "Failed to update gateway proxy headers; configuration was rolled back",
    },
    captcha: {
      turnstileKeysRequired:
        "When Cloudflare Turnstile is enabled, both site_key and secret_key are required",
    },
    ipLocation: {
      ipLookupUrlLabel: "IP lookup database URL",
      cidrUrlLabel: "CIDR database URL",
    },
    connectionTest: {
      httpStatus: "Service returned HTTP status {status}",
      invalidData: "Service returned invalid data",
      success: "Connection succeeded",
      timeout: "Connection timed out",
      failed: "Connection failed",
    },
    autoHttps: {
      dockerUnsupported: "Automatic HTTPS is not supported in the Docker build",
      openWrtUnsupported:
        "Automatic HTTPS is not supported in the OpenWrt build",
      startFailed: "Failed to start automatic HTTPS",
    },
    hostMappings: {
      singleAuthPortMapping:
        "Only one Host mapping can point to AUTH_PORT as the auth service",
      authMappingMustBePublic:
        "Auth service {host} must stay public and cannot enable self-auth or strict whitelist, otherwise the login entry becomes unreachable",
      authMappingBasicAuthForbidden:
        "Auth service {host} cannot enable credential injection",
      basicAuthInvalid:
        "Host mapping {host} credential injection requires username and password, and the username cannot contain a colon",
      locationPathRequired: "Host mapping {host} path rule requires a path",
      locationPathMustStartSlash:
        "Host mapping {host} path rule {path} must start with /",
      locationRootForbidden:
        "Host mapping {host} cannot use root path / as a path rule",
      locationReservedPath:
        "Host mapping {host} path rule {path} uses a reserved path",
      locationDuplicate: "Host mapping {host} has duplicate path rule {path}",
      locationTargetRequired:
        "Host mapping {host} path rule {path} requires a target",
      locationStatusInvalid:
        "Host mapping {host} path rule {path} response status must be between 100 and 599",
      locationHeaderInvalid:
        "Host mapping {host} path rule {path} contains invalid response header {header}",
      locationHeaderForbidden:
        "Host mapping {host} path rule {path} cannot customize response header {header}",
      syncHostRulesFailed: "Failed to sync Host routes",
      syncAuthConfigFailed: "Failed to sync auth gateway configuration",
      updateFailed: "Failed to update Host mappings",
      updateFailedRolledBack:
        "Failed to update Host mappings; configuration was rolled back",
      metadataFailed: "Failed to refresh target title",
      bookmarkFolderForRoot: "{root} subdomain mappings",
      bookmarkFolderDefault: "fn-knock subdomain mappings",
    },
    streamMappings: {
      listenPortNotInteger: "Listen port {port} is not a valid integer",
      listenPortOutOfRange: "Listen port {port} is out of range",
      duplicatePort:
        "{protocol} listen port {port} is duplicated. Keep protocol + port unique.",
      targetMustBeHostPort: "Target address {target} must be in host:port form",
      syncFailed:
        "Failed to sync protocol mappings and gateway port allow rules",
      syncFailedRolledBack:
        "Failed to sync protocol mappings and gateway port allow rules; configuration was rolled back",
    },
    passkeyRp: {
      parentDomainRequired:
        "When parent-domain Passkey RP is enabled, enter the root domain or explicitly specify a parent RP ID.",
      mustMatchAuthHost:
        "Parent-domain Passkey RP ID {rpId} must match auth service {authHost} or be its parent domain.",
    },
    subdomainMode: {
      sslAutoSelected:
        "Automatically switched to a certificate better suited to the current subdomain mode.",
      sslAutoSelectionSyncFailed:
        "A recommended certificate was found, but syncing it to the gateway failed, so it was not switched automatically.",
    },
    totp: {
      invalidCode: "Verification code is incorrect. Try again.",
      notFound: "TOTP not found",
    },
    passkeys: {
      notFound: "Passkey not found",
    },
    syncRoutes: {
      partialFailedGatewayLogging:
        "Partial sync failed: gateway_logging={gatewayLogging}",
      partialFailedGatewayLoggingWaf:
        "Partial sync failed: gateway_logging={gatewayLogging}, waf={waf}",
      success:
        "Synced {rules} path routes, {hostRules} Host routes, {streamRules} protocol mappings, request log configuration, and WAF configuration for the current run mode",
    },
    backup: {
      readFnosDirectoryFailed: "Failed to read FNOS backup directory",
      exportFnosSuccess: "Backup exported to the FNOS directory",
      exportFnosFailed: "Failed to export to the FNOS directory",
      importSuccessWithWarnings:
        "Backup imported, but some runtime sync steps failed",
      importSuccess: "Backup imported and runtime sync completed",
      importFailed: "Failed to import backup",
      importFnosSuccessWithWarnings:
        "FNOS backup imported, but some runtime sync steps failed",
      importFnosSuccess: "FNOS backup imported and runtime sync completed",
      importFnosFailed: "Failed to import backup from FNOS",
    },
    sessions: {
      notFound: "Session not found",
    },
  },
  gatewayLogs: {
    configSyncFailed:
      "Request log settings were saved, but syncing them to the gateway failed",
    readDirectoryFailed: "Failed to read the log directory",
    readDatesFailed: "Failed to read log dates",
    readEntriesFailed: "Failed to read request logs",
    deleteEntriesFailed: "Failed to delete request logs",
  },
  backoffRoutes: {
    ipRequired: "ip parameter is missing",
  },
  ipLocationRoutes: {
    batchLimit: "Query at most {max} IPs at a time",
  },
  gatewayPortal: {
    syncConfigFailed: "Failed to sync portal display configuration to gateway",
    syncHostRulesFailed: "Failed to sync Host routes",
  },
  gatewayVisibility: {
    customCidrInvalid: "Custom CIDR format is invalid: {cidrs}",
    emptyEnabledConfig:
      "After enabling visibility, add at least one region or one custom CIDR",
    syncFailed: "Failed to sync gateway visibility configuration",
  },
  gatewayLogging: {
    syncConfigFailed: "Failed to sync gateway request log configuration",
  },
  sslGateway: {
    clearFailed: "Failed to clear gateway certificate",
    syncFailed: "Failed to sync gateway certificate",
  },
  sslRoutes: {
    gatewayStatusReadFailed: "Unable to read gateway SSL status",
    readSharedFileFailed: "Failed to read shared directory file",
    emptyDomains: "Domain list is empty. Add a domain or IP first.",
    certOrKeyInvalid: "Certificate or private key is invalid",
    hostRequired: "host is required",
    localCaCertificateLabel: "Local CA certificate",
    success: "Succeeded",
    certNotInstalled: "Certificate is not installed",
    manualCertificateLabel: "Manually uploaded certificate",
    certNotFound: "Certificate not found",
  },
  redis: {
    defaultCredential: "Default credential",
    certificateLabels: {
      acme: "ACME certificate",
      ca: "Self-signed certificate",
      manual: "Manually uploaded certificate",
      current: "Current certificate",
    },
    ssl: {
      certFormatInvalid: "Certificate format is invalid: {message}",
      keyFormatInvalid: "Private key format is invalid: {message}",
      certKeyMismatch: "Certificate and private key do not match",
      certKeyCheckFailed: "Certificate and private key check failed: {message}",
      certContentRequired: "Certificate content is required",
      certNotFound: "Certificate not found",
      certOrKeyInvalid: "Certificate or private key is invalid",
    },
    acme: {
      domainRequired: "Domain is required",
      domainsRequired: "Domain list is required",
      dnsProviderRequired: "DNS provider is required",
      primaryDomainDuplicated:
        "Primary domain {primaryDomain} already exists in another request item",
      applicationNotFound: "Request item not found",
      noMatchingIssuedCertificate:
        "This request item has no issued certificate matching the domain configuration",
      jobDataInvalid: "ACME task data is invalid",
      multipleApplicationsUseNewApi:
        "Multiple request items already exist. Use the new API to manage ACME request items.",
    },
  },
  acmeService: {
    waiting: "Waiting for action",
    sendSignalFailed: "Failed to send {signal} to {target}: {detail}",
    setDefaultCaFailed:
      "Failed to set default certificate authority (exit code: {code}){brief}",
    registerAccountFailed:
      "Failed to register ACME account (exit code: {code}){brief}",
    bundledZipMissing: "Bundled acmesh.zip resource was not found",
    extractingBundled: "Extracting bundled acme.sh resources...",
    unzipFailed: "Extraction failed, exit code: {code}",
    extractedAcmeMissing: "Extraction succeeded but acme.sh was not found",
    writingDataDir: "Writing data directory...",
    writtenAcmeMissing: "acme.sh was not found after writing",
    checkInstallFailed: "Failed to check installation status: {detail}",
    ready: "acme.sh is ready",
    notInstalled: "acme.sh is not installed",
    initializingBundled: "Initializing bundled acme.sh...",
    registeringAccount: "Registering ACME account...",
    savingDefaultCa: "Saving default certificate authority...",
    installSuccess: "Installation succeeded, account email: {email}",
    installFailed: "Installation failed: {detail}",
    installFirst: "Install acme.sh first",
    installingCannotDelete: "acme.sh is installing and cannot be deleted",
    deleted: "acme.sh was deleted",
    deleteFailed: "Delete failed: {detail}",
    domainsRequired: "Domain list is required",
    dnsTypeRequired: "DNS verification type is missing",
    issueFailed: "Certificate issuance failed (exit code: {code}){brief}",
  },
  acmeJobRunner: {
    manualStop: "The ACME task was stopped manually by the user",
    lockMessages: {
      manualRequest: "Requesting certificate",
      autoRenew: "Automatically renewing certificate",
    },
    activeTaskRunning: "An ACME task is already running. Try again later.",
    flowFailed: "Certificate request flow failed: {message}",
    stopSignalSent: "Stop signal sent, terminated {count} acme.sh processes",
    noRunningProcess: "No running acme.sh process was found",
    stopProcessError: "Exception while stopping process: {message}",
    processStillRunning: "acme.sh processes are still running: {pids}",
    lockLost:
      "The ACME runtime lock was lost. The task has stopped. Start the request again.",
    lockRefreshFailed: "ACME runtime lock refresh failed: {message}",
    lockLeaseExpired:
      "{message}; lock lease expired. The task has stopped. Start the request again.",
    applicationChangedSkipped:
      "Request item domains changed during execution. Writing the old certificate was skipped. Start the request again.",
    issuedButApplicationChanged:
      "Certificate was issued, but the request item domains changed, so it was not written to the current request item.",
    issuedButCertReadFailed:
      "Certificate was issued, but reading the certificate file failed. Try again later or check the acme.sh directory.",
    clearedDomainWorkingState:
      "Cleared the acme.sh domain working directory. Certificate listing and renewal are now managed by system tasks.",
    clearDomainWorkingStateFailed:
      "Certificate was saved, but clearing acme.sh domain state failed: {message}",
    linkedLibrarySyncedGateway:
      "Synced the linked certificate library entry and refreshed the gateway certificate list",
    linkedLibraryUpdated: "Updated the linked certificate library entry",
    addedToLibraryAndSyncedGateway:
      "Certificate was automatically added to the certificate library after issuance, and the gateway certificate list was refreshed",
    addedToLibrary:
      "Certificate was automatically added to the certificate library after issuance",
    addToLibraryFailed:
      "Certificate was issued and saved, but adding it to the certificate library failed: {message}",
    stoppedIgnoredProcessError:
      "The task has stopped. The process exit error was ignored.",
  },
  acmeRoutes: {
    domainsInvalid: "Domain list is empty or invalid",
    dnsTypeRequired: "DNS verification type is missing",
    unsupportedDnsProvider: "Unsupported DNS provider",
    missingDnsCredentials:
      "DNS API credentials are missing. Fill in one of these options: {requirements}",
    cloudflareInvalidKey:
      "Cloudflare API key is incorrect (invalid X-Auth-Key format)",
    cloudflareInvalidEmail:
      "Cloudflare email is incorrect (invalid X-Auth-Email format)",
    cloudflareInvalidHeaders:
      "Cloudflare API request headers are invalid, usually because the API key or email is incorrect",
    acmeFrequencyLimited:
      "Request frequency is limited (Retry-After={seconds} seconds; retries stop above 600 seconds). Wait and try again.",
    dnsApiRateLimited:
      "DNS API rate limit was triggered (429/Rate limit). Try again later.",
    logUnknownFailure:
      "An error was detected in the logs, but it could not be categorized automatically.",
    installingRetryLater: "acme.sh is installing. Try again later.",
    installFirst: "Install acme.sh first",
    multipleApplicationsUseNewApi:
      "Multiple request items already exist. Use the new API to manage ACME request items.",
    applicationNotFound: "Request item not found",
    notFound: "Not found",
    installingCannotDelete: "acme.sh is installing and cannot be deleted",
    installingCannotSwitchCa:
      "acme.sh is installing. Certificate authority cannot be switched yet.",
    noMatchingIssuedCertificate:
      "This request item has no issued certificate matching the domain configuration",
    success: "Succeeded",
    dns01Only: "Only DNS-01 verification is supported",
    certNotFound: "Certificate not found",
    certOrKeyInvalid: "Certificate or private key is invalid",
  },
  acmeDnsProviders: {
    groups: {
      common: "Common",
      domestic: "China",
      international: "International",
      selfHostedAdvanced: "Self-hosted / Advanced",
    },
    credentialSchemes: {
      default: "Default credentials",
    },
    fields: {
      accountEmail: "Account email",
      sshPrivateKeyPath: "SSH private key file path",
    },
    labels: {
      aliyun: "Alibaba Cloud DNS",
      tencentCloudDnspod: "Tencent Cloud DNSPod (TencentCloud)",
      huaweiCloudDns: "Huawei Cloud DNS",
      jdCloudDns: "JD Cloud DNS",
      westCn: "West.cn",
    },
    cloudflare: {
      globalKeyDescription:
        "Compatible with Cloudflare's legacy Global API Key method.",
      apiTokenDescription:
        "Recommended. Only Token is required. If Zone ID or Account ID is known, fill them in as well to reduce auto-detection.",
    },
    gcloud: {
      description:
        "Depends on the gcloud command and authorized configuration in the runtime environment. If left blank, the default gcloud configuration is used.",
    },
    azure: {
      managedIdentityDescription: "Set AZUREDNS_MANAGEDIDENTITY to true.",
    },
    descriptions: {
      boolean01: "Enter 0 or 1.",
      optionalBoolean01: "Optional. Enter 0 or 1.",
    },
    requirements: {
      optionalSuffix: "; optional {keys}",
      orSeparator: "; or ",
    },
  },
  acmePatches: {
    duckdns: {
      scriptMissing: "DuckDNS DNS API script was not found: {path}",
      proxyApplied: "Switched DuckDNS API from {from} to {to}",
    },
  },
  reverseProxyTrustedIps: {
    syncFailed: "Failed to sync reverse proxy throttle exempt IPs",
  },
  commonAuthLocations: {
    cidrLookupFailed: "CIDR lookup failed",
    syncFailed:
      "Failed to sync common-location exemption configuration to gateway",
  },
  fnosDataShare: {
    invalidPath: "Invalid shared file path",
    shareMissing:
      "FNOS shared directory was not found. Confirm that app resources are configured correctly.",
    fileOnly: "Only files in the shared directory can be read",
    fileTooLarge:
      "File is too large. Only place certificate or private key text files here.",
  },
  autoHttps: {
    listenEacces:
      "No permission to listen on port 80. Confirm that this device or container allows the process to bind low ports.",
    listenEaddrinuse:
      "Port 80 is already used by another program, so automatic HTTPS cannot start. Try FNOS system settings, Security, Port settings, Edit, and uncheck redirects for ports 80 and 443.",
    listenFailedWithMessage: "Failed to listen on port 80: {message}",
    listenFailed: "Failed to listen on port 80.",
  },
  wafCollector: {
    drainFailed: "Failed to pull WAF events",
  },
  hostMappingBookmarks: {
    defaultFolderTitle: "fn-knock subdomain mappings",
  },
  whitelist: {
    addFailed: "Failed to add whitelist record",
    recordNotFound: "Whitelist record not found",
    domainResolveFailed: "Domain resolution failed",
    refreshFailed: "Failed to refresh whitelist record",
  },
  whitelistManager: {
    dnsRecordQueryFailedWithCode:
      "{label} record query failed ({code}): {message}",
    dnsRecordQueryFailed: "{label} record query failed: {message}",
    targetFormatInvalid: "IP, CIDR, or domain format is invalid",
    autoGrantIpOnly: "Automatic login authorization only supports a single IP",
    cidrInvalid: "CIDR format is invalid",
    domainInvalid: "Domain format is invalid",
    ipInvalid: "IP format is invalid",
    autoOwnerMissing: "Automatic whitelist owner identifier is missing",
    domainResolveFailed: "Domain resolution failed",
    resolvedIpCount: "Resolved {count} IPs",
    noAaaaRecords: "No A / AAAA records were resolved",
    syncAllowedStateFailed:
      "Domain resolution results were updated, but syncing system allow state failed",
  },
  terminal: {
    defaultTitle: "Web terminal",
    defaultSessionTitlePrefix: "Session-",
    tmuxNotDetectedInstallFirst:
      "tmux was not detected. Install the tmux environment first.",
    tmuxReadyWithVersion: "tmux is ready: {version}",
    refreshingApt: "Refreshing Debian package sources...",
    aptUpdateFailed: "apt-get update failed",
    installingTmux: "Installing tmux...",
    aptInstallTmuxFailed: "apt-get install tmux failed",
    verifyingTmuxInstall: "Verifying tmux installation...",
    tmuxMissingAfterInstall:
      "tmux was still not detected after installation completed",
    tmuxInstallCompleteWithVersion: "tmux installation completed: {version}",
    tmuxInstallFailed: "tmux installation failed",
    cwdUnavailable:
      "Working directory does not exist or is inaccessible: {path}",
    webTerminalDisabled: "Web terminal is not enabled",
    tmuxInstallingWait:
      "tmux is being installed. Wait for installation to finish.",
    tmuxStatusError: "tmux status error: {message}",
    tmuxMissingCannotCreate:
      "tmux was not detected, so a resumable terminal session cannot be created",
    rootRunRequiresDangerToggle:
      "The current process is running as root. Enable the explicit high-risk run switch in settings before creating a terminal.",
    requestedShellUnavailable: "Requested shell is unavailable: {shell}",
    noShellDetected:
      "No available shell was detected. Confirm that zsh, bash, or sh is installed.",
    paneMetadataReadFailed: "Unable to read terminal pane metadata",
    paneTtyParseFailed: "Unable to parse terminal pane tty",
    inputPipeCreateFailed: "Unable to create terminal input pipe",
    ioRelayCreateFailed: "Unable to create terminal IO relay",
    sessionLimitReached: "Terminal session limit reached ({count})",
    tmuxSessionCreateFailed: "Failed to create tmux session",
    sessionTitleRequired: "Session name is required",
    sessionMissingOrExpired: "Terminal session does not exist or has expired",
    tmuxMissingCannotAttach:
      "tmux was not detected, so the terminal session cannot be attached",
    inputPipeNotReady: "Terminal input pipe is not ready yet",
    inputWriteInterrupted: "Terminal input write was interrupted",
    attachmentExpired: "Terminal attachment has expired",
    inputSendFailed: "Failed to send terminal input",
    resizeFailed: "Failed to resize terminal",
    sessionNotFound: "Terminal session not found",
  },
  waf: {
    manifestInvalid: "System rule manifest format is invalid",
    manifestMissingZipInfo:
      "System rule manifest is missing zip file information",
    manifestRequestFailed: "System rule manifest request failed: HTTP {status}",
    manifestRefreshFailed: "Failed to refresh system rule manifest",
    confOnly: "Only .conf rule files are supported",
    ruleFilenameInvalid: "Rule filename is invalid",
    fileTooLarge: "{filename} exceeds 1MB",
    fileInvalidUtf8: "{filename} is not valid UTF-8 text",
    filesystemDirectiveBlocked:
      "{filename} contains disallowed filesystem directives",
    systemRuleDescription: "System security rule",
    customRuleDescription: "User uploaded rule",
    enableNeedsRule: "Enable at least one WAF rule file before turning WAF on",
    rulesLoadFailed: "Failed to load WAF rules",
    configSyncFailed: "Failed to sync WAF configuration to the gateway",
    sourceInvalid: "Rule source is invalid",
    ruleFileNotFound: "Rule file not found",
    zipInvalid: "System rule zip format is invalid",
    zipDirectoryInvalid: "System rule zip directory is invalid",
    zipUnpackedTooLarge: "System rule package is too large after extraction",
    zipHeaderInvalid: "System rule zip file header is invalid",
    zipMethodUnsupported: "Unsupported zip compression method {method}",
    zipSizeInvalid: "System rule zip file size is invalid",
    zipPathInvalid: "System rule zip file path is invalid: {path}",
    downloadFailed: "System rule download failed: HTTP {status}",
    zipTooLarge: "System rule package is too large",
    zipHashMismatch: "System rule package hash verification failed",
    zipEmpty: "System rule package is empty",
    zipDuplicateFile: "Duplicate file exists in system rule package: {path}",
    zipConfRootOnly:
      ".conf files in the system rule package must be in the root directory",
    zipNoConf: "System rule package does not contain any .conf files",
    systemRulePathInvalid: "System rule file path is invalid",
    manifestEmpty: "System rule manifest is empty",
    keepOneEnabledRule:
      "Keep at least one rule file enabled while WAF is turned on",
    uploadSelectConf: "Select .conf files to upload",
    reloadRulesFailed: "Failed to reload WAF rules",
    statusReadFailed: "Failed to read WAF status",
    configSaveOrLoadFailed: "Failed to save or load WAF settings",
    systemRulesSyncFailed: "Failed to sync system rules",
    ruleToggleFailed: "Failed to enable or disable WAF rules",
    ruleReadFailed: "Failed to read WAF rule",
    customRuleUploadFailed: "Failed to upload custom rule",
    customRuleDeleteFailed: "Failed to delete custom rule",
    eventsDrainFailed: "Failed to pull WAF events",
    logsQueryFailed: "Failed to query WAF logs",
    logNotFound: "WAF log not found",
    logsDeleteFailed: "Failed to delete WAF logs",
  },
  oidc: {
    callbackStateExpired: "Login state has expired. Start sign-in again.",
    loginFailedRetry: "External sign-in failed. Start sign-in again.",
    reservedExtraAuthParam:
      "extra_auth_params contains a reserved OIDC parameter: {key}",
    urlInvalid: "{label} must be a valid URL",
    urlMustUseHttps: "{label} must use HTTPS",
    providerUnsupported: "Unsupported external sign-in provider",
    providerMissingRequiredConfig:
      "{provider} is missing required configuration: {fields}",
    providerMissingRequiredFields:
      "External sign-in provider is missing required configuration: {fields}",
    accessTokenMissing: "access_token was not returned",
    idTokenMissing: "id_token was not returned",
    callbackUrlBuildFailed:
      "Unable to build the external sign-in callback URL. Configure public_auth_base_url.",
    issuerMissing: "OIDC issuer is not configured",
    discoveryMissingFields:
      "OIDC discovery document is missing required fields",
    nonceCheckFailed: "OIDC nonce verification failed",
    issuerCheckFailed: "OIDC issuer verification failed",
    subjectEmpty: "OIDC subject is empty",
    githubUserIdEmpty: "GitHub user ID is empty",
    providerNotFound: "External sign-in provider not found",
    connectionTestSuccess: "Connection test succeeded",
    oauthEndpointIncomplete: "OAuth2 endpoint configuration is incomplete",
    connectionTestFailed: "Connection test failed",
    totpMissing: "TOTP credential not found",
    selectProvider: "Select an external sign-in provider",
    providerUnavailable: "External sign-in provider is unavailable",
    bindingNotFound: "External account binding not found",
    inviteInvalid: "Binding invite link is invalid",
    inviteExpired: "Binding invite link has expired",
    inviteProviderNotAllowed: "This invite link does not allow this provider",
    authorizationEndpointMissing: "authorization endpoint is not configured",
    bindStateInvalid: "Binding invite state is invalid",
    accountNotBoundCannotLogin:
      "This external account is not bound and cannot sign in",
    tokenEndpointMissing: "token endpoint is not configured",
    bindProviderMismatch: "Binding invite does not match the sign-in provider",
    inviteTotpMissing:
      "The TOTP linked to this binding invite no longer exists",
    accountAlreadyBoundOtherTotp:
      "This external account is already bound to another TOTP",
    inviteUsed: "Binding invite link has already been used",
    providerErrors: {
      accessDenied:
        "You canceled external sign-in authorization, or the provider rejected the request.",
      temporarilyUnavailable:
        "External sign-in service is temporarily unavailable. Try again later.",
      serverError:
        "The external sign-in provider returned a service error. Try again later.",
      invalidScope:
        "External sign-in scopes are configured incorrectly. Ask an administrator to check the provider settings.",
      rejected:
        "The external sign-in request was rejected by the provider. Check the external sign-in configuration and retry.",
      incomplete: "External sign-in was not completed. Start sign-in again.",
    },
    bindWithProvider: "Bind with {provider}",
    selectProviderTitle: "Choose an external account provider",
    bindToTotp: "Bind the external account to {totp}.",
    linkMissingToken: "The link is missing token.",
    inviteMissingExpiredUsed:
      "This invite does not exist, has expired, or has already been used.",
    noProvidersTitle: "No external sign-in providers available",
    noProvidersBody:
      "This invite currently has no external account provider available for binding.",
    bindFailedTitle: "External account binding failed",
    bindStartFailed: "Unable to start external account binding.",
    startFailed: "Failed to start external sign-in",
    callbackMissingParams:
      "External sign-in callback is missing required parameters. Start sign-in again.",
    loginFailed: "External sign-in failed",
    operationAborted:
      "External sign-in request was interrupted. Start sign-in again.",
    loginFailedRetryAfter: "{message}. Retry in {seconds} seconds.",
    createProviderFailed: "Failed to create external sign-in provider",
    updateProviderFailed: "Failed to update external sign-in provider",
    deleteProviderFailed: "Failed to delete external sign-in provider",
    testProviderFailed: "Failed to test external sign-in provider",
    deleteBindingFailed: "Failed to delete external account binding",
    createInviteFailed: "Failed to create binding invite",
    catalog: {
      googleDescription: "Sign in with a Google account.",
      microsoftDescription: "Sign in with a Microsoft / Azure AD account.",
      githubDescription: "Sign in with GitHub OAuth.",
      customLabel: "Custom OIDC",
      customDescription:
        "Use a custom provider with standard OpenID Connect Discovery.",
    },
  },
  subdomainMode: {
    recommendationMissingBase:
      "Root domain or auth service is not configured, so recommended certificate domains cannot be generated yet.",
    recommendationWildcardSummary:
      "Recommended domains: {rootDomain} and *.{rootDomain}, covering the root domain, auth service, and business subdomains under the same parent domain.",
    authOutOfRootWarning:
      "The current auth service {authHost} is not under root domain {rootDomain}; the exact domain was added separately. Confirm that the selected DNS provider can manage these domains.",
    recommendationSingleHostSummary:
      "Root domain is not configured, so only a single-domain certificate for auth service {authHost} can be recommended.",
    wildcardSuggestion:
      "To cover multiple business subdomains later, add the root domain before requesting a wildcard certificate.",
    configureRootOrAuth:
      "Configure a root domain in subdomain mode, or specify an auth service in Host mappings first.",
    authMissingWarning:
      "Auth service is not specified, so the recommendation is derived only from the root domain.",
    uncoveredHostMappingsWarning:
      "{count} Host mappings are outside the recommended certificate coverage. If they need public exposure, add certificates or adjust domain planning.",
    coverageNoSsl:
      "SSL certificate is not enabled, so the auth service and business subdomains are not covered by HTTPS yet.",
    coverageReadyConcrete:
      "The deployed certificate covers the auth service and all configured Host mappings.",
    coverageReadyRecommended:
      "The deployed certificate satisfies the current recommended coverage for subdomain mode.",
    coveragePartialConcrete:
      "The current certificate covers only some domains required by subdomain mode. The auth service or some business Hosts may still have certificate mismatches.",
    coveragePartialRecommended:
      "The current certificate covers only some recommended domains. Certificate mismatches may still occur when subdomain mode is enabled later.",
    coverageMismatchConcrete:
      "The deployed certificate does not match subdomain mode. The auth service and business Hosts are still not correctly covered.",
    coverageMismatchRecommended:
      "The deployed certificate does not yet cover the domain range recommended for subdomain mode.",
    coverageMissingRequiredWarning:
      "The current certificate is missing {count} required coverage items. Reissue or replace the certificate.",
    coverageMissingRecommendedWarning:
      "The current certificate is missing {count} recommended domain coverage items. Reissue or replace it if these domains will be used later.",
    coverageAuthHostMissingWarning:
      "The current certificate does not cover auth service {authHost}.",
    inventoryEmpty:
      "The certificate inventory does not contain certificates usable for subdomain mode yet.",
    inventoryActiveReady:
      "The active certificate fully covers the domains required by subdomain mode.",
    inventoryOneReady:
      "One certificate in the inventory fully covers subdomain mode and can be switched to active directly.",
    inventoryMultipleReady:
      "{count} certificates in the inventory each fully cover the current subdomain mode.",
    inventoryCombinedReady:
      "The certificate inventory can provide full coverage when combined.",
    inventoryCandidateReady:
      "The certificate inventory already has a candidate that covers the current subdomain mode.",
    inventoryCombinedNeedsMultiSni:
      "The certificate inventory can cover the current subdomain mode in combination, but the gateway is still in single-active-certificate mode, so they cannot all take effect yet.",
    inventoryPartialCandidates:
      "The certificate inventory has partial candidates, but they still do not fully cover the auth service and all Host mappings.",
    inventoryNoCertificateCoversRecommendation:
      "No certificate currently covers the recommended domains for subdomain mode.",
    inventoryMultiCertRequiresSniWarning:
      "The certificate inventory needs multiple certificates for combined coverage, but the gateway is still in single-active-certificate mode, so they cannot all take effect at once.",
    inventorySwitchRecommendedWarning:
      "The active certificate does not fully match subdomain mode. Switch to the recommended certificate.",
    inventoryBetterForSniWarning:
      "The existing certificate inventory is better suited for future multi-certificate/SNI deployment.",
  },
  cloudflared: {
    missingToken: "Configure the Cloudflare Token first",
    startFailedWithDetail: "Failed to start cloudflared: {detail}",
    processExited: "cloudflared process exited",
    processExitedWithCode: "cloudflared process exited with code {code}",
    processCrashed: "cloudflared process crashed: {message}",
    resumeOnBoot:
      "resume: Cloudflared was running last time and is being restored automatically...",
    unknownError: "Unknown error",
    notInitialized: "Cloudflared is not initialized",
    startFailed: "Failed to start",
  },
  dnsmasq: {
    notDetectedInstallFirst: "dnsmasq was not detected. Install it first.",
    dnsPortUnavailable: "DNS port 53 is unavailable. Free the port and retry.",
    dnsPortUnavailableWithDetail:
      "DNS port 53 is unavailable. Free the port and retry: {detail}",
    detectedWithVersion:
      "dnsmasq detected: {version}. Waiting for initialization or service start.",
    detected: "dnsmasq detected. Waiting for initialization or service start.",
    missingServiceAutoComplete:
      "System service is missing and will be completed during initialization.",
    servicePackageMissing:
      "A dnsmasq executable was detected, but the system service is not installed. Install the dnsmasq package first.",
    completingService: "Completing dnsmasq system service...",
    completeServiceFailed: "Failed to complete dnsmasq system service",
    serviceDefinitionMissingAfterInstall:
      "dnsmasq service installation finished, but no usable system service definition was detected.",
    executableMissing: "dnsmasq executable was not detected",
    configTestFailed: "dnsmasq configuration validation failed",
    restartFailed: "Failed to restart dnsmasq",
    serviceDefinitionMissing:
      "dnsmasq system service definition was not detected. Finish initialization to complete the service environment.",
    readyWithVersion: "dnsmasq is ready: {version}",
    ready: "dnsmasq is ready",
    refreshingApt: "Refreshing Debian package sources...",
    aptUpdateFailed: "apt-get update failed",
    installing: "Installing dnsmasq...",
    aptInstallFailed: "apt-get install dnsmasq failed",
    enablingService: "Enabling dnsmasq service...",
    verifyingService: "Verifying dnsmasq service...",
    installMissingAfterComplete:
      "dnsmasq was still not detected after installation completed",
    installFailed: "dnsmasq installation failed",
    checkingEnvironment: "Checking dnsmasq environment...",
    validatingConfig: "Validating dnsmasq configuration...",
    startingService: "Starting dnsmasq service...",
    initializeFailed: "dnsmasq initialization failed",
  },
  firewall: {
    goBackendCallFailed: "Go backend API call failed: {message}",
    clearLegacyTcpRedirectFailed:
      "Failed to clear legacy TCP redirect {listenPort} -> {targetPort}",
    initDefaultRulesFailed: "Failed to initialize default firewall rules",
    syncWhitelistTargetFailed: "Failed to sync whitelist target {target}",
    cleanRulesFailed: "Failed to clear firewall rules",
    syncAuthGatewayConfigFailed: "Failed to sync auth gateway configuration",
    syncReverseProxyThrottleFailed:
      "Failed to sync reverse proxy throttling configuration",
    syncGatewayVisibilityConfigFailed:
      "Failed to sync gateway visibility configuration",
    syncGatewayProxyHeadersConfigFailed:
      "Failed to sync gateway proxy header configuration",
    syncGatewayHostResponseConfigFailed:
      "Failed to sync gateway Host response configuration",
    enableProxyProtocolForceFailed:
      "Failed to enable forced Proxy Protocol mode",
    disableProxyProtocolForceFailed:
      "Failed to disable forced Proxy Protocol mode",
    disableStreamRulesFailed: "Failed to disable protocol mapping listeners",
    flushPathRoutesFailed: "Failed to clear path routes",
    syncHostRoutesFailed: "Failed to sync Host routes",
    syncDefaultRouteFailed: "Failed to sync default route",
    flushHostRoutesFailed: "Failed to clear Host routes",
    syncPathRoutesFailed: "Failed to sync path routes",
    syncStreamRulesFailed: "Failed to sync protocol mappings",
    syncAuthEntryRouteFailed: "Failed to sync auth entry route",
    syncAuthDefaultRouteFailed: "Failed to sync auth default route",
  },
  updateManager: {
    manifestFieldInvalid: "Update manifest field {field} is invalid",
    manifestFormatInvalid: "Update manifest format is invalid",
    manifestMissingVersion: "Update manifest is missing version",
    manifestMissingUpdateAvailable:
      "Update manifest is missing update_available",
    manifestMissingForceUpdate: "Update manifest is missing force_update",
    manifestMissingDownloadUrl: "Update manifest is missing download_url",
    manifestArm64FieldsIncomplete:
      "Update manifest ARM64 download fields are incomplete",
    architectureUnsupported:
      "Automatic updates are not supported on this architecture: {arch}",
    manifestMissingArm64DownloadUrl:
      "Update manifest is missing the ARM64 download URL",
    manifestMissingArm64Checksum:
      "Update manifest is missing the ARM64 checksum",
    checkHttpFailed: "Update check failed: HTTP {status}",
    checkFailed: "Update check failed",
    noUpdateInfo: "Update information has not been fetched yet",
    featureDisabled: "Updates are currently disabled",
    alreadyLatest: "Already on the latest version",
    downloadHttpFailed: "Download failed: HTTP {status}",
    responseBodyUnreadable: "Download failed: response body is unreadable",
    checksumFailed: "Checksum failed: expected {expected}, got {actual}",
    downloadFailed: "Download failed",
    noInstallableUpdate: "No installable update is available",
    downloadPackageFirst:
      "Download and verify the update package before installing",
    packageMissing: "Update package is missing. Download it again.",
    packageChecksumFailed: "Update package checksum failed. Download it again.",
    installStartFailed: "Failed to start update installation",
  },
  tunnelManagers: {
    cloudflared: {
      macAutoDownloadUnsupported:
        "Automatic app download is not supported on macOS. Install it manually with brew install cloudflared.",
      platformUnsupported: "This platform is not supported",
      responseBodyUnreadable: "Download response body is unreadable",
      downloadCancelled: "Download cancelled",
      unknownError: "Unknown error",
      macManualRemove: "Remove cloudflared manually on macOS",
      notInstalledBrew:
        "Cloudflared is not installed. Install it with brew install cloudflared first.",
      notInitialized: "Cloudflared is not initialized. Download it first.",
    },
    frp: {
      platformUnsupported: "This platform is not supported",
      packageMissing: "FRP package is missing",
      extractFailed: "Extraction failed with exit code {code}",
      responseBodyUnreadable: "Download response body is unreadable",
      connectionFailed: "Connection failed",
      downloadFailed: "Download failed: {detail}",
      unknownError: "Unknown error",
      downloadCancelled: "Download cancelled",
      notInitialized: "FRP is not initialized. Download it first.",
    },
  },
  frpc: {
    instanceNotFound: "FRP instance does not exist: {id}",
    instanceLimitExceeded: "At most {limit} extra FRP instances are supported",
    primaryName: "Primary FRP",
    instanceName: "FRP instance",
    verifyFailedWithDetail: "frpc verify failed: {detail}",
    verifyFailedWithCode: "frpc verify failed with exit code {code}",
    verifyFrpNotInitialized:
      "FRP is not initialized, so frpc.toml cannot be verified. Download FRP resources in System Settings first.",
    pidInvalidForInstance:
      "PID is no longer valid or does not belong to this instance",
    processExited: "frpc process exited",
    processExitedWithCode: "frpc process exited with code {code}",
    processCrashed: "frpc process crashed: {message}",
    processStillRunning: "FRP process is still running pid={pid}",
    primaryDeleteDenied: "The primary FRP instance cannot be deleted",
    notInitialized: "FRP is not initialized",
    startFailedWithDetail: "Failed to start frpc: {detail}",
    pidCleanedForInstance:
      "PID does not belong to this instance; this instance runtime record was cleared",
    resumeOnBoot:
      "resume: this FRP instance was running last time and is being restored automatically...",
    routes: {
      saveConfigFailed: "Failed to save configuration",
      startFailed: "Failed to start",
      stopFailed: "Failed to stop",
      createInstanceFailed: "Failed to create instance",
      startInstanceFailed: "Failed to start instance",
      stopInstanceFailed: "Failed to stop instance",
      restartInstanceFailed: "Failed to restart instance",
      getInstanceLogsFailed: "Failed to get instance logs",
      clearInstanceLogsFailed: "Failed to clear instance logs",
      pollInstanceFailed: "Failed to poll instance",
      getInstanceDetailFailed: "Failed to get instance details",
      updateInstanceFailed: "Failed to update instance",
      deleteInstanceFailed: "Failed to delete instance",
    },
  },
  dockerAdminPanel: {
    passwordTooShort: "Admin panel password must be at least 6 characters",
    passwordTooLong: "Admin panel password cannot exceed 128 characters",
    passwordWhitespace: "Admin panel password cannot contain whitespace",
    passwordNeedsLettersAndNumbers:
      "Admin panel password must contain both letters and numbers",
    passwordAlreadyConfigured: "Admin panel password has already been set",
    passwordNotConfigured: "Admin panel password has not been set yet",
    newPasswordSameAsCurrent:
      "New password cannot be the same as the current password",
    resetHelp:
      "fn-knock admin panel password reset tool\n\nUsage:\n  fn-knock-reset-panel-password\n\nActions:\n  - Clear the admin panel password\n  - Clear all admin panel login sessions\n  - Clear login failure backoff state\n\nAfter completion, the next visit to the admin entry will enter the first-time password setup flow again.",
    resetCleared: "[fn-knock] Admin panel password state cleared",
    resetNextVisit:
      "[fn-knock] Set the admin panel password again on the next visit to the admin entry",
    resetFailed: "[fn-knock] Failed to clear admin panel password:",
  },
  passkeyRoutes: {
    notFoundWithRetry: "Passkey not found. Retry in {seconds} seconds.",
    verifyFailedWithRetry: "Verification failed. Retry in {seconds} seconds.",
    bindTokenExpired: "Binding credential has expired",
  },
  maintenanceBackup: {
    commandMissing: "System command is missing: {command}",
    commandFailed: "Failed to run command: {command}",
    commandCheckFailed: "Failed to check command: {command}",
    commandsMissingNoApt:
      "System commands are missing: {commands}. Debian apt-get was not found, so they cannot be installed automatically.",
    commandsMissingNoPackageManager:
      "System commands are missing: {commands}. opkg or Debian apt-get was not found, so they cannot be installed automatically.",
    opkgUpdateFailed: "opkg update failed",
    aptUpdateFailed: "apt-get update failed",
    packageInstallFailed: "Failed to install {packages}",
    commandsStillMissingAfterInstall:
      "Commands are still missing after automatic installation: {commands}",
    commandErrorWithDetail: "{message} (exit code: {code}): {detail}",
    commandError: "{message} (exit code: {code})",
    shareDirectoryMissing:
      "FNOS shared directory was not found. Confirm that app resources are configured correctly.",
    invalidBackupPath: "Invalid backup file path",
    invalidRedisStreamData: "Invalid Redis stream data format: {key} ({id})",
    unsupportedRedisExportType:
      "Unsupported Redis data type for export: {type} ({key})",
    createArchiveFailed: "Failed to create backup archive",
    invalidBackupExtension: "Backup file extension must be {extension}",
    stringArrayRequired: "{label} must be an array of strings",
    stringArrayOnlyStrings: "{label} can only contain strings",
    objectRequired: "{label} must be an object",
    fieldStringRequired: "{label}.{field} must be a string",
    arrayRequired: "{label} must be an array",
    zsetMemberRequired: "{label}[{index}] must contain string member",
    zsetScoreRequired: "{label}[{index}] must contain a valid numeric score",
    streamIdRequired: "{label}[{index}] must contain string id",
    streamFieldsInvalid:
      "{label}[{index}].fields must be a non-empty string array with an even length",
    entryObjectRequired: "entries[{index}] must be an object",
    entryKeyPrefixRequired: "entries[{index}].key must start with {prefix}",
    entryTypeUnsupported: "entries[{index}].type is not supported",
    entryTtlInvalid:
      "entries[{index}].ttl_ms must be a positive integer or null",
    entryValueStringRequired: "entries[{index}].value must be a string",
    jsonParseFailed: "Backup file JSON could not be parsed",
    payloadObjectInvalid: "Backup file content is not a valid object",
    unsupportedSchemaVersion:
      "Only backup files with version={version} are supported",
    unsupportedPrefix: "Only backup files with prefix {prefix} are supported",
    missingAppVersion: "Backup file is missing app_version",
    appVersionUnsupported:
      "Current version {currentVersion} can only import backups exported from {range}; received {appVersion}",
    missingExportedAt: "Backup file is missing exported_at",
    missingEntries: "Backup file is missing the entries array",
    duplicateRedisKey: "Backup file contains duplicate Redis keys",
    archiveMissingPayload: "Backup archive is missing {filename}",
    archivePasswordInvalid: "Backup archive password verification failed",
    readArchiveFailed: "Failed to read .knock backup archive",
    writeRedisFailed: "Failed to write Redis backup data",
    unknownError: "Unknown error",
    syncSteps: {
      runModeGatewayRoutes: "Run mode and gateway routes",
      directModeWhitelist: "Direct-mode whitelist",
      gatewayLogging: "Request log configuration",
      sslDeployment: "SSL certificate deployment",
      legacyAuthLogCleanup: "Legacy auth log cleanup",
      systemResourceMonitorReset: "System resource monitor state reset",
    },
    archiveEmpty: "Backup archive content is empty",
    directoryImportFileOnly:
      "Only files in the backup directory can be imported",
    directoryImportExtensionOnly:
      "Only {extension} backup files can be imported",
    directoryImportTooLarge:
      "Backup file is too large to import from the FNOS directory",
    archiveContentMissing: "Backup archive content is missing",
    archiveBase64Invalid: "Backup archive is not valid Base64 data",
  },
  captcha: {
    powServerNotConfigured: "PoW captcha is not configured on the server",
    providerMismatch: "Captcha type does not match",
    turnstileNotConfigured:
      "Turnstile is not configured. Contact an administrator to complete the settings.",
    turnstileSecretMissing: "Cloudflare Turnstile secret_key is not configured",
    turnstileTokenRequired: "Turnstile token is required",
    turnstileServiceUnavailable:
      "Turnstile verification service is temporarily unavailable",
    turnstileVerifyFailedWithReason: "Turnstile verification failed: {reason}",
    turnstileVerifyFailed: "Turnstile verification failed",
    providerUnavailable: "No available captcha provider was found",
    powNotEnabled: "PoW captcha is not enabled",
    powUnavailable: "PoW captcha is unavailable",
    providerConfigMismatch:
      "Captcha provider does not match the current configuration",
  },
  cidr: {
    serviceError: "CIDR service error",
    emptyResponse: "<empty response>",
    upstreamUrl: "upstream URL: {url}",
    status: "status: {status}{statusText}",
    contentType: "content type: {contentType}",
    upstreamCode: "upstream code: {code}",
    upstreamMessage: "upstream message: {message}",
    requestId: "request ID: {requestId}",
    responsePreview: "response preview: {preview}",
    provinceRequired: "Province is required",
    upstreamTimeout: "CIDR upstream request timed out",
    upstreamRequestFailed: "CIDR upstream request failed ({status})",
    invalidJson: "CIDR upstream returned invalid JSON",
    upstreamUnexpected: "CIDR upstream returned an unexpected response",
    provinceWideLabel: "All {province}",
  },
  dashboard: {
    inbound: "Inbound",
    outbound: "Outbound",
    upstreamUnavailable: "Upstream service is unavailable",
    hostRequired: "host is required",
  },
  acme: {
    alreadyInstalled: "acme.sh is already installed",
    installInProgress: "Installation task is already in progress",
    installSubmitted: "Installation task submitted",
    issueSucceeded: "Certificate issued successfully",
  },
  ddns: {
    ipv6OnlyUnavailable:
      "Update scope is IPv6 only, but no usable IPv6 address was detected",
    ipv4OnlyUnavailable:
      "Update scope is IPv4 only, but no usable IPv4 address was detected",
    dualStackUnavailable:
      "No usable IPv4 or IPv6 address was detected within the update scope",
    domainConfigIncomplete: "Domain configuration is incomplete",
    domainNotInZone: "Domain {fqdn} does not belong to root zone {zone}",
    invalidJsonResponse: "Response is not valid JSON: {text}",
    aRecordFailed: "A record processing failed",
    aaaaRecordFailed: "AAAA record processing failed",
    providerDnsUpdateSuccess: "{provider} DNS update succeeded",
    aliyunParamKeyMissing: "Aliyun request parameter is missing a key name",
    requestFailed: "Request failed",
    tencentMissingResponse:
      "HTTP {status}: Tencent Cloud API response is missing Response",
    invalidHeaderFormat: "Invalid Header format: {header}",
    interfaceSourceLabel: "Interface {name}",
    selectedInterfaceSourceLabel: "Selected interface",
    publicSourceLabel: "Public network",
    interfaceIpv6Unavailable:
      "IP source is direct interface, but the selected interface has no usable IPv6 address",
    interfaceIpv4Unavailable:
      "IP source is direct interface, but the selected interface has no usable IPv4 address",
    interfaceDualStackUnavailable:
      "IP source is direct interface, but the selected interface has no usable IPv4 or IPv6 address",
    publicIpv6Unavailable:
      "IP source is public network, but no usable IPv6 address was obtained",
    publicIpv4Unavailable:
      "IP source is public network, but no usable IPv4 address was obtained",
    publicDualStackUnavailable:
      "IP source is public network, but no usable IPv4 or IPv6 address was obtained",
    selectInterfaceAddress:
      "Select a {family} address before using direct interface mode",
    selectedInterfaceAddressUnavailable:
      "The selected interface's {index} {family} address is no longer available. Select again.",
    ipv4FailedContinueIpv6:
      "IPv4 detection failed; continuing with IPv6 ({error})",
    ipv4Failed: "IPv4 detection failed ({error})",
    ipv6FailedContinueIpv4:
      "IPv6 detection failed; continuing with IPv4 ({error})",
    ipv6Failed: "IPv6 detection failed ({error})",
    publicIpv6NotSelectable:
      "Public detection returned IPv6 ({ip}), but it is not among selectable interface addresses on this machine or Docker host. If it cannot be reached externally, use direct interface mode and select the host public IPv6.",
    interfaceRequired:
      "Select an outbound interface before using direct interface mode",
    interfaceNotFound: "No usable interface found: {name}",
    dockerHostInterfaceLabel: "Host {name} ({summary})",
    curlStatusLineParseFailed:
      "Unable to parse curl response status line: {line}",
    curlNoHeaders: "curl did not return any response headers",
    requestCanceled: "Request was canceled",
    curlRequestFailed: "curl request failed: {detail}",
    triggerCron: "Scheduled check",
    triggerEnable: "Immediate check after enabling automatic updates",
    triggerMessage: "{trigger}: {message}",
    notConfigured: "Not configured",
    skippedNoProvider: "No DDNS provider selected; skipped",
    skippedIncompleteConfig: "Current configuration is incomplete; skipped",
    skippedPublicIpUnavailable: "Unable to get public IP; skipped",
    skippedReason: "{reason}; skipped",
    targetIpNoChange: "Target IP did not change; no update needed",
    none: "none",
    ipChange: "{family}: {before} -> {after}",
    targetIpChanged: "Detected target IP change: {changes}",
    dnsUpdateSuccess: "DNS update succeeded [{provider}]: {message}",
    dnsUpdateFailed: "DNS update failed [{provider}]: {message}",
    taskError: "Task error: {message}",
    intervalOutOfRange:
      "Automatic sync interval must be an integer between {min} and {max} minutes",
    primaryDomainName: "Primary domain",
    noProviderSelected: "No provider selected",
    duplicateTarget:
      "A DDNS entry with the same provider and domain summary already exists",
    primaryInitFailed: "Failed to initialize the primary DDNS entry",
    primaryDomainScope: "Primary domain",
    additionalDomainScope: "Additional domain",
    targetNotFound: "DDNS entry not found",
    unknownProvider: "Unknown DDNS provider: {provider}",
    primaryDeleteForbidden: "Primary domain entry cannot be deleted",
    primaryDisableForbidden: "Primary domain entry cannot be disabled alone",
    unknownProviderShort: "Unknown provider: {provider}",
    selectProviderFirst: "Select a DDNS provider first",
    primaryConfigIncomplete:
      "Current primary domain configuration is incomplete. Fill in all required fields.",
    targetConfigIncomplete:
      "Current entry configuration is incomplete. Fill in all required fields.",
    manualTestStart: "Manual test started; resolving current target IP...",
    manualTestPrefix: "Manual test",
    currentTargetIp:
      "Current target IP ({source}) — IPv4: {ipv4}, IPv6: {ipv6}",
    testAborted: "{message}; test aborted",
    updateSuccess: "Update succeeded: {message}",
    updateFailed: "Update failed: {message}",
    testError: "Test error: {message}",
    settingsSaveFailed: "Failed to save DDNS automatic sync settings",
    providerSetFailed: "Failed to set provider",
    configSaveFailed: "Failed to save DDNS configuration",
    createTargetFailed: "Failed to create DDNS entry",
    updateTargetFailed: "Failed to update DDNS entry",
    deleteTargetFailed: "Failed to delete DDNS entry",
    updateTargetEnabledFailed: "Failed to update DDNS entry enabled state",
    providers: {
      common: {
        fields: {
          root_domain: {
            label: "Root domain",
            description: "Used to determine the Zone, such as example.com",
          },
          domain: {
            label: "Full domain",
            shortLabel: "Domain",
            description: "Full domain name to update",
            hostDescription: "Full hostname to update",
          },
          ttl: {
            description: "Default {seconds} seconds",
          },
        },
      },
      dynv6: {
        fields: {
          token: {
            description: "Generated in your dynv6.com account",
          },
          zone: {
            label: "Zone name",
            description: "Your dynv6 zone domain",
          },
          ipv6prefix: {
            description: "Optional. Passed through to the dynv6 API",
          },
        },
        configIncomplete: "dynv6 configuration is incomplete",
        empty: "(empty)",
        success: "dynv6: {detail} (sent: {params})",
        updateFailed: "dynv6 update failed [{status}]: {detail}",
        requestError: "dynv6 request error: {detail}",
      },
      duckdns: {
        fields: {
          domains: {
            label: "Subdomains",
            description:
              "Enter only DuckDNS subdomains without the .duckdns.org suffix. Comma-separated values are supported.",
          },
          token: {
            description: "Account token shown on the DuckDNS console home page",
          },
        },
        configIncomplete: "DuckDNS configuration is incomplete",
        noIpAvailable: "DuckDNS update failed: no usable IPv4 or IPv6 address",
        updateFailedWithStatus: "DuckDNS update failed [{status}]: {detail}",
        requestFailed: "Request failed",
        updateFailed: "DuckDNS update failed: {detail}",
        nonOkResponse: "Returned a non-OK response",
        success: "DuckDNS update succeeded{detail}",
        requestError: "DuckDNS request error: {detail}",
      },
      dnspod: {
        fields: {
          record_line: {
            label: "Line",
            description: "Uses the default line by default",
          },
        },
        defaultLine: "Default",
        configIncomplete: "DNSPod configuration is incomplete",
        queryRecordFailed: "Failed to query record",
        updateRecordFailed: "Failed to update record",
        createRecordFailed: "Failed to create record",
      },
      cloudflare: {
        fields: {
          api_token: {
            label: "API token",
            description: "Requires Zone.DNS edit permission",
          },
          zone_id: {
            description:
              "On the Cloudflare domain page, click the three dots and choose copy Zone ID",
          },
          proxied: {
            label: "Cloudflare proxy",
            description:
              "Whether to enable the Cloudflare proxy (orange cloud)",
            options: {
              dnsOnly: "DNS only",
              orangeCloud: "Orange cloud",
            },
          },
        },
        configIncomplete: "Cloudflare configuration is incomplete",
        searchRecordFailed: "Failed to query {type} record: {detail}",
        updateRecordFailed: "Failed to update {type} record: {detail}",
        createRecordFailed: "Failed to create {type} record: {detail}",
        recordOperationError: "{type} record operation error: {detail}",
        success: "Cloudflare DNS update succeeded",
      },
      godaddy: {
        configIncomplete: "GoDaddy configuration is incomplete",
        updateFailed: "Update failed",
        updateFailedWithStatus: "[{status}] {detail}",
      },
      porkbun: {
        configIncomplete: "Porkbun configuration is incomplete",
        queryRecordFailed: "Failed to query record",
        updateRecordFailed: "Failed to update record",
        createRecordFailed: "Failed to create record",
      },
      alidns: {
        label: "Aliyun DNS",
        fields: {
          access_key_secret: {
            placeholder: "Aliyun AccessKey Secret",
          },
          line: {
            label: "Line",
            description: 'Uses Aliyun "default" line by default',
          },
        },
        configIncomplete: "Aliyun DNS configuration is incomplete",
        requestFailed: "Request failed",
        updateFailed: "Update failed",
        createFailed: "Create failed",
      },
      baidu: {
        label: "Baidu Cloud DNS",
        fields: {
          access_key_id: {
            placeholder: "Baidu AI Cloud Access Key",
          },
          secret_access_key: {
            placeholder: "Baidu AI Cloud Secret Key",
          },
        },
        configIncomplete: "Baidu Cloud DNS configuration is incomplete",
        queryFailed: "Query failed",
        updateFailed: "Update failed",
        createFailed: "Create failed",
      },
      huawei: {
        label: "Huawei Cloud DNS",
        fields: {
          access_key_id: {
            placeholder: "Huawei Cloud AK",
          },
          secret_access_key: {
            placeholder: "Huawei Cloud SK",
          },
        },
        webCryptoUnsupported:
          "The current runtime does not support Web Crypto, so Huawei Cloud AK/SK signatures cannot be generated",
        configIncomplete: "Huawei Cloud DNS configuration is incomplete",
        requestFailed:
          "Huawei Cloud DNS request failed: HTTP {status} {statusText}, {detail}",
        zoneNotFound: "Huawei Cloud Zone not found: {zone}",
      },
      tencentcloud: {
        label: "Tencent Cloud DNS",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          record_line: {
            label: "Line",
            description: "Uses the default line by default",
          },
          record_line_id: {
            label: "Line ID",
            description: "Optional. If set, Line ID takes priority.",
          },
        },
        defaultLine: "Default",
        configIncomplete: "Tencent Cloud DNS configuration is incomplete",
        missingUpdatedRecordId:
          "Tencent Cloud did not return the updated RecordId",
        missingCreatedRecordId:
          "Tencent Cloud did not return the created RecordId",
      },
      noip: {
        fields: {
          hostname: {
            description:
              "Enter full hostnames. Multiple hostnames can be comma-separated.",
          },
          username: {
            label: "Username",
            description:
              "Use the DDNS Key username generated in the NO-IP console.",
          },
          password: {
            label: "Password",
            description:
              "Use the password paired with the DDNS Key, not the main account password.",
          },
        },
        statusMessages: {
          nohost:
            "The specified hostname does not exist or does not belong to the current DDNS Key",
          badauth: "Username or password is incorrect",
          badagent:
            "The client is disabled by NO-IP. Check User-Agent or client status.",
          "!donator":
            "The current account does not support the enhanced feature in this request",
          abuse: "This DDNS Key was blocked by NO-IP for abuse",
          "911":
            "NO-IP has a temporary server-side failure. Official guidance is to retry after at least 30 minutes.",
        },
        unknownStatus: "Returned unknown status: {code}",
        updateFailed: "NO-IP update failed: {detail}",
        updateSuccess: "NO-IP update succeeded{detail}",
        ipUnchanged: "NO-IP IP did not change{detail}",
        configIncomplete: "NO-IP configuration is incomplete",
        noIpAvailable: "NO-IP update failed: no usable IPv4 or IPv6 address",
        updateFailedWithStatus: "NO-IP update failed [{status}]: {detail}",
        requestFailed: "Request failed",
        emptyResponse: "NO-IP update failed: returned an empty response",
        requestError: "NO-IP request error: {detail}",
      },
      esa: {
        label: "Aliyun ESA DNS",
        fields: {
          access_key_secret: {
            placeholder: "Aliyun AccessKey Secret",
          },
          site_name: {
            label: "Site name",
            description:
              "ESA site name, usually the root domain. If Site ID is set, this is only used as a fallback lookup.",
          },
          site_id: {
            description:
              "Optional. When set, this site is operated directly instead of querying the site list first.",
          },
          proxied: {
            label: "ESA proxy",
            description:
              "DNS-only by default. When proxying is enabled, the business type is sent automatically.",
            options: {
              dnsOnly: "DNS only",
              enabled: "Enable proxy",
            },
          },
          biz_name: {
            label: "Business type",
            description:
              "Only applies when ESA proxying is enabled. Defaults to web.",
            options: {
              web: "Web",
              api: "API",
              imageVideo: "Audio/video",
            },
          },
        },
        configIncomplete: "Aliyun ESA DNS configuration is incomplete",
        siteNameMissing: "Aliyun ESA DNS site name is missing",
        siteNotFound: "ESA site not found: {site}",
        noIpAvailable: "Aliyun ESA DNS has no IP address to update",
        createRecordFailed: "CreateFailed: failed to create record",
        success: "Aliyun ESA DNS update succeeded",
        recordIdMissing: "UpdateFailed: record is missing RecordId",
      },
      dynu: {
        fields: {
          api_key: {
            description: "API-Key generated in Dynu API Credentials",
          },
          domain: {
            description: "Full Dynu hostname to update",
          },
          group: {
            description: "Optional. Group written to the Dynu DNS record.",
          },
        },
        actionFailed: "{action} failed",
        actions: {
          resolveRoot: "Resolve Dynu root domain",
          readDnsService: "Read Dynu DNS service",
          updateWildcardAlias: "Update Dynu Wildcard Alias",
          queryRecord: "Query Dynu {type} record",
          updateRecord: "Update Dynu {type} record",
          createRecord: "Create Dynu {type} record",
        },
        invalidRootInfo: "Dynu did not return valid root domain information",
        wildcardUnsupported:
          "Dynu REST does not support using *.{domain} as a DNS record nodeName. Add {domain} as an independent service in Dynu DDNS Services and enable Wildcard Alias, or change the DDNS configuration to {domain}.",
        wildcardUnchanged: "Dynu Wildcard Alias IP did not change",
        wildcardSuccess: "Dynu Wildcard Alias update succeeded",
        configIncomplete: "Dynu configuration is incomplete",
        noIpAvailable: "Dynu update failed: no usable IPv4 or IPv6 address",
        recordIdMissing: "Dynu DNS record is missing RecordId",
        requestError: "Dynu request error: {detail}",
      },
      edgeone: {
        label: "Tencent Cloud EdgeOne",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          zone_id: {
            description: "EdgeOne site ID used to locate the hosted Zone",
          },
          domain: {
            description:
              "Full hostname to update. Convert internationalized domain names to punycode first.",
          },
          location: {
            label: "Line",
            placeholder: "Default or CN.BJ",
            description:
              "Optional. Leave empty to use the Default global line.",
          },
          ttl: {
            description: "Default 300 seconds. EdgeOne allows 60-86400.",
          },
          overseas_access: {
            label: "Overseas access control",
            description:
              "When enabled, the EdgeOne security policy API blocks overseas IP access. Hong Kong, Macau, and Taiwan are not considered overseas. This syncs once when configuration changes and is not repeated on every DDNS update.",
            options: {
              off: "Off",
              blockOverseas: "Block overseas IPs",
            },
          },
          endpoint: {
            description:
              "Defaults to the mainland endpoint. You can use https://teo.intl.tencentcloudapi.com or a regional endpoint.",
          },
          region: {
            placeholder: "Empty",
            description: "Optional. Most scenarios can leave this empty.",
          },
        },
        configIncomplete: "Tencent Cloud EdgeOne configuration is incomplete",
        configTargetIncomplete:
          "Tencent Cloud EdgeOne configuration is incomplete: Zone ID or domain is missing",
        missingRecordId: "EdgeOne returned a record without RecordId",
        missingCreatedRecordId: "EdgeOne did not return the created RecordId",
        overseasAccess: {
          describeRulesFailed:
            "EdgeOne overseas access control failed to read existing custom rules (provider_target={target}, zone_id={zoneId}, endpoint_host={endpointHost}, region={region}, entity={entity}, scope={scope}): {message}",
          syncFailedWithAttempt:
            "EdgeOne overseas access control sync failed ({attempt}, submitted_rule_count={count}): {message}",
          syncAllScopesFailed:
            "EdgeOne overseas access control sync failed: all rule scopes failed",
          cleanupAllScopesFailed:
            "EdgeOne overseas access control cleanup failed: all rule scopes failed",
          syncSuccess:
            "EdgeOne overseas IP blocking policy synced. Only mainland China, Hong Kong, Macau, and Taiwan are allowed.",
          cleanupSuccess: "EdgeOne overseas IP blocking policy was cleared",
        },
      },
      edgeone_cname: {
        label: "Tencent Cloud EdgeOne (CNAME access)",
        fields: {
          secret_key: {
            placeholder: "Tencent Cloud SecretKey",
          },
          zone_id: {
            description:
              "EdgeOne site ID used to locate the acceleration domain's site",
          },
          domain: {
            label: "Acceleration domain",
            description:
              "Acceleration domain already created in EdgeOne. Only IP_DOMAIN origins are supported, and only one origin address can be updated at a time.",
          },
          overseas_access: {
            label: "Overseas access control",
            description:
              "When enabled, the EdgeOne security policy API blocks overseas IP access. Hong Kong, Macau, and Taiwan are not considered overseas. This syncs once when configuration changes and is not repeated on every DDNS update.",
            options: {
              off: "Off",
              blockOverseas: "Block overseas IPs",
            },
          },
          endpoint: {
            description:
              "Defaults to the mainland endpoint. You can use https://teo.intl.tencentcloudapi.com or a regional endpoint.",
          },
          region: {
            placeholder: "Empty",
            description: "Optional. Most scenarios can leave this empty.",
          },
        },
        configIncomplete:
          "Tencent Cloud EdgeOne (CNAME access) configuration is incomplete",
        singleAddressOnly:
          'Tencent Cloud EdgeOne (CNAME access) can update only one origin address at a time. Set the DDNS update scope to "IPv4 only" or "IPv6 only".',
        noIpAvailable:
          "Tencent Cloud EdgeOne (CNAME access) has no IP address to update",
        domainNotFound: "EdgeOne acceleration domain not found: {domain}",
        unsupportedOriginType:
          "Current acceleration domain origin type is {originType}. Only IP_DOMAIN acceleration domains can be updated by DDNS.",
        originUnchanged:
          "Tencent Cloud EdgeOne (CNAME access) origin is already up to date",
        successWithInvalidHostHeaderIgnored:
          "Tencent Cloud EdgeOne (CNAME access) origin updated successfully (ignored invalid Host Header)",
        success:
          "Tencent Cloud EdgeOne (CNAME access) origin updated successfully",
      },
    },
  },
  smartConnect: {
    runTypes: {
      direct: "direct mode",
      reverseProxy: "reverse proxy mode",
      subdomain: "subdomain mode",
    },
    currentMode: "current mode",
    unavailableReason:
      "Only subdomain mode is available. Current mode: {mode}.",
    selectLocalIp: "Select the local LAN IP",
    selectValidLocalIpv4: "Select a valid local LAN IPv4 address",
    dnsmasqNotInstalled: "dnsmasq was not detected. Install it first.",
    dnsmasqNotInitialized:
      "dnsmasq has not finished initialization. Complete environment initialization first.",
    syncFailed: "Smart Connect sync failed",
  },
  scanDiscovery: {
    localIpv4CidrOnly: "Scan ranges only support local IPv4 CIDR: {cidrs}",
    maxCidrsExceeded: "Select at most {max} scan ranges at a time",
    maxHostsExceededWithCurrent:
      "Scan at most {max} hosts at a time; current selection has {current} hosts",
    maxHostsExceeded: "Scan at most {max} hosts at a time",
    selectAtLeastOneCidr: "Select at least one local IPv4 scan range",
    targetLabels: {
      docker: "{cidr} (Docker host LAN)",
      loopback: "{cidr} (local loopback)",
      interface: "{cidr} ({name})",
      mapping: "{cidr} (existing mapping target)",
      custom: "{cidr} (custom)",
      saved: "{cidr} (saved)",
    },
    serviceLabels: {
      lottery: "Lottery Assistant",
      dlymusic: "Daoliyu Music Manager",
      kuake: "Quark Auto Transfer",
      xunlei: "Xunlei",
      nowen: "Nebula Portal",
      fnos: "FNOS",
      fnys: "FNOS Video",
      xiaoyaAlist: "Xiaoya Alist",
    },
  },
  gatewayProxyHeaders: {
    runTypes: {
      direct: "direct mode",
      reverseProxy: "reverse proxy mode",
      subdomain: "subdomain mode",
    },
    unavailableReason:
      "Only subdomain mode is available. Current mode: {mode}.",
    syncFailed: "Failed to sync gateway proxy header configuration",
  },
  sshSecurity: {
    logSourceUnavailable:
      "journalctl or /var/log/auth.log was not found on this system",
    openWrtUnsupported:
      "SSH security is not supported in the OpenWrt build yet",
    enableUnavailable: "SSH security cannot be enabled in this environment",
    syncFirewallUnavailable:
      "SSH firewall cannot be synced in this environment",
    clearFirewallUnavailable:
      "SSH firewall cannot be cleared in this environment",
    logSourceUnavailableShort: "SSH log source is unavailable",
    customCidrInvalid: "Custom CIDR format is invalid: {cidrs}",
    syncSshPolicyFailed: "Failed to sync SSH dedicated firewall rules",
    clearSshPolicyFailed: "Failed to clear SSH dedicated firewall rules",
    blockRecordInvalid: "Block record format is invalid",
    routes: {
      updateConfigFailed: "Failed to update SSH security configuration",
      syncFirewallSuccess:
        "Synced {allowedCidrs} allowed CIDRs and {synced} SSH blocked IPs to ports {ports}",
      syncFirewallFailed: "Failed to sync SSH firewall",
      clearFirewallSuccess: "Cleared SSH dedicated firewall rules",
      clearFirewallFailed: "Failed to clear SSH firewall",
      readLoginLogsFailed: "Failed to read SSH login logs",
      blockNotFound: "Block record not found",
      removeBlockFailed: "Failed to remove block",
      selectIps: "Select IPs to unblock",
      removeBlocksFailed: "Failed to remove blocks",
    },
  },
  notifications: {
    brand: {
      prefix: "Knock ",
      defaultTitle: "Knock notification",
    },
    templates: {
      events: {
        authLoginSuccess: "Login succeeded",
        authLogout: "Signed out",
        authLoginFailure: "Login failed",
        authSessionIpDrift: "Session IP drift",
        securityScannerBlocked: "Scanner blocked",
        ddnsUpdateCompleted: "DDNS updated",
        gatewayThrottleBlocked: "Gateway throttling blocked",
        wafBlocked: "WAF blocked",
        sshLoginSuccess: "SSH login succeeded",
        sshLoginFailure: "SSH login failed",
        sshIpBlocked: "SSH IP blocked",
        appUpdateAvailable: "Application update available",
        cpuAlert: "CPU alert",
        cpuRecovered: "CPU recovered",
        memoryAlert: "Memory alert",
        memoryRecovered: "Memory recovered",
        frpConnected: "FRP connected",
        frpDisconnected: "FRP disconnected",
        cloudflaredConnected: "Cloudflared connected",
        cloudflaredDisconnected: "Cloudflared disconnected",
      },
      ruleName: "{event} notification",
      levels: {
        info: "Info",
        warn: "Warning",
        error: "Error",
        critical: "Critical",
      },
      sources: {
        serverAdmin: "Admin backend",
        goReauthProxy: "Auth proxy",
        systemMonitor: "System monitor",
      },
      authMethods: {
        oidc: "External account",
      },
      grantTypes: {
        browserSession: "Browser session",
        loginIpGrant: "Login IP grant",
      },
      wafModes: {
        detection: "Detection",
        blocking: "Blocking",
        off: "Off",
      },
      wafActions: {
        block: "Block",
        deny: "Deny",
        detect: "Detect",
        log: "Log",
        pass: "Pass",
      },
      logoutSources: {
        userLogout: "User signed out",
        adminSessionDelete: "Administrator ended session",
      },
      driftSources: {
        proxySession: "Proxy session",
        fnosToken: "FNOS token",
        sessionRefresh: "Session refresh",
        browserSession: "Browser session",
      },
      ddnsTriggers: {
        cron: "Scheduled task",
        enable: "First run after enabling",
        manualTest: "Manual test",
      },
      ddnsUpdateScopes: {
        ipv4Only: "IPv4 only",
        ipv6Only: "IPv6 only",
      },
      ddnsIpSources: {
        public: "Public detection",
        interface: "Interface read",
      },
      updateCheckReasons: {
        cron: "Scheduled check",
        manual: "Manual check",
        manualCheckAndDownload: "Manual check and download",
        downloadBootstrap: "Pre-download check",
      },
      credential: "Credential",
      unknownCredential: "Unknown credential",
      credentialLinkedTotp:
        '{authMethod} "{credential}" linked to TOTP "{totp}"',
      credentialName: 'Credential "{credential}"',
      sessionCommentCompact: "Note: {comment}",
      appendSessionComment: "{text} (note: {comment})",
      yes: "Yes",
      no: "No",
      wafOutcomeBlocked: "blocked",
      wafOutcomeLogged: "logged",
      sections: {
        overview: "Event overview",
        aggregation: "Aggregation",
        advice: "Recommended action",
      },
      aggregationText:
        "This notification aggregated {count} similar events within a {seconds}-second window.",
      details: {
        units: {
          seconds: "{count} seconds",
          minutes: "{count} minutes",
          times: "{count} times",
          ratePerSecond: "{count}/s",
        },
        listSeparator: ", ",
        unknown: "Unknown",
        unknownIp: "Unknown IP",
        unknownMethod: "Unknown method",
        unknownProvider: "Unknown provider",
        unknownUser: "Unknown user",
        unknownHost: "Unknown host",
        currentSession: "Current session",
        memoryMetric: "Memory",
        connected: "Connected",
        disconnected: "Disconnected",
        parenthesized: " ({value})",
        sessionCommentSentence: 'Current session note: "{comment}".',
        aggregationStatsValue: "{count} events / {seconds}-second window",
        facts: {
          credentialName: "Credential name",
          linkedTotp: "Linked TOTP",
          sessionComment: "Session note",
          loginIp: "Login IP",
          ipLocation: "IP location",
          authMethod: "Auth method",
          loginProvider: "Login provider",
          grantType: "Grant type",
          rememberLogin: "Remember login",
          sessionExpiresAt: "Session expires at",
          sessionId: "Session ID",
          logoutSource: "Logout source",
          loginTime: "Login time",
          sourceIp: "Source IP",
          failureAttempts: "Failed attempts",
          retryWait: "Retry wait",
          limitUntil: "Limited until",
          originalIp: "Original IP",
          originalLocation: "Original location",
          currentIp: "Current IP",
          currentLocation: "Current location",
          driftSource: "Drift source",
          hitCount: "Hit count",
          observationWindow: "Observation window",
          triggerThreshold: "Trigger threshold",
          blockedAt: "Blocked at",
          recentPaths: "Recent paths",
          target: "Target",
          provider: "Provider",
          targetType: "Target type",
          trigger: "Trigger",
          updateScope: "Update scope",
          ipSource: "IP source",
          ipv4Change: "IPv4 change",
          ipv6Change: "IPv6 change",
          result: "Result",
          blockDuration: "Block duration",
          blockedUntil: "Blocked until",
          rateLimit: "Rate limit",
          burstCapacity: "Burst capacity",
          targetHost: "Target host",
          requestPath: "Request path",
          routeType: "Route type",
          authRoute: "Auth route",
          traceId: "Trace ID",
          requestAddress: "Request address",
          outcome: "Outcome",
          wafAction: "WAF action",
          wafMode: "WAF mode",
          ruleIds: "Rule IDs",
          ruleBundle: "Rule bundle",
          statusCode: "Status code",
          user: "User",
          port: "Port",
          logTime: "Log time",
          invalidUser: "Invalid user",
          threshold: "Threshold",
          window: "Window",
          blockedReason: "Block reason",
          relatedUser: "Related user",
          currentVersion: "Current version",
          latestVersion: "Latest version",
          checkReason: "Check reason",
          forceUpdate: "Force update",
          releaseNotes: "Release notes",
          hostname: "Hostname",
          currentUsage: "Current usage",
          alertThreshold: "Alert threshold",
          recoverThreshold: "Recovery threshold",
          sampleInterval: "Sample interval",
          sustainDuration: "Sustain duration",
          tunnelType: "Tunnel type",
          connectionStatus: "Connection status",
          processPid: "Process PID",
          runtimeFeedback: "Runtime feedback",
          eventType: "Event type",
          riskLevel: "Risk level",
          eventSource: "Event source",
          happenedAt: "Occurred at",
          aggregationStats: "Aggregation",
        },
        authLoginSuccess: {
          loginViaProvider: "Signed in through {provider}",
          loginWithMethod: "using {method}",
          authViaProvider: "through {provider}",
          authWithMethod: "using {method}",
          summaryOidc: "{credential} {method} succeeded from IP {ip}{totpPart}",
          linkedTotpPart: ', linked TOTP "{totp}"',
          summaryTotp:
            '{method} "{credential}" with linked TOTP "{totp}" signed in from {ip}',
          summaryCredential: 'Credential "{credential}" signed in from {ip}',
          overview:
            "This login completed authentication {auth}; grant type: {grantType}{locationPart}. {commentPart}",
          locationPart: ", location: {location}",
          advice:
            "If this login was not yours, revoke the session promptly and review access policies.",
        },
        authLogout: {
          summaryTotp:
            '{method} "{credential}" with linked TOTP "{totp}" signed out',
          summaryCredential: 'Credential "{credential}" signed out',
          overview:
            "This session signed out from {ip}{locationPart}; logout source: {source}. {commentPart}",
          advice:
            "If this logout was unexpected, check whether an administrator ended the session or abnormal cleanup occurred.",
        },
        authLoginFailure: {
          summary: "Login failures from {ip} reached {attempts} attempts",
          overview:
            "Repeated login authentication failures were detected. Current source IP: {ip}{retryPart}{blockedPart}.",
          retryPart: "; retry after {seconds} seconds",
          blockedPart: "; restriction lasts until {time}",
          advice:
            "If this was not you, check credential security immediately and consider blocking the source IP or raising login protection.",
        },
        authSessionIpDrift: {
          summary: "{session} IP changed from {fromIp} to {toIp}",
          overview:
            "The access source IP for {session} changed; source classification: {source}. {commentPart}This is usually related to network switching, proxy changes, or session anomalies.",
          advice:
            "If this IP change was unexpected, check the current session for takeover risk as soon as possible.",
        },
        securityScannerBlocked: {
          summary: "{ip} was blocked for scan behavior",
          overview:
            "This source triggered {hits} scan hits within {minutes} minutes, exceeding the threshold of {threshold}{pathsPart}.",
          pathsPart: "; recent matched paths include {paths}",
          advice:
            "Review gateway logs to confirm whether this was malicious probing. If it was a false positive, adjust scan thresholds.",
        },
        ddnsUpdateCompleted: {
          defaultTarget: "DDNS target",
          summarySuccess: "{target} DDNS update succeeded",
          summaryFailure: "{target} DDNS update failed",
          currentTask: "This task",
          overview:
            "{trigger} executed a DDNS update. Scope: {scope}; IP source: {ipSource}. {resultPart}",
          resultPart: "Result: {message}",
          adviceSuccess:
            "If DNS resolution has not taken effect, wait for DNS caches to refresh and then verify external access again.",
          adviceFailure:
            "Check provider credentials, DNS record configuration, and public IP detection status.",
          primaryDomain: "Primary domain",
          additionalDomain: "Additional domain",
        },
        gatewayThrottleBlocked: {
          summary:
            "{ip} was blocked for {seconds} seconds due to fast requests",
          overview:
            "This source triggered gateway throttling. Rate limit: {rate}/s; burst capacity: {burst}{targetPart}.",
          targetPart: "; target request: {target}",
          advice:
            "Review access logs to determine whether this was burst traffic, a false positive, or malicious traffic, then adjust throttling as needed.",
        },
        wafBlocked: {
          summary: "{ip}'s request was {outcome} by WAF",
          overview:
            "WAF {outcome} source {ip}{hostPart}{pathPart}{actionPart}{modePart}. {rulesPart}",
          hostPart: " accessing {host}",
          pathPart: " {path}",
          actionPart: "; action: {action}",
          modePart: "; current mode: {mode}",
          rulesPart: "Matched rules: {rules}.",
          adviceBlocked:
            "Use the Trace ID in WAF logs to inspect the hit details. If this is a false positive, report the issue to the project maintainers.",
          adviceLogged:
            "Use the Trace ID in WAF logs to inspect hit details, then decide whether policy changes are needed based on rules and request context.",
        },
        sshLoginSuccess: {
          summary: 'SSH user "{username}" signed in from {ip}',
          overview: "An SSH login succeeded from {ip}{locationPart}{authPart}.",
          authPart: "; auth method: {authMethod}",
          advice:
            "If this login was unexpected, check SSH accounts, keys, and source access policies.",
        },
        sshLoginFailure: {
          summary: 'SSH user "{username}" failed to sign in from {ip}',
          overview:
            "This source accumulated {attempts}/{threshold} SSH login failures within a {minutes}-minute window{locationPart}.",
          locationPart: "; location: {location}",
          advice:
            "Watch whether failures approach the block threshold. Tighten SSH exposure or adjust credentials if needed.",
        },
        sshIpBlocked: {
          reasonCidrNotAllowed: "outside the allowed region range",
          reasonFailedThreshold: "failed attempts reached the threshold",
          summary: "{ip} was blocked by SSH security",
          overview:
            "SSH security blocked source {ip}{locationPart}; reason: {reason}.",
          advice:
            "Confirm whether this source is trusted. If it was blocked by mistake, unblock it from the SSH security block list.",
        },
        appUpdateAvailable: {
          currentVersionUnknown: "Current version unknown",
          targetVersionUnknown: "Target version unknown",
          summary: "New version {version} available",
          currentCheck: "This check",
          overview:
            "{reason} found that fn-knock can upgrade from {localVersion} to {latestVersion}{forcePart}.",
          forcePart: "; schedule the update soon",
          releaseNotesAdvice: "Release notes: {releaseNotes}",
          advice:
            "Complete the update in a suitable maintenance window, and confirm current configuration and service status before installation.",
        },
        systemMetric: {
          recoveredSummary: "{hostname} {metric} usage recovered to {usage}%",
          alertSummary: "{hostname} {metric} usage rose to {usage}%",
          recoveredOverview:
            "{hostname} {metric} usage fell back to {usage}%; recovery line: {recover}%; previous alert threshold: {threshold}%.",
          alertOverview:
            "{hostname} {metric} usage is now {usage}%, exceeding the alert threshold {threshold}%; recovery line is {recover}%.",
          recoveredAdvice:
            "Resources have returned to a safer range. Continue observing for repeated fluctuations.",
          alertAdvice:
            "Check high-load processes, background jobs, or external traffic changes soon to avoid sustained resource saturation.",
        },
        tunnel: {
          connectedSummary: "{tunnel} connected",
          disconnectedSummary: "{tunnel} disconnected",
          connectedOverview:
            "{tunnel} tunnel connection has recovered{messagePart}.",
          connectedMessagePart: "; runtime feedback: {message}",
          disconnectedOverview:
            "{tunnel} tunnel connection is disconnected{messagePart}.",
          disconnectedMessagePart: "; current feedback: {message}",
          connectedAdvice:
            "If you were troubleshooting access, verify the external entry again now.",
          disconnectedAdvice:
            "Check tunnel configuration, upstream network state, and remote service reachability.",
        },
        short: {
          loginFailureAttempts: "{count} failures",
          scanHits: "{count} scan hits",
          scanBlocked: "Scanner blocked",
          success: "Success",
          failure: "Failure",
          blockSeconds: "Blocked {seconds}s",
          blockTriggered: "Block triggered",
          rules: "Rules {rules}",
          sshLoginSuccess: "SSH login succeeded",
          sshLoginFailure: "SSH login failed",
          regionNotAllowed: "Region not allowed",
          failureThreshold: "Failure threshold",
          currentVersion: "Current {version}",
        },
        titles: {
          ddnsUpdateSuccess: "{target} update succeeded",
          ddnsUpdateFailure: "{target} update failed",
          credentialIpDrift: 'Credential "{credential}" IP drift',
          appUpdateAvailable: "New version {version} available",
        },
      },
    },
    providers: {
      catalog: {
        email: {
          label: "Email",
          description:
            "Send email notifications through SMTP, with optional IMAP settings saved for unified mailbox connection management.",
          fields: {
            smtp_host: {
              label: "SMTP host",
              description:
                "Mail sending server address, such as smtp.example.com.",
            },
            smtp_port: {
              label: "SMTP port",
              description: "Common ports are 465 (SSL/TLS) and 587 (STARTTLS).",
            },
            smtp_security: {
              label: "SMTP encryption",
              options: {
                none: "No encryption",
              },
            },
            smtp_auth_mode: {
              label: "SMTP auth mode",
              description:
                "Automatically prefers AUTH PLAIN and falls back to AUTH LOGIN when needed.",
              options: {
                auto: "Auto negotiate",
                none: "No auth",
              },
            },
            smtp_username: {
              label: "SMTP username",
            },
            smtp_password: {
              label: "SMTP password",
            },
            from_address: {
              label: "From address",
              description: "Used as the MAIL FROM address and the From header.",
            },
            from_name: {
              label: "Sender name",
            },
            to_addresses: {
              label: "Default recipients",
              description:
                "Separate multiple email addresses with commas or new lines. Test sends use these recipients, and rules can override them in the target.",
              targetLabel: "Recipient override",
              targetDescription:
                "Optional. Leave empty to use the provider default recipients.",
              addressLabel: "Recipients",
            },
            cc_addresses: {
              label: "Default CC",
              targetLabel: "CC override",
              addressLabel: "CC",
            },
            bcc_addresses: {
              label: "Default BCC",
              targetLabel: "BCC override",
              addressLabel: "BCC",
            },
            reply_to: {
              label: "Default reply-to",
              targetLabel: "Reply-to override",
              addressLabel: "Reply-to",
            },
            allow_invalid_tls: {
              label: "Allow invalid certificates",
              description:
                "Only recommended for self-hosted mail servers or self-signed certificate debugging. Keep it off in production.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            imap_host: {
              label: "IMAP host",
              description:
                "Optional, saved for inbound mailbox configuration. The current notification flow only uses SMTP and does not read IMAP.",
            },
            imap_port: {
              label: "IMAP port",
            },
            imap_security: {
              label: "IMAP encryption",
              options: {
                none: "No encryption",
              },
            },
            imap_username: {
              label: "IMAP username",
            },
            imap_password: {
              label: "IMAP password",
            },
            imap_mailbox: {
              label: "IMAP mailbox",
            },
            subject_prefix: {
              label: "Subject prefix",
              description: "Optional, for example [Production].",
              placeholder: "[Production]",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
            details: "Details:",
            actionLinks: "Action links:",
            severity: "Severity: {value}",
            eventId: "Event ID: {value}",
            occurredAt: "Occurred at: {value}",
          },
          errors: {
            invalidEmailAddress:
              "{field} contains an invalid email address: {value}",
            smtpConnectionClosed: "SMTP connection was closed",
            smtpReaderDisposed: "SMTP reader was disposed",
            invalidSmtpResponse: "Unable to parse SMTP response: {line}",
            smtpConnectionTimeout: "SMTP connection timed out",
            smtpTlsHandshakeTimeout: "SMTP TLS handshake timed out",
            smtpCommandFailed: "{message}: {code} {response}",
            unknownResponse: "Unknown response",
            authPlainUnsupported: "SMTP server does not support AUTH PLAIN",
            authLoginUnsupported: "SMTP server does not support AUTH LOGIN",
            unsupportedAuthMechanisms:
              "Unsupported SMTP auth mechanisms: {mechanisms}",
            authFailed: "SMTP authentication failed",
            usernameAuthFailed: "SMTP username authentication failed",
            passwordAuthFailed: "SMTP password authentication failed",
            dataStartFailed: "Failed to start SMTP DATA phase",
            submitFailed: "Failed to submit SMTP message",
            invalidFromAddress: "From address format is invalid",
            recipientRequired:
              "At least one recipient email address is required",
            handshakeFailed: "SMTP server greeting failed",
            ehloFailed: "SMTP EHLO failed",
            startTlsUnsupported:
              "SMTP server did not advertise STARTTLS support",
            startTlsFailed: "SMTP STARTTLS failed",
            ehloAfterTlsFailed: "SMTP EHLO after TLS upgrade failed",
            credentialsRequired: "SMTP username and password cannot be empty",
            noAuthMechanism:
              "SMTP server did not provide an available auth mechanism",
            mailFromFailed: "Failed to set SMTP sender",
            recipientSetFailed: "Failed to set SMTP recipient {recipient}",
            quitFailed: "SMTP quit failed",
            missingSmtpHost: "Missing SMTP host",
            deliveryFailed: "Email delivery failed",
          },
        },
        pushplus: {
          description:
            "Send notifications through the PushPlus standard API, with per-rule channel choices such as WeChat Official Account, App, and email.",
          fields: {
            server_url: {
              label: "Service URL",
              description: "Keep the official API URL unless needed.",
            },
            token: {
              description:
                "PushPlus user token or message token. Keep it secret.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            topic: {
              label: "Topic code",
              description:
                "Optional. Send messages to the specified topic. Leave empty to send to the token owner.",
            },
            template: {
              label: "Message template",
              description:
                "Markdown is used by default. Switch per target if plain text or HTML fits the channel better.",
              options: {
                txt: "Plain text",
              },
            },
            channel: {
              label: "Send channel",
              description:
                "Defaults to WeChat Official Account. Switch here if you configured other channels in PushPlus.",
              options: {
                wechat: "WeChat Official Account",
                webhook: "Third-party webhook",
                cp: "WeCom app",
                mail: "Email",
                sms: "SMS",
                voice: "Voice",
                extension: "Plugin / desktop app",
                clawbot: "WeChat ClawBot",
              },
            },
            option: {
              label: "Channel option",
              description:
                "Optional. Channels such as cp, webhook, and mail usually require a channel code configured in the PushPlus account center.",
            },
            to: {
              label: "Friend token / user ID",
              description:
                "Optional. Use a friend token for the WeChat Official Account channel or user IDs for WeCom app. Multiple recipients can follow the PushPlus format.",
              placeholder: "friend_token or user1,user2",
            },
            callback_url: {
              label: "Callback URL",
              description:
                "Optional. PushPlus calls this URL after asynchronous delivery completes.",
            },
            pre: {
              label: "Preprocess code",
              description:
                "Optional. Fill this only when your PushPlus account has the corresponding preprocessing logic configured.",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingToken: "Missing PushPlus token",
            requestFailed: "PushPlus request failed",
          },
        },
        wxpusher: {
          description:
            "Send notifications to specified UIDs or Topics through the WxPusher standard API. Empty rule targets inherit the provider defaults.",
          fields: {
            server_url: {
              label: "Service URL",
              description: "Keep the official service URL unless needed.",
            },
            app_token: {
              description:
                "AppToken for the WxPusher backend app. Keep it secret.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            uids: {
              label: "Default UID list",
              targetLabel: "UID list",
              description:
                "Optional. Test sends prefer these UIDs, and rule targets inherit them when empty.",
              targetDescription:
                "Optional. Override the provider default UID list. Leave empty to inherit the default.",
            },
            topic_ids: {
              label: "Default Topic",
              description:
                "Optional. Test sends prefer this Topic. Configure at least one default UID or Topic to verify the channel directly.",
              targetDescription:
                "Optional. Override the provider default Topic. Leave empty to inherit the default.",
            },
            url: {
              label: "Default message URL",
              targetLabel: "Message URL",
              description:
                "Optional. Rule targets inherit this jump URL when empty, and test sends use it too.",
              targetDescription:
                "Optional. Override the provider default jump URL. Leave empty to inherit the default.",
            },
            verify_pay_type: {
              label: "Default subscription verification",
              targetLabel: "Subscription verification",
              description:
                "Optional. Rule targets inherit this subscription verification policy when empty.",
              targetDescription:
                "Optional. Override the provider default subscription verification policy. Choose inherit to avoid a separate override.",
              options: {
                "0": "Do not verify",
                "1": "Paid subscribers only",
                "2": "Unsubscribed or expired users only",
                __inherit__: "Inherit provider default",
              },
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingAppToken: "Missing WxPusher AppToken",
            invalidTopicIds: "Invalid Topic ID format: {values}",
            recipientRequired:
              "WxPusher requires at least one UID or Topic ID. Configure it in provider defaults or override it in the rule target.",
            targetsFailed: "{failed}/{total} WxPusher targets failed",
            requestFailed: "WxPusher request failed",
          },
        },
        bark: {
          description:
            "Send APNs push notifications to iPhone through the official Bark service or a self-hosted Bark Server.",
          fields: {
            server_url: {
              label: "Service URL",
              description:
                "Keep the official online service URL unless you use a self-hosted Bark Server.",
            },
            device_key: {
              description:
                "Device Key copied from the Bark app. Multiple keys can be separated with commas.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            level: {
              label: "Notification level",
              description:
                "active is the default instant alert, timeSensitive can bypass Focus, and critical is a critical alert.",
            },
            group: {
              label: "Message group",
              description:
                "Optional. Same-group messages are grouped in the Bark client.",
            },
            sound: {
              label: "Sound",
              description:
                "Optional. Enter a system or custom sound name supported by Bark.",
            },
            url: {
              label: "Tap URL",
              description:
                "Optional. Open this link after tapping the notification. If empty, the first message action link is used.",
            },
            icon: {
              label: "Icon URL",
              description:
                "Optional. iOS 15 and later can display a custom icon.",
            },
            badge: {
              label: "Badge number",
              description: "Optional. Number shown on the Bark app icon badge.",
            },
            call: {
              label: "Repeated ringing",
              description:
                "When enabled, Bark rings continuously for about 30 seconds.",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingDeviceKey: "Missing Bark Device Key",
            requestFailed: "Bark request failed",
            pushFailed: "Bark push failed",
            targetsFailed: "{failed}/{total} Bark targets failed",
          },
        },
        serverchan: {
          label: "ServerChan",
          description:
            "Send Markdown notifications through ServerChan Turbo and reuse the default receiving channels configured on the website.",
          fields: {
            server_url: {
              label: "Service URL",
              description: "Keep the official API URL unless needed.",
            },
            sendkey: {
              description:
                "SendKey provided by ServerChan Turbo. Keep it secret.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            channel: {
              label: "Message channel",
              description:
                "Optional. Dynamically choose up to two channels for this push, separated by |, such as 9|66.",
            },
            openid: {
              description:
                "Optional. Test accounts use openid, and WeCom app messages use recipient UIDs. For multiple values, follow the ServerChan documentation format.",
              placeholder: "openid1,openid2 or uid1|uid2",
            },
            short: {
              label: "Card summary",
              description:
                "Optional. Short summary for the message card, up to 64 characters. Leave empty for ServerChan to derive it from the body.",
              placeholder: "Login anomaly, handle soon",
            },
            noip: {
              label: "Hide caller IP",
              description:
                "When enabled, this push will not show the caller source IP.",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingSendKey: "Missing ServerChan SendKey",
            requestReturned: "ServerChan returned HTTP {status}",
            requestFailed: "ServerChan request failed",
          },
        },
        dingtalk: {
          label: "DingTalk bot",
          description:
            "Send Markdown notifications to group chats through a DingTalk bot Webhook, with optional signature verification.",
          fields: {
            webhook_url: {
              description: "Full Webhook URL generated by the DingTalk bot.",
            },
            secret: {
              label: "Signing secret",
              description:
                "Optional. If the bot enabled signing, enter the SEC-prefixed secret shown on the security settings page.",
            },
            keyword_prefix: {
              label: "Keyword prefix",
              description:
                "Optional. If the bot enabled custom keyword validation, set a fixed keyword here. It is automatically prepended to the title.",
              placeholder: "Monitoring alert",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            at_mobiles: {
              label: "@ mobile numbers",
              description:
                "Optional. Separate multiple values with commas or new lines. Values must be member mobile numbers in the group.",
            },
            at_user_ids: {
              label: "@ user IDs",
              description:
                "Optional. Separate multiple values with commas or new lines. @userId tokens are appended to the body automatically.",
            },
            is_at_all: {
              label: "@ everyone",
              description:
                "When enabled, the request includes isAtAll and appends @everyone to the body.",
            },
          },
          mentionAll: "@everyone",
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingWebhookUrl: "Missing DingTalk Webhook URL",
            requestReturned: "DingTalk returned HTTP {status}",
            requestFailed: "DingTalk request failed",
          },
        },
        feishu: {
          label: "Feishu bot",
          description:
            "Send rich post notifications to group chats through a Feishu bot Webhook, with optional signature verification.",
          fields: {
            webhook_url: {
              description: "Full Webhook URL generated by the Feishu bot.",
            },
            secret: {
              label: "Signing secret",
              description:
                "Optional. If the bot enabled signature verification, enter the secret copied from security settings.",
            },
            keyword_prefix: {
              label: "Keyword prefix",
              description:
                "Optional. If the bot enabled custom keyword validation, set a fixed keyword here. It is automatically prepended to the title.",
              placeholder: "App alert",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            mention_user_ids: {
              label: "@ user IDs",
              description:
                "Optional. Separate multiple values with commas or new lines. Supports all. In external groups, mentioning a single user only supports Open ID.",
            },
          },
          mentionAll: "Everyone",
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingWebhookUrl: "Missing Feishu Webhook URL",
            requestReturned: "Feishu returned HTTP {status}",
            requestFailed: "Feishu request failed",
          },
        },
        webhook: {
          description:
            "Send standard notification JSON to any endpoint that supports HTTP JSON.",
          fields: {
            url: {
              description:
                "Target address that receives standard notification JSON.",
            },
            method: {
              label: "Request method",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            shared_secret: {
              label: "Shared secret",
              description:
                "Optional. When set, it is sent through the X-Fn-Knock-Signature request header.",
            },
            endpoint_path: {
              label: "Additional path",
              description:
                "Optional. Appended to the base Webhook URL before sending.",
            },
            extra_headers_json: {
              label: "Extra headers JSON",
              description: 'Optional, for example {"X-Env":"prod"}.',
            },
            extra_body_json: {
              label: "Extra body JSON",
              description: "Optional. Attached to payload.extra_body.",
            },
          },
          errors: {
            missingUrl: "Missing Webhook URL",
            requestReturned: "Webhook returned HTTP {status}",
            requestFailed: "Webhook request failed",
          },
        },
        magicpush: {
          label: "MagicPush",
          description:
            "Push notifications to configured channels through a self-hosted MagicPush service, supporting standard push and MagicPush inbound mode.",
          fields: {
            server_url: {
              label: "Base API URL",
              description:
                "Enter the MagicPush service root, such as http://192.168.31.98:3000. URLs that already include /api/push or /api/inbound are also accepted.",
            },
            delivery_mode: {
              label: "Delivery mode",
              description:
                "Standard push sends to /api/push. Inbound mode sends to /api/inbound/:token and lets MagicPush inbound rules map fields.",
              options: {
                push: "Standard push",
                inbound: "Inbound config",
              },
            },
            token: {
              description:
                "MagicPush API token. Standard push sends it as Authorization: Bearer; inbound mode appends it to /api/inbound/:token.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingBaseUrl: "Missing MagicPush base API URL",
            missingToken: "Missing MagicPush token",
            invalidBaseUrl: "Invalid MagicPush base API URL",
            requestReturned: "MagicPush returned HTTP {status}",
            requestFailed: "MagicPush request failed",
          },
        },
        telegram: {
          description:
            "Send text notifications through Telegram Bot API to a specified chat or channel, with inline action buttons.",
          fields: {
            server_url: {
              label: "Bot API URL",
              description:
                "Keep the official Bot API by default. If network access to the official endpoint is unavailable, use https://tgapi.fnknock.cn as a relay. If you run a self-hosted Local Bot API Server, enter its root URL.",
            },
            bot_token: {
              description:
                "Bot Token obtained after creating a bot through @BotFather.",
            },
            chat_id: {
              description:
                "Target chat ID or channel username, such as @channelusername. You can message @UserIdzhBot first to get a Chat ID; test sends use this target too.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            message_thread_id: {
              description:
                "Optional. Topic ID (message_thread_id) for sending to a group topic.",
            },
            disable_notification: {
              label: "Silent send",
              description:
                "When enabled, Telegram delivers silently without notification sound.",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingBotToken: "Missing Telegram Bot Token",
            missingChatId: "Missing Telegram Chat ID",
            requestReturned: "Telegram returned HTTP {status}",
            requestFailed: "Telegram request failed",
          },
        },
        wecom: {
          label: "WeCom group bot",
          description:
            "Send text or markdown notifications to a specified group chat through WeCom group Webhook.",
          fields: {
            webhook_url: {
              description:
                "Full Webhook URL generated on the WeCom message push page. Keep it secret.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
            mentioned_list: {
              label: "Mention member UserIDs",
              description:
                "Optional. Separate multiple values with commas or new lines. Supports @all.",
            },
            mentioned_mobile_list: {
              label: "Mention mobile numbers",
              description:
                "Optional. Separate multiple values with commas or new lines. Supports @all.",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingWebhookUrl: "Missing WeCom Webhook URL",
            requestReturned: "WeCom returned HTTP {status}",
            requestFailed: "WeCom request failed",
          },
        },
        pushdeer: {
          description:
            "Send Markdown notifications to bound devices through PushDeer official online service or a self-hosted service.",
          fields: {
            server_url: {
              label: "Service URL",
              description:
                "Keep the official online service URL unless you use a self-hosted PushDeer service.",
            },
            pushkey: {
              description:
                "PushKey generated in the PushDeer client. Multiple keys can be separated with commas.",
            },
            timeout_seconds: {
              label: "Timeout seconds",
            },
          },
          message: {
            fallbackTitle: "fn-knock notification",
          },
          errors: {
            missingPushKey: "Missing PushDeer PushKey",
            requestReturned: "PushDeer returned HTTP {status}",
            apiReturnedCode: "PushDeer API returned code {code}",
            requestFailed: "PushDeer request failed",
          },
        },
      },
    },
    routes: {
      createProviderFailed: "Failed to create notification provider",
      testProviderFailed: "Failed to test notification provider",
      getProviderFailed: "Failed to get notification provider",
      updateProviderFailed: "Failed to update notification provider",
      deleteProviderFailed: "Failed to delete notification provider",
      createRuleFailed: "Failed to create notification rule",
      updateRuleFailed: "Failed to update notification rule",
      deleteRuleFailed: "Failed to delete notification rule",
      unsupportedDeliveryStatus: "Unsupported delivery status",
      clearDeliveriesFailed: "Failed to clear delivery records",
    },
    service: {
      unnamed: "Unnamed",
      invalidJson: "{field} must be valid JSON",
      invalidSelectValue: "{field} has an invalid value",
      fieldRequired: "{field} cannot be empty",
      testMessage: {
        title: "Test notification",
        summary:
          "The notification channel is configured correctly and a test message was triggered successfully.",
        bodyText:
          "This is a test notification sent by fn-knock to verify provider connectivity, structured copy, and display behavior.",
        bodyMarkdown:
          "**Connectivity check passed.**\n\nThis is a test notification sent by fn-knock to verify provider connectivity, structured copy, and display behavior.",
        sendType: "Send type",
        providerTest: "Provider test",
        sentAt: "Sent at",
      },
      providerNotFound: "Notification provider does not exist",
      unsupportedProviderType: "Unsupported notification provider type",
      providerDefinitionMissing:
        "Notification provider definition does not exist",
      providerReferencedByRule:
        'This provider is still referenced by rule "{rule}"',
      testSendFailed: "Test send failed",
      testSendSuccess: "Test send succeeded",
      providerTypeMismatch:
        "Provider type does not match the existing configuration",
      providerTestName: "{provider} test",
      ruleProviderMissing:
        "The rule references a notification provider that does not exist",
      invalidTemplateOverrideMode: "Invalid target template override mode",
      unsupportedEventType: "Unsupported system event type",
      invalidGroupBy: "Invalid aggregation dimension",
      invalidMessageTemplateMode: "Invalid message template mode",
      invalidEventLevelFilter: "Invalid event level filter",
      invalidEventSourceFilter: "Invalid event source filter",
      targetRequired: "At least one notification target is required",
      duplicateEventRule:
        "A notification rule already exists for this event. Delete the original rule first.",
      ruleNotFound: "Notification rule does not exist",
      deletedProvider: "Deleted provider",
    },
  },
};
