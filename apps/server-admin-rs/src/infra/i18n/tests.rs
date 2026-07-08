use super::*;

const RUNTIME_I18N_KEYS: &[&str] = &[
    "server.acmeRoutes.domainsInvalid",
    "server.acmeRoutes.dnsTypeRequired",
    "server.acmeRoutes.unsupportedDnsProvider",
    "server.acmeRoutes.missingDnsCredentials",
    "server.acmeRoutes.installingRetryLater",
    "server.acmeRoutes.installFirst",
    "server.acmeRoutes.multipleApplicationsUseNewApi",
    "server.acmeRoutes.applicationNotFound",
    "server.acmeRoutes.notFound",
    "server.acmeRoutes.installingCannotDelete",
    "server.acmeRoutes.installingCannotSwitchCa",
    "server.acmeRoutes.noMatchingIssuedCertificate",
    "server.acmeRoutes.success",
    "server.acmeRoutes.dns01Only",
    "server.acmeRoutes.certNotFound",
    "server.acmeRoutes.certOrKeyInvalid",
    "server.acme.alreadyInstalled",
    "server.acme.installInProgress",
    "server.acme.installSubmitted",
    "server.acme.issueSucceeded",
    "server.acmeService.waiting",
    "server.acmeService.ready",
    "server.acmeService.sendSignalFailed",
    "server.acmeService.setDefaultCaFailed",
    "server.acmeService.registerAccountFailed",
    "server.acmeService.bundledZipMissing",
    "server.acmeService.extractingBundled",
    "server.acmeService.unzipFailed",
    "server.acmeService.extractedAcmeMissing",
    "server.acmeService.writingDataDir",
    "server.acmeService.writtenAcmeMissing",
    "server.acmeService.checkInstallFailed",
    "server.acmeService.notInstalled",
    "server.acmeService.initializingBundled",
    "server.acmeService.registeringAccount",
    "server.acmeService.savingDefaultCa",
    "server.acmeService.installSuccess",
    "server.acmeService.installFailed",
    "server.acmeService.installFirst",
    "server.acmeService.installingCannotDelete",
    "server.acmeService.deleted",
    "server.acmeService.deleteFailed",
    "server.acmeService.domainsRequired",
    "server.acmeService.dnsTypeRequired",
    "server.acmeService.issueFailed",
    "server.acmeJobRunner.manualStop",
    "server.acmeJobRunner.lockMessages.manualRequest",
    "server.acmeJobRunner.lockMessages.autoRenew",
    "server.acmeJobRunner.flowFailed",
    "server.acmeJobRunner.activeTaskRunning",
    "server.acmeJobRunner.applicationChangedSkipped",
    "server.acmeJobRunner.issuedButApplicationChanged",
    "server.acmeJobRunner.issuedButCertReadFailed",
    "server.acmeJobRunner.clearedDomainWorkingState",
    "server.acmeJobRunner.clearDomainWorkingStateFailed",
    "server.acmeJobRunner.linkedLibrarySyncedGateway",
    "server.acmeJobRunner.linkedLibraryUpdated",
    "server.acmeJobRunner.addedToLibraryAndSyncedGateway",
    "server.acmeJobRunner.addedToLibrary",
    "server.acmeJobRunner.addToLibraryFailed",
    "server.acmeJobRunner.stoppedIgnoredProcessError",
    "server.store.acme.domainRequired",
    "server.store.acme.domainsRequired",
    "server.store.acme.dnsProviderRequired",
    "server.store.acme.primaryDomainDuplicated",
    "server.store.acme.applicationNotFound",
    "server.store.acme.noMatchingIssuedCertificate",
    "server.store.ssl.certNotFound",
    "server.store.ssl.certOrKeyInvalid",
    "server.acmeDnsProviders.groups.common",
    "server.acmeDnsProviders.groups.domestic",
    "server.acmeDnsProviders.groups.international",
    "server.acmeDnsProviders.groups.selfHostedAdvanced",
    "server.acmeDnsProviders.credentialSchemes.default",
    "server.acmeDnsProviders.fields.accountEmail",
    "server.acmeDnsProviders.labels.aliyun",
    "server.acmeDnsProviders.labels.tencentCloudDnspod",
    "server.acmeDnsProviders.labels.huaweiCloudDns",
    "server.acmeDnsProviders.labels.jdCloudDns",
    "server.acmeDnsProviders.labels.westCn",
    "server.acmeDnsProviders.requirements.optionalSuffix",
    "server.acmeDnsProviders.requirements.orSeparator",
    "server.acmePatches.duckdns.scriptMissing",
    "server.acmePatches.duckdns.proxyApplied",
    "server.subdomainMode.recommendationMissingBase",
    "server.subdomainMode.recommendationWildcardSummary",
    "server.subdomainMode.authOutOfRootWarning",
    "server.subdomainMode.recommendationSingleHostSummary",
    "server.subdomainMode.wildcardSuggestion",
    "server.subdomainMode.configureRootOrAuth",
    "server.subdomainMode.authMissingWarning",
    "server.subdomainMode.uncoveredHostMappingsWarning",
    "server.gatewayVisibility.customCidrInvalid",
    "server.gatewayVisibility.emptyEnabledConfig",
    "server.gatewayVisibility.syncFailed",
    "server.gatewayProxyHeaders.runTypes.direct",
    "server.gatewayProxyHeaders.runTypes.reverseProxy",
    "server.gatewayProxyHeaders.runTypes.subdomain",
    "server.gatewayProxyHeaders.unavailableReason",
    "server.gatewayProxyHeaders.syncFailed",
    "server.gatewayHostResponse.runTypes.direct",
    "server.gatewayHostResponse.runTypes.reverseProxy",
    "server.gatewayHostResponse.runTypes.subdomain",
    "server.gatewayHostResponse.unavailableReason",
    "server.gatewayHostResponse.editSubdomainOnly",
    "server.gatewayHostResponse.updateFailedRolledBack",
    "server.gatewayHostResponse.restoreConfigFailed",
    "server.gatewayHostResponse.restoreRuntimeFailed",
    "server.gatewayHostResponse.restoreGatewayRuntimeFailed",
    "server.admin.rollback.failed",
    "server.admin.rollback.restoreVisibilityConfigFailed",
    "server.admin.rollback.restoreVisibilityRuntimeFailed",
    "server.admin.rollback.restoreGatewayVisibilityFailed",
    "server.admin.rollback.restoreProxyHeadersConfigFailed",
    "server.admin.rollback.restoreProxyHeadersRuntimeFailed",
    "server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed",
    "server.admin.gatewayVisibility.updateFailedRolledBack",
    "server.admin.gatewayProxyHeaders.subdomainOnly",
    "server.admin.gatewayProxyHeaders.updateFailedRolledBack",
    "server.admin.hostMappings.bookmarkFolderForRoot",
    "server.admin.hostMappings.bookmarkFolderDefault",
    "server.whitelist.regionAddFailed",
    "server.whitelist.regionRequired",
    "server.whitelist.regionEmpty",
    "server.whitelist.regionNotFound",
    "server.scanDiscovery.selectAtLeastOneCidr",
    "server.scanDiscovery.scanJobNotFound",
    "server.dockerAdminPanel.resetHelp",
    "server.dockerAdminPanel.resetCleared",
    "server.dockerAdminPanel.resetNextVisit",
    "server.dockerAdminPanel.resetFailed",
];

#[test]
fn normalizes_locale_aliases() {
    assert_eq!(normalize_locale(Some("zh-HK")), "zh-Hant");
    assert_eq!(normalize_locale(Some("en-US")), "en");
    assert_eq!(normalize_locale(Some("unknown")), "zh-CN");
}

#[test]
fn translates_and_interpolates() {
    let translator = Translator::new("en");
    assert_eq!(
        translator.t_params(
            "server.store.acme.primaryDomainDuplicated",
            &[("primaryDomain", "example.com".to_string())]
        ),
        "Primary domain example.com already exists in another request item"
    );
}

#[test]
fn generated_catalog_wins_for_non_clock_runtime_messages() {
    let translator = Translator::new("zh-Hant");
    assert_eq!(translator.t("server.acmeRoutes.certNotFound"), "證書不存在");
}

#[test]
fn generated_catalog_serves_non_static_messages() {
    let translator = Translator::new("en");
    assert_eq!(translator.t("server.apiPathNotFound"), "API path not found");
}

#[test]
fn locale_catalog_loader_extracts_requested_language_branch() {
    assert_eq!(
        load_locale_catalog("zh-Hant").message("acmeRoutes.certNotFound"),
        Some("證書不存在")
    );
    assert_eq!(
        load_locale_catalog("zh-Hant").message("apiPathNotFound"),
        Some("接口不存在")
    );
    assert!(load_locale_catalog("missing").entries.is_empty());
}

#[test]
fn locale_catalog_borrows_static_message_values() {
    let catalog = load_locale_catalog("en");
    let borrowed = catalog
        .entries
        .iter()
        .filter(|(_, message)| matches!(message, std::borrow::Cow::Borrowed(_)))
        .count();
    assert!(borrowed > catalog.entries.len() / 2);
}

#[test]
fn system_clock_messages_use_static_fast_path() {
    let translator = Translator::new("ko-KR");
    assert_eq!(
        translator.t_params(
            "server.systemClock.duration.seconds",
            &[("seconds", "5".to_string())]
        ),
        "5초"
    );
}

#[test]
fn runtime_messages_are_explicit_for_supported_locales() {
    for key in RUNTIME_I18N_KEYS {
        assert!(zh_cn_message(key).is_some(), "missing zh-CN key {key}");
        assert!(en_message(key).is_some(), "missing en key {key}");
        assert!(zh_hant_message(key).is_some(), "missing zh-Hant key {key}");
        assert!(ko_kr_message(key).is_some(), "missing ko-KR key {key}");
        assert!(ja_jp_message(key).is_some(), "missing ja-JP key {key}");
    }
}
