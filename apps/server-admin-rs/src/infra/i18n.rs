use std::{borrow::Cow, sync::OnceLock};

use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

use crate::state::AppState;

pub const DEFAULT_LOCALE: &str = "zh-CN";

#[derive(Clone, Debug)]
pub struct Translator {
    locale: String,
}

impl Translator {
    pub fn new(locale: impl AsRef<str>) -> Self {
        Self {
            locale: normalize_locale(Some(locale.as_ref())).to_string(),
        }
    }

    pub async fn from_state(state: &AppState) -> Self {
        let locale = state.storage.store.locale().await.ok().and_then(|value| {
            value
                .get("default_locale")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        Self::new(locale.unwrap_or_else(|| DEFAULT_LOCALE.to_string()))
    }

    pub fn t(&self, key: &str) -> String {
        translate(&self.locale, key, &[])
    }

    pub fn t_params(&self, key: &str, params: &[(&str, String)]) -> String {
        translate(&self.locale, key, params)
    }

    pub fn t_with_fallback(&self, key: &str, fallback: &str) -> String {
        let translated = self.t(key);
        if translated == key {
            fallback.to_string()
        } else {
            translated
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}

pub fn normalize_locale(value: Option<&str>) -> &'static str {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return DEFAULT_LOCALE;
    };
    match raw {
        "zh-CN" | "zh-Hant" | "en" | "ko-KR" | "ja-JP" => return raw_to_static(raw),
        _ => {}
    }
    let lower = raw.replace('_', "-").to_ascii_lowercase();
    match lower.as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh-hans-cn" | "zh-sg" | "zh-my" => "zh-CN",
        "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant" | "zh-hant-tw" => "zh-Hant",
        "en" | "en-us" | "en-gb" => "en",
        "ko" | "ko-kr" => "ko-KR",
        "ja" | "ja-jp" => "ja-JP",
        value if value.starts_with("en-") => "en",
        value if value.starts_with("ko-") => "ko-KR",
        value if value.starts_with("ja-") => "ja-JP",
        value if value.starts_with("zh-hant") => "zh-Hant",
        value if value.starts_with("zh-") => "zh-CN",
        _ => DEFAULT_LOCALE,
    }
}

fn raw_to_static(value: &str) -> &'static str {
    match value {
        "zh-Hant" => "zh-Hant",
        "en" => "en",
        "ko-KR" => "ko-KR",
        "ja-JP" => "ja-JP",
        _ => "zh-CN",
    }
}

fn translate(locale: &str, key: &str, params: &[(&str, String)]) -> String {
    let template = message_template(locale, key).unwrap_or(key);
    interpolate(template, params)
}

fn message_template(locale: &str, key: &str) -> Option<&'static str> {
    if key.starts_with("server.systemClock.") {
        return message_for(locale, key)
            .or_else(|| generated_message_for(locale, key))
            .or_else(|| message_for(DEFAULT_LOCALE, key))
            .or_else(|| generated_message_for(DEFAULT_LOCALE, key));
    }

    generated_message_for(locale, key)
        .or_else(|| generated_message_for(DEFAULT_LOCALE, key))
        .or_else(|| message_for(locale, key))
        .or_else(|| message_for(DEFAULT_LOCALE, key))
}

fn interpolate(template: &str, params: &[(&str, String)]) -> String {
    if params.is_empty() {
        return template.to_string();
    }

    let mut output = template.to_string();
    for (key, value) in params {
        output = output.replace(&format!("{{{key}}}"), value);
    }
    output
}

fn message_for(locale: &str, key: &str) -> Option<&'static str> {
    match locale {
        "en" => en_message(key),
        "zh-Hant" => zh_hant_message(key).or_else(|| zh_cn_message(key)),
        "ko-KR" => ko_kr_message(key).or_else(|| en_message(key)),
        "ja-JP" => ja_jp_message(key).or_else(|| en_message(key)),
        _ => zh_cn_message(key),
    }
}

fn generated_message_for(locale: &str, key: &str) -> Option<&'static str> {
    let path = key.strip_prefix("server.").unwrap_or(key);
    let catalog = server_i18n_locale_catalog(locale);
    catalog.message(path).or_else(|| {
        path.strip_prefix("store.").and_then(|suffix| {
            let legacy_path = format!("redis.{suffix}");
            catalog.message(&legacy_path)
        })
    })
}

fn server_i18n_locale_catalog(locale: &str) -> &'static LocaleCatalog {
    static ZH_CN: OnceLock<LocaleCatalog> = OnceLock::new();
    static ZH_HANT: OnceLock<LocaleCatalog> = OnceLock::new();
    static EN: OnceLock<LocaleCatalog> = OnceLock::new();
    static KO_KR: OnceLock<LocaleCatalog> = OnceLock::new();
    static JA_JP: OnceLock<LocaleCatalog> = OnceLock::new();

    match normalize_locale(Some(locale)) {
        "zh-Hant" => locale_catalog(&ZH_HANT, "zh-Hant"),
        "en" => locale_catalog(&EN, "en"),
        "ko-KR" => locale_catalog(&KO_KR, "ko-KR"),
        "ja-JP" => locale_catalog(&JA_JP, "ja-JP"),
        _ => locale_catalog(&ZH_CN, DEFAULT_LOCALE),
    }
}

fn locale_catalog(
    cell: &'static OnceLock<LocaleCatalog>,
    locale: &'static str,
) -> &'static LocaleCatalog {
    let was_initialized = cell.get().is_some();
    let value = cell.get_or_init(|| load_locale_catalog(locale));
    if !was_initialized {
        super::memory::trim_allocated_memory();
    }
    value
}

#[derive(Debug, Default)]
struct LocaleCatalog {
    entries: Box<[(Box<str>, CatalogMessage)]>,
}

type CatalogMessage = Cow<'static, str>;
const SERVER_I18N_LOCALE_ENTRY_CAPACITY: usize = 2111;

impl LocaleCatalog {
    fn message(&self, path: &str) -> Option<&str> {
        self.entries
            .binary_search_by(|(key, _)| key.as_ref().cmp(path))
            .ok()
            .map(|index| self.entries[index].1.as_ref())
    }
}

fn load_locale_catalog(locale: &str) -> LocaleCatalog {
    let mut deserializer = serde_json::Deserializer::from_str(include_str!("server_i18n.json"));
    LocaleCatalogSeed { locale }
        .deserialize(&mut deserializer)
        .expect("server_i18n.json must be valid JSON")
}

struct LocaleCatalogSeed<'a> {
    locale: &'a str,
}

impl DeserializeSeed<'static> for LocaleCatalogSeed<'_> {
    type Value = LocaleCatalog;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'static>,
    {
        deserializer.deserialize_map(LocaleCatalogVisitor {
            locale: self.locale,
        })
    }
}

struct LocaleCatalogVisitor<'a> {
    locale: &'a str,
}

impl Visitor<'static> for LocaleCatalogVisitor<'_> {
    type Value = LocaleCatalog;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("server i18n top-level locale object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'static>,
    {
        let mut entries = None;
        while let Some(key) = map.next_key::<String>()? {
            if entries.is_none() && key == self.locale {
                let mut locale_entries = Vec::with_capacity(SERVER_I18N_LOCALE_ENTRY_CAPACITY);
                map.next_value_seed(LocaleEntriesSeed {
                    prefix: String::new(),
                    entries: &mut locale_entries,
                })?;
                entries = Some(locale_entries);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        let mut entries = entries.unwrap_or_default();
        entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        Ok(LocaleCatalog {
            entries: entries.into_boxed_slice(),
        })
    }
}

struct LocaleEntriesSeed<'a> {
    prefix: String,
    entries: &'a mut Vec<(Box<str>, CatalogMessage)>,
}

impl DeserializeSeed<'static> for LocaleEntriesSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'static>,
    {
        deserializer.deserialize_any(LocaleEntriesVisitor {
            prefix: self.prefix,
            entries: self.entries,
        })
    }
}

struct LocaleEntriesVisitor<'a> {
    prefix: String,
    entries: &'a mut Vec<(Box<str>, CatalogMessage)>,
}

impl Visitor<'static> for LocaleEntriesVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("server i18n nested message object or string")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'static>,
    {
        while let Some(key) = map.next_key::<String>()? {
            let path = if self.prefix.is_empty() {
                key
            } else {
                format!("{}.{key}", self.prefix)
            };
            map.next_value_seed(LocaleEntriesSeed {
                prefix: path,
                entries: &mut *self.entries,
            })?;
        }
        Ok(())
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if !self.prefix.is_empty() {
            self.entries.push((
                self.prefix.into_boxed_str(),
                CatalogMessage::Owned(value.to_string()),
            ));
        }
        Ok(())
    }

    fn visit_borrowed_str<E>(self, value: &'static str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if !self.prefix.is_empty() {
            self.entries.push((
                self.prefix.into_boxed_str(),
                CatalogMessage::Borrowed(value),
            ));
        }
        Ok(())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if !self.prefix.is_empty() {
            self.entries
                .push((self.prefix.into_boxed_str(), CatalogMessage::Owned(value)));
        }
        Ok(())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'static>,
    {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(())
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }
}

fn zh_cn_message(key: &str) -> Option<&'static str> {
    Some(match key {
        "auth.autoIpGrantComment" => "登录后自动授权",
        "server.systemClock.unknown" => "未知",
        "server.systemClock.actionSeparator" => "；",
        "server.systemClock.listSeparator" => "，",
        "server.systemClock.duration.seconds" => "{seconds} 秒",
        "server.systemClock.duration.minutes" => "{minutes} 分钟",
        "server.systemClock.duration.minutesSeconds" => "{minutes} 分 {seconds} 秒",
        "server.systemClock.networkTimeUnavailable" => "未能从网络获取标准时间",
        "server.systemClock.sourceFetchFailed" => "从 {source} 获取时间失败",
        "server.systemClock.missingDateHeader" => "{source} 未返回可用的 Date 响应头",
        "server.systemClock.invalidDateHeader" => "{source} 返回了无法解析的时间",
        "server.systemClock.issues.timezone.title" => "系统时区不是北京时间",
        "server.systemClock.issues.timezone.message" => {
            "当前系统时区为 {timezone}，应设置为 {expected}。"
        }
        "server.systemClock.issues.timeMismatch.title" => "系统时间与联网校验结果不一致",
        "server.systemClock.issues.timeMismatch.message" => {
            "当前系统时间与联网校验结果相差约 {drift}。"
        }
        "server.systemClock.statusRefreshed" => "系统时间状态已刷新",
        "server.systemClock.timezoneSet" => "已设置系统时区为 {timezone}",
        "server.systemClock.missingZoneinfoFile" => "系统缺少时区文件 {path}",
        "server.systemClock.timezoneWritten" => "已写入系统时区 {timezone}",
        "server.systemClock.clockAdjusted" => "已校准系统时间",
        "server.systemClock.ntpEnabled" => "已启用 NTP 自动校时",
        "server.systemClock.serviceRestarted" => "已重启 {service} 服务",
        "server.acmeRoutes.domainsInvalid" => "域名列表不能为空或格式无效",
        "server.acmeRoutes.dnsTypeRequired" => "缺少 DNS 验证类型",
        "server.acmeRoutes.unsupportedDnsProvider" => "不支持的 DNS 服务商",
        "server.acmeRoutes.missingDnsCredentials" => {
            "缺少 DNS API 凭据，请填写以下任一方案: {requirements}"
        }
        "server.acmeRoutes.installingRetryLater" => "acme.sh 安装中，请稍后再试",
        "server.acmeRoutes.installFirst" => "请先安装 acme.sh",
        "server.acmeRoutes.multipleApplicationsUseNewApi" => {
            "当前已存在多个申请项，请使用新接口管理 ACME 申请项"
        }
        "server.acmeRoutes.applicationNotFound" => "申请项不存在",
        "server.acmeRoutes.notFound" => "未找到",
        "server.acmeRoutes.installingCannotDelete" => "acme.sh 安装中，无法删除",
        "server.acmeRoutes.installingCannotSwitchCa" => "acme.sh 安装中，暂时无法切换证书颁发机构",
        "server.acmeRoutes.noMatchingIssuedCertificate" => {
            "当前申请项还没有与域名配置匹配的已签发证书"
        }
        "server.acmeRoutes.success" => "成功",
        "server.acmeRoutes.dns01Only" => "仅支持 DNS-01 验证方式",
        "server.acmeRoutes.certNotFound" => "证书不存在",
        "server.acmeRoutes.certOrKeyInvalid" => "证书或私钥无效",
        "server.acme.alreadyInstalled" => "acme.sh 已经安装过了",
        "server.acme.installInProgress" => "安装任务正在进行中",
        "server.acme.installSubmitted" => "安装任务已提交",
        "server.acme.issueSucceeded" => "证书签发成功",
        "server.acmeService.waiting" => "等待操作",
        "server.acmeService.ready" => "acme.sh 已就绪",
        "server.acmeService.sendSignalFailed" => "发送 {signal} 到 {target} 失败: {detail}",
        "server.acmeService.setDefaultCaFailed" => {
            "设置默认证书颁发机构失败（退出码: {code}）{brief}"
        }
        "server.acmeService.registerAccountFailed" => "注册 ACME 账号失败（退出码: {code}）{brief}",
        "server.acmeService.bundledZipMissing" => "未找到内置 acmesh.zip 资源",
        "server.acmeService.extractingBundled" => "正在解压内置 acme.sh 资源...",
        "server.acmeService.unzipFailed" => "解压失败，退出码: {code}",
        "server.acmeService.extractedAcmeMissing" => "解压成功但未找到 acme.sh",
        "server.acmeService.writingDataDir" => "正在写入数据目录...",
        "server.acmeService.writtenAcmeMissing" => "写入后未找到 acme.sh",
        "server.acmeService.checkInstallFailed" => "检查安装状态失败: {detail}",
        "server.acmeService.notInstalled" => "acme.sh 未安装",
        "server.acmeService.initializingBundled" => "正在初始化内置 acme.sh...",
        "server.acmeService.registeringAccount" => "正在注册 ACME 账号...",
        "server.acmeService.savingDefaultCa" => "正在保存默认证书颁发机构...",
        "server.acmeService.installSuccess" => "安装成功，账号邮箱: {email}",
        "server.acmeService.installFailed" => "安装失败: {detail}",
        "server.acmeService.installFirst" => "请先安装 acme.sh",
        "server.acmeService.installingCannotDelete" => "acme.sh 正在安装中，无法删除",
        "server.acmeService.deleted" => "acme.sh 已删除",
        "server.acmeService.deleteFailed" => "删除失败: {detail}",
        "server.acmeService.domainsRequired" => "域名列表不能为空",
        "server.acmeService.dnsTypeRequired" => "缺少 DNS 验证类型",
        "server.acmeService.issueFailed" => "证书签发失败（退出码: {code}）{brief}",
        "server.acmeJobRunner.manualStop" => "ACME 任务已由用户手动停止",
        "server.acmeJobRunner.lockMessages.manualRequest" => "正在申请证书",
        "server.acmeJobRunner.lockMessages.autoRenew" => "正在自动续期证书",
        "server.acmeJobRunner.flowFailed" => "证书申请流程失败: {message}",
        "server.acmeJobRunner.activeTaskRunning" => "当前已有 ACME 任务正在执行，请稍后再试",
        "server.acmeJobRunner.applicationChangedSkipped" => {
            "申请项域名已在执行期间发生变化，已跳过写入旧证书，请重新发起申请"
        }
        "server.acmeJobRunner.issuedButApplicationChanged" => {
            "证书签发成功，但由于申请项域名已变更，未写入当前申请项"
        }
        "server.acmeJobRunner.issuedButCertReadFailed" => {
            "证书签发成功，但读取证书文件失败（请稍后重试或检查 acme.sh 目录）"
        }
        "server.acmeJobRunner.clearedDomainWorkingState" => {
            "已清理 acme.sh 域名工作目录，证书列表与续期由系统任务统一管理"
        }
        "server.acmeJobRunner.clearDomainWorkingStateFailed" => {
            "证书已保存，但清理 acme.sh 域名状态失败: {message}"
        }
        "server.acmeJobRunner.linkedLibrarySyncedGateway" => {
            "已同步已关联的证书库条目，并刷新网关证书列表"
        }
        "server.acmeJobRunner.linkedLibraryUpdated" => "已更新已关联的证书库条目",
        "server.acmeJobRunner.addedToLibraryAndSyncedGateway" => {
            "证书签发成功后已自动加入证书库，并刷新网关证书列表"
        }
        "server.acmeJobRunner.addedToLibrary" => "证书签发成功后已自动加入证书库",
        "server.acmeJobRunner.addToLibraryFailed" => {
            "证书已签发并保存，但自动加入证书库失败: {message}"
        }
        "server.acmeJobRunner.stoppedIgnoredProcessError" => "任务已停止，已忽略进程退出后的错误",
        "server.store.acme.domainRequired" => "域名不能为空",
        "server.store.acme.domainsRequired" => "域名列表不能为空",
        "server.store.acme.dnsProviderRequired" => "DNS 服务商不能为空",
        "server.store.acme.primaryDomainDuplicated" => {
            "主域名 {primaryDomain} 已存在于其他申请项中"
        }
        "server.store.acme.applicationNotFound" => "申请项不存在",
        "server.store.acme.noMatchingIssuedCertificate" => {
            "当前申请项还没有与域名配置匹配的已签发证书"
        }
        "server.store.ssl.certNotFound" => "证书不存在",
        "server.store.ssl.certOrKeyInvalid" => "证书或私钥无效",
        "server.acmeDnsProviders.groups.common" => "常用",
        "server.acmeDnsProviders.groups.domestic" => "国内",
        "server.acmeDnsProviders.groups.international" => "国际",
        "server.acmeDnsProviders.groups.selfHostedAdvanced" => "自建/高级",
        "server.acmeDnsProviders.credentialSchemes.default" => "默认凭据",
        "server.acmeDnsProviders.fields.accountEmail" => "账户邮箱",
        "server.acmeDnsProviders.labels.aliyun" => "阿里云 DNS",
        "server.acmeDnsProviders.labels.tencentCloudDnspod" => "腾讯云 DNSPod (TencentCloud)",
        "server.acmeDnsProviders.labels.huaweiCloudDns" => "华为云 DNS",
        "server.acmeDnsProviders.labels.jdCloudDns" => "京东云 DNS",
        "server.acmeDnsProviders.labels.westCn" => "西部数码",
        "server.acmeDnsProviders.requirements.optionalSuffix" => "；可选 {keys}",
        "server.acmeDnsProviders.requirements.orSeparator" => "；或 ",
        "server.acmePatches.duckdns.scriptMissing" => "未找到 DuckDNS DNS API 脚本: {path}",
        "server.acmePatches.duckdns.proxyApplied" => "已将 DuckDNS API 从 {from} 切换为 {to}",
        "server.subdomainMode.recommendationMissingBase" => {
            "尚未配置根域名或鉴权服务，暂时无法生成推荐证书域名。"
        }
        "server.subdomainMode.recommendationWildcardSummary" => {
            "推荐申请 {rootDomain} 与 *.{rootDomain}，用于覆盖根域名、鉴权服务和同一父域下的业务子域。"
        }
        "server.subdomainMode.authOutOfRootWarning" => {
            "当前鉴权服务 {authHost} 不在根域名 {rootDomain} 下，已额外加入精确域名；请确认所选 DNS 服务商能够管理这些域名。"
        }
        "server.subdomainMode.recommendationSingleHostSummary" => {
            "尚未配置根域名，当前仅能推荐为鉴权服务 {authHost} 申请单域名证书。"
        }
        "server.subdomainMode.wildcardSuggestion" => {
            "如果后续要统一覆盖多个业务子域，建议先补充根域名后再申请 wildcard 证书。"
        }
        "server.subdomainMode.configureRootOrAuth" => {
            "请先在子域模式里配置根域名，或在 Host 映射中指定一条鉴权服务。"
        }
        "server.subdomainMode.authMissingWarning" => {
            "尚未指定鉴权服务，当前推荐结果只基于根域名推导。"
        }
        "server.subdomainMode.uncoveredHostMappingsWarning" => {
            "当前有 {count} 个 Host 映射不在推荐证书的覆盖范围内，如需对外暴露，仍需额外证书或调整域名规划。"
        }
        "server.gatewayVisibility.customCidrInvalid" => "自定义 CIDR 格式不正确：{cidrs}",
        "server.gatewayVisibility.emptyEnabledConfig" => {
            "开启可见性后，至少需要添加一个地区或一条自定义 CIDR"
        }
        "server.gatewayVisibility.syncFailed" => "同步网关可见性配置失败",
        "server.gatewayProxyHeaders.runTypes.direct" => "直连模式",
        "server.gatewayProxyHeaders.runTypes.reverseProxy" => "内网穿透",
        "server.gatewayProxyHeaders.runTypes.subdomain" => "子域模式",
        "server.gatewayProxyHeaders.unavailableReason" => "仅子域模式可用，当前为{mode}。",
        "server.gatewayProxyHeaders.syncFailed" => "同步网关协议头配置失败",
        "server.gatewayHostResponse.runTypes.direct" => "直连模式",
        "server.gatewayHostResponse.runTypes.reverseProxy" => "内网穿透",
        "server.gatewayHostResponse.runTypes.subdomain" => "子域模式",
        "server.gatewayHostResponse.unavailableReason" => "仅子域模式可用，当前为{mode}。",
        "server.gatewayHostResponse.editSubdomainOnly" => "Host 响应仅可在子域映射模式下编辑",
        "server.gatewayHostResponse.updateFailedRolledBack" => "更新网关 Host 响应失败，已回滚配置",
        "server.gatewayHostResponse.restoreConfigFailed" => "恢复 Host 响应原始配置失败",
        "server.gatewayHostResponse.restoreRuntimeFailed" => "恢复 Host 响应运行态失败",
        "server.gatewayHostResponse.restoreGatewayRuntimeFailed" => "恢复网关 Host 响应运行态失败",
        "server.admin.rollback.failed" => "{message}；回滚失败：{rollbackError}",
        "server.admin.rollback.restoreVisibilityConfigFailed" => "恢复可见性原始配置失败",
        "server.admin.rollback.restoreVisibilityRuntimeFailed" => "恢复可见性运行时 CIDR 失败",
        "server.admin.rollback.restoreGatewayVisibilityFailed" => "恢复网关可见性运行态失败",
        "server.admin.rollback.restoreProxyHeadersConfigFailed" => "恢复协议头原始配置失败",
        "server.admin.rollback.restoreProxyHeadersRuntimeFailed" => "恢复协议头运行态失败",
        "server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed" => {
            "恢复网关协议头运行态失败"
        }
        "server.admin.gatewayVisibility.updateFailedRolledBack" => "更新网关可见性失败，已回滚配置",
        "server.admin.gatewayProxyHeaders.subdomainOnly" => "协议头仅可在子域映射模式下编辑",
        "server.admin.gatewayProxyHeaders.updateFailedRolledBack" => {
            "更新网关协议头失败，已回滚配置"
        }
        "server.admin.hostMappings.bookmarkFolderForRoot" => "{root} 子域映射",
        "server.admin.hostMappings.bookmarkFolderDefault" => "fn-knock 子域映射",
        "server.whitelist.regionAddFailed" => "按地区新增白名单失败",
        "server.whitelist.regionRequired" => "请至少选择一个地区",
        "server.whitelist.regionEmpty" => "所选地区未解析到可用 CIDR",
        "server.whitelist.regionNotFound" => "未找到地区白名单",
        "server.scanDiscovery.selectAtLeastOneCidr" => "请选择至少一个本地 IPv4 扫描网段",
        "server.scanDiscovery.scanJobNotFound" => "扫描任务不存在或已过期",
        "server.dockerAdminPanel.resetHelp" => {
            "fn-knock 管理面板密码重置工具\n\n用法:\n  fn-knock-reset-panel-password\n\n作用:\n  - 清除管理面板密码\n  - 清除所有管理面板登录会话\n  - 清除登录失败退避状态\n\n执行完成后，下次访问管理入口会重新进入“首次设置密码”流程。"
        }
        "server.dockerAdminPanel.resetCleared" => "[fn-knock] 管理面板密码状态已清理",
        "server.dockerAdminPanel.resetNextVisit" => {
            "[fn-knock] 下次访问管理入口时，需要重新设置管理面板密码"
        }
        "server.dockerAdminPanel.resetFailed" => "[fn-knock] 清理管理面板密码失败:",
        _ => return None,
    })
}

fn en_message(key: &str) -> Option<&'static str> {
    Some(match key {
        "auth.autoIpGrantComment" => "Automatically authorized after sign-in",
        "server.systemClock.unknown" => "unknown",
        "server.systemClock.actionSeparator" => "; ",
        "server.systemClock.listSeparator" => ", ",
        "server.systemClock.duration.seconds" => "{seconds}s",
        "server.systemClock.duration.minutes" => "{minutes} min",
        "server.systemClock.duration.minutesSeconds" => "{minutes} min {seconds}s",
        "server.systemClock.networkTimeUnavailable" => {
            "Could not get standard time from the network"
        }
        "server.systemClock.sourceFetchFailed" => "Failed to get time from {source}",
        "server.systemClock.missingDateHeader" => "{source} did not return a usable Date header",
        "server.systemClock.invalidDateHeader" => "{source} returned an unparseable time",
        "server.systemClock.issues.timezone.title" => "System timezone is not Beijing time",
        "server.systemClock.issues.timezone.message" => {
            "Current system timezone is {timezone}; it should be {expected}."
        }
        "server.systemClock.issues.timeMismatch.title" => {
            "System time differs from the online check"
        }
        "server.systemClock.issues.timeMismatch.message" => {
            "System time differs from the online check by about {drift}."
        }
        "server.systemClock.statusRefreshed" => "System time status refreshed",
        "server.systemClock.timezoneSet" => "Set system timezone to {timezone}",
        "server.systemClock.missingZoneinfoFile" => "System timezone file is missing: {path}",
        "server.systemClock.timezoneWritten" => "Wrote system timezone {timezone}",
        "server.systemClock.clockAdjusted" => "System time adjusted",
        "server.systemClock.ntpEnabled" => "Enabled automatic NTP time sync",
        "server.systemClock.serviceRestarted" => "Restarted {service}",
        "server.acmeRoutes.domainsInvalid" => "Domain list is empty or invalid",
        "server.acmeRoutes.dnsTypeRequired" => "DNS verification type is missing",
        "server.acmeRoutes.unsupportedDnsProvider" => "Unsupported DNS provider",
        "server.acmeRoutes.missingDnsCredentials" => {
            "DNS API credentials are missing. Fill in one of these options: {requirements}"
        }
        "server.acmeRoutes.installingRetryLater" => "acme.sh is installing. Try again later.",
        "server.acmeRoutes.installFirst" => "Install acme.sh first",
        "server.acmeRoutes.multipleApplicationsUseNewApi" => {
            "Multiple request items already exist. Use the new API to manage ACME request items."
        }
        "server.acmeRoutes.applicationNotFound" => "Request item not found",
        "server.acmeRoutes.notFound" => "Not found",
        "server.acmeRoutes.installingCannotDelete" => "acme.sh is installing and cannot be deleted",
        "server.acmeRoutes.installingCannotSwitchCa" => {
            "acme.sh is installing. Certificate authority cannot be switched yet."
        }
        "server.acmeRoutes.noMatchingIssuedCertificate" => {
            "This request item has no issued certificate matching the domain configuration"
        }
        "server.acmeRoutes.success" => "Succeeded",
        "server.acmeRoutes.dns01Only" => "Only DNS-01 verification is supported",
        "server.acmeRoutes.certNotFound" => "Certificate not found",
        "server.acmeRoutes.certOrKeyInvalid" => "Certificate or private key is invalid",
        "server.acme.alreadyInstalled" => "acme.sh is already installed",
        "server.acme.installInProgress" => "Installation task is already in progress",
        "server.acme.installSubmitted" => "Installation task submitted",
        "server.acme.issueSucceeded" => "Certificate issued successfully",
        "server.acmeService.waiting" => "Waiting for action",
        "server.acmeService.ready" => "acme.sh is ready",
        "server.acmeService.sendSignalFailed" => "Failed to send {signal} to {target}: {detail}",
        "server.acmeService.setDefaultCaFailed" => {
            "Failed to set default certificate authority (exit code: {code}){brief}"
        }
        "server.acmeService.registerAccountFailed" => {
            "Failed to register ACME account (exit code: {code}){brief}"
        }
        "server.acmeService.bundledZipMissing" => "Bundled acmesh.zip resource was not found",
        "server.acmeService.extractingBundled" => "Extracting bundled acme.sh resources...",
        "server.acmeService.unzipFailed" => "Extraction failed, exit code: {code}",
        "server.acmeService.extractedAcmeMissing" => {
            "Extraction succeeded but acme.sh was not found"
        }
        "server.acmeService.writingDataDir" => "Writing data directory...",
        "server.acmeService.writtenAcmeMissing" => "acme.sh was not found after writing",
        "server.acmeService.checkInstallFailed" => "Failed to check installation status: {detail}",
        "server.acmeService.notInstalled" => "acme.sh is not installed",
        "server.acmeService.initializingBundled" => "Initializing bundled acme.sh...",
        "server.acmeService.registeringAccount" => "Registering ACME account...",
        "server.acmeService.savingDefaultCa" => "Saving default certificate authority...",
        "server.acmeService.installSuccess" => "Installation succeeded, account email: {email}",
        "server.acmeService.installFailed" => "Installation failed: {detail}",
        "server.acmeService.installFirst" => "Install acme.sh first",
        "server.acmeService.installingCannotDelete" => {
            "acme.sh is installing and cannot be deleted"
        }
        "server.acmeService.deleted" => "acme.sh was deleted",
        "server.acmeService.deleteFailed" => "Delete failed: {detail}",
        "server.acmeService.domainsRequired" => "Domain list is required",
        "server.acmeService.dnsTypeRequired" => "DNS verification type is missing",
        "server.acmeService.issueFailed" => {
            "Certificate issuance failed (exit code: {code}){brief}"
        }
        "server.acmeJobRunner.manualStop" => "The ACME task was stopped manually by the user",
        "server.acmeJobRunner.lockMessages.manualRequest" => "Requesting certificate",
        "server.acmeJobRunner.lockMessages.autoRenew" => "Automatically renewing certificate",
        "server.acmeJobRunner.flowFailed" => "Certificate request flow failed: {message}",
        "server.acmeJobRunner.activeTaskRunning" => {
            "An ACME task is already running. Try again later."
        }
        "server.acmeJobRunner.applicationChangedSkipped" => {
            "Request item domains changed during execution. Writing the old certificate was skipped. Start the request again."
        }
        "server.acmeJobRunner.issuedButApplicationChanged" => {
            "Certificate was issued, but the request item domains changed, so it was not written to the current request item."
        }
        "server.acmeJobRunner.issuedButCertReadFailed" => {
            "Certificate was issued, but reading the certificate file failed. Try again later or check the acme.sh directory."
        }
        "server.acmeJobRunner.clearedDomainWorkingState" => {
            "Cleared the acme.sh domain working directory. Certificate listing and renewal are now managed by system tasks."
        }
        "server.acmeJobRunner.clearDomainWorkingStateFailed" => {
            "Certificate was saved, but clearing acme.sh domain state failed: {message}"
        }
        "server.acmeJobRunner.linkedLibrarySyncedGateway" => {
            "Synced the linked certificate library entry and refreshed the gateway certificate list"
        }
        "server.acmeJobRunner.linkedLibraryUpdated" => {
            "Updated the linked certificate library entry"
        }
        "server.acmeJobRunner.addedToLibraryAndSyncedGateway" => {
            "Certificate was automatically added to the certificate library after issuance, and the gateway certificate list was refreshed"
        }
        "server.acmeJobRunner.addedToLibrary" => {
            "Certificate was automatically added to the certificate library after issuance"
        }
        "server.acmeJobRunner.addToLibraryFailed" => {
            "Certificate was issued and saved, but adding it to the certificate library failed: {message}"
        }
        "server.acmeJobRunner.stoppedIgnoredProcessError" => {
            "The task has stopped. The process exit error was ignored."
        }
        "server.store.acme.domainRequired" => "Domain is required",
        "server.store.acme.domainsRequired" => "Domain list is required",
        "server.store.acme.dnsProviderRequired" => "DNS provider is required",
        "server.store.acme.primaryDomainDuplicated" => {
            "Primary domain {primaryDomain} already exists in another request item"
        }
        "server.store.acme.applicationNotFound" => "Request item not found",
        "server.store.acme.noMatchingIssuedCertificate" => {
            "This request item has no issued certificate matching the domain configuration"
        }
        "server.store.ssl.certNotFound" => "Certificate not found",
        "server.store.ssl.certOrKeyInvalid" => "Certificate or private key is invalid",
        "server.acmeDnsProviders.groups.common" => "Common",
        "server.acmeDnsProviders.groups.domestic" => "China",
        "server.acmeDnsProviders.groups.international" => "International",
        "server.acmeDnsProviders.groups.selfHostedAdvanced" => "Self-hosted / Advanced",
        "server.acmeDnsProviders.credentialSchemes.default" => "Default credentials",
        "server.acmeDnsProviders.fields.accountEmail" => "Account email",
        "server.acmeDnsProviders.labels.aliyun" => "Alibaba Cloud DNS",
        "server.acmeDnsProviders.labels.tencentCloudDnspod" => {
            "Tencent Cloud DNSPod (TencentCloud)"
        }
        "server.acmeDnsProviders.labels.huaweiCloudDns" => "Huawei Cloud DNS",
        "server.acmeDnsProviders.labels.jdCloudDns" => "JD Cloud DNS",
        "server.acmeDnsProviders.labels.westCn" => "West.cn",
        "server.acmeDnsProviders.requirements.optionalSuffix" => "; optional {keys}",
        "server.acmeDnsProviders.requirements.orSeparator" => "; or ",
        "server.acmePatches.duckdns.scriptMissing" => {
            "DuckDNS DNS API script was not found: {path}"
        }
        "server.acmePatches.duckdns.proxyApplied" => "Switched DuckDNS API from {from} to {to}",
        "server.subdomainMode.recommendationMissingBase" => {
            "Root domain or auth service is not configured, so recommended certificate domains cannot be generated yet."
        }
        "server.subdomainMode.recommendationWildcardSummary" => {
            "Recommended domains: {rootDomain} and *.{rootDomain}, covering the root domain, auth service, and business subdomains under the same parent domain."
        }
        "server.subdomainMode.authOutOfRootWarning" => {
            "The current auth service {authHost} is not under root domain {rootDomain}; the exact domain was added separately. Confirm that the selected DNS provider can manage these domains."
        }
        "server.subdomainMode.recommendationSingleHostSummary" => {
            "Root domain is not configured, so only a single-domain certificate for auth service {authHost} can be recommended."
        }
        "server.subdomainMode.wildcardSuggestion" => {
            "To cover multiple business subdomains later, add the root domain before requesting a wildcard certificate."
        }
        "server.subdomainMode.configureRootOrAuth" => {
            "Configure a root domain in subdomain mode, or specify an auth service in Host mappings first."
        }
        "server.subdomainMode.authMissingWarning" => {
            "Auth service is not specified, so the recommendation is derived only from the root domain."
        }
        "server.subdomainMode.uncoveredHostMappingsWarning" => {
            "{count} Host mappings are outside the recommended certificate coverage. If they need public exposure, add certificates or adjust domain planning."
        }
        "server.gatewayVisibility.customCidrInvalid" => "Custom CIDR format is invalid: {cidrs}",
        "server.gatewayVisibility.emptyEnabledConfig" => {
            "After enabling visibility, add at least one region or one custom CIDR"
        }
        "server.gatewayVisibility.syncFailed" => "Failed to sync gateway visibility configuration",
        "server.gatewayProxyHeaders.runTypes.direct" => "direct mode",
        "server.gatewayProxyHeaders.runTypes.reverseProxy" => "reverse proxy mode",
        "server.gatewayProxyHeaders.runTypes.subdomain" => "subdomain mode",
        "server.gatewayProxyHeaders.unavailableReason" => {
            "Only subdomain mode is available. Current mode: {mode}."
        }
        "server.gatewayProxyHeaders.syncFailed" => {
            "Failed to sync gateway proxy header configuration"
        }
        "server.gatewayHostResponse.runTypes.direct" => "direct mode",
        "server.gatewayHostResponse.runTypes.reverseProxy" => "reverse proxy mode",
        "server.gatewayHostResponse.runTypes.subdomain" => "subdomain mode",
        "server.gatewayHostResponse.unavailableReason" => {
            "Only subdomain mode is available. Current mode: {mode}."
        }
        "server.gatewayHostResponse.editSubdomainOnly" => {
            "Host response can only be edited in subdomain mapping mode"
        }
        "server.gatewayHostResponse.updateFailedRolledBack" => {
            "Failed to update gateway Host response; configuration was rolled back"
        }
        "server.gatewayHostResponse.restoreConfigFailed" => {
            "Failed to restore Host response configuration"
        }
        "server.gatewayHostResponse.restoreRuntimeFailed" => {
            "Failed to restore Host response runtime state"
        }
        "server.gatewayHostResponse.restoreGatewayRuntimeFailed" => {
            "Failed to restore gateway Host response runtime state"
        }
        "server.admin.rollback.failed" => "{message}; rollback failed: {rollbackError}",
        "server.admin.rollback.restoreVisibilityConfigFailed" => {
            "Failed to restore visibility configuration"
        }
        "server.admin.rollback.restoreVisibilityRuntimeFailed" => {
            "Failed to restore visibility runtime CIDRs"
        }
        "server.admin.rollback.restoreGatewayVisibilityFailed" => {
            "Failed to restore gateway visibility runtime state"
        }
        "server.admin.rollback.restoreProxyHeadersConfigFailed" => {
            "Failed to restore proxy header configuration"
        }
        "server.admin.rollback.restoreProxyHeadersRuntimeFailed" => {
            "Failed to restore proxy header runtime state"
        }
        "server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed" => {
            "Failed to restore gateway proxy header runtime state"
        }
        "server.admin.gatewayVisibility.updateFailedRolledBack" => {
            "Failed to update gateway visibility; configuration was rolled back"
        }
        "server.admin.gatewayProxyHeaders.subdomainOnly" => {
            "Proxy headers can only be edited in subdomain mapping mode"
        }
        "server.admin.gatewayProxyHeaders.updateFailedRolledBack" => {
            "Failed to update gateway proxy headers; configuration was rolled back"
        }
        "server.admin.hostMappings.bookmarkFolderForRoot" => "{root} subdomain mappings",
        "server.admin.hostMappings.bookmarkFolderDefault" => "fn-knock subdomain mappings",
        "server.whitelist.regionAddFailed" => "Failed to add region whitelist",
        "server.whitelist.regionRequired" => "Select at least one region",
        "server.whitelist.regionEmpty" => "No usable CIDRs were resolved for the selected regions",
        "server.whitelist.regionNotFound" => "Region whitelist not found",
        "server.scanDiscovery.selectAtLeastOneCidr" => "Select at least one local IPv4 scan range",
        "server.scanDiscovery.scanJobNotFound" => "Scan job not found or expired",
        "server.dockerAdminPanel.resetHelp" => {
            "fn-knock admin panel password reset tool\n\nUsage:\n  fn-knock-reset-panel-password\n\nActions:\n  - Clear the admin panel password\n  - Clear all admin panel login sessions\n  - Clear login failure backoff state\n\nAfter completion, the next visit to the admin entry will enter the first-time password setup flow again."
        }
        "server.dockerAdminPanel.resetCleared" => "[fn-knock] Admin panel password state cleared",
        "server.dockerAdminPanel.resetNextVisit" => {
            "[fn-knock] Set the admin panel password again on the next visit to the admin entry"
        }
        "server.dockerAdminPanel.resetFailed" => "[fn-knock] Failed to clear admin panel password:",
        _ => return None,
    })
}

fn zh_hant_message(key: &str) -> Option<&'static str> {
    Some(match key {
        "auth.autoIpGrantComment" => "登入後自動授權",
        "server.systemClock.unknown" => "未知",
        "server.systemClock.actionSeparator" => "；",
        "server.systemClock.listSeparator" => "，",
        "server.systemClock.duration.seconds" => "{seconds} 秒",
        "server.systemClock.duration.minutes" => "{minutes} 分鐘",
        "server.systemClock.duration.minutesSeconds" => "{minutes} 分 {seconds} 秒",
        "server.systemClock.networkTimeUnavailable" => "未能從網路獲取標準時間",
        "server.systemClock.sourceFetchFailed" => "從 {source} 獲取時間失敗",
        "server.systemClock.missingDateHeader" => "{source} 未返回可用的 Date 響應頭",
        "server.systemClock.invalidDateHeader" => "{source} 返回了無法解析的時間",
        "server.systemClock.issues.timezone.title" => "系統時區不是北京時間",
        "server.systemClock.issues.timezone.message" => {
            "目前系統時區為 {timezone}，應設定為 {expected}。"
        }
        "server.systemClock.issues.timeMismatch.title" => "系統時間與聯網校驗結果不一致",
        "server.systemClock.issues.timeMismatch.message" => {
            "目前系統時間與聯網校驗結果相差約 {drift}。"
        }
        "server.systemClock.statusRefreshed" => "系統時間狀態已刷新",
        "server.systemClock.timezoneSet" => "已設定系統時區為 {timezone}",
        "server.systemClock.missingZoneinfoFile" => "系統缺少時區文件 {path}",
        "server.systemClock.timezoneWritten" => "已寫入系統時區 {timezone}",
        "server.systemClock.clockAdjusted" => "已校準系統時間",
        "server.systemClock.ntpEnabled" => "已啟用 NTP 自動校時",
        "server.systemClock.serviceRestarted" => "已重啟 {service} 服務",
        "server.acmeRoutes.domainsInvalid" => "域名列表不能為空或格式無效",
        "server.acmeRoutes.dnsTypeRequired" => "缺少 DNS 驗證類型",
        "server.acmeRoutes.unsupportedDnsProvider" => "不支持的 DNS 服務商",
        "server.acmeRoutes.missingDnsCredentials" => {
            "缺少 DNS API 憑據，請填寫以下任一方案: {requirements}"
        }
        "server.acmeRoutes.installingRetryLater" => "acme.sh 安裝中，請稍後再試",
        "server.acmeRoutes.installFirst" => "請先安裝 acme.sh",
        "server.acmeRoutes.multipleApplicationsUseNewApi" => {
            "目前已存在多個申請項，請使用新接口管理 ACME 申請項"
        }
        "server.acmeRoutes.applicationNotFound" => "申請項不存在",
        "server.acmeRoutes.notFound" => "未找到",
        "server.acmeRoutes.installingCannotDelete" => "acme.sh 安裝中，無法刪除",
        "server.acmeRoutes.installingCannotSwitchCa" => "acme.sh 安裝中，暫時無法切換憑證頒發機構",
        "server.acmeRoutes.noMatchingIssuedCertificate" => {
            "目前申請項還沒有與域名設定相符的已簽發憑證"
        }
        "server.acmeRoutes.success" => "成功",
        "server.acmeRoutes.dns01Only" => "僅支援 DNS-01 驗證方式",
        "server.acmeRoutes.certNotFound" => "憑證不存在",
        "server.acmeRoutes.certOrKeyInvalid" => "憑證或私鑰無效",
        "server.acme.alreadyInstalled" => "acme.sh 已經安裝過了",
        "server.acme.installInProgress" => "安裝任務正在進行中",
        "server.acme.installSubmitted" => "安裝任務已提交",
        "server.acme.issueSucceeded" => "證書簽發成功",
        "server.acmeService.waiting" => "等待操作",
        "server.acmeService.ready" => "acme.sh 已就緒",
        "server.acmeService.sendSignalFailed" => "傳送 {signal} 到 {target} 失敗: {detail}",
        "server.acmeService.setDefaultCaFailed" => {
            "設定預設憑證頒發機構失敗（退出碼: {code}）{brief}"
        }
        "server.acmeService.registerAccountFailed" => "註冊 ACME 帳號失敗（退出碼: {code}）{brief}",
        "server.acmeService.bundledZipMissing" => "未找到內建 acmesh.zip 資源",
        "server.acmeService.extractingBundled" => "正在解壓內建 acme.sh 資源...",
        "server.acmeService.unzipFailed" => "解壓失敗，退出碼: {code}",
        "server.acmeService.extractedAcmeMissing" => "解壓成功但未找到 acme.sh",
        "server.acmeService.writingDataDir" => "正在寫入資料目錄...",
        "server.acmeService.writtenAcmeMissing" => "寫入後未找到 acme.sh",
        "server.acmeService.checkInstallFailed" => "檢查安裝狀態失敗: {detail}",
        "server.acmeService.notInstalled" => "acme.sh 未安裝",
        "server.acmeService.initializingBundled" => "正在初始化內建 acme.sh...",
        "server.acmeService.registeringAccount" => "正在註冊 ACME 帳號...",
        "server.acmeService.savingDefaultCa" => "正在儲存預設憑證頒發機構...",
        "server.acmeService.installSuccess" => "安裝成功，帳號信箱: {email}",
        "server.acmeService.installFailed" => "安裝失敗: {detail}",
        "server.acmeService.installFirst" => "請先安裝 acme.sh",
        "server.acmeService.installingCannotDelete" => "acme.sh 正在安裝中，無法刪除",
        "server.acmeService.deleted" => "acme.sh 已刪除",
        "server.acmeService.deleteFailed" => "刪除失敗: {detail}",
        "server.acmeService.domainsRequired" => "域名列表不能為空",
        "server.acmeService.dnsTypeRequired" => "缺少 DNS 驗證類型",
        "server.acmeService.issueFailed" => "憑證簽發失敗（退出碼: {code}）{brief}",
        "server.acmeJobRunner.manualStop" => "ACME 任務已由使用者手動停止",
        "server.acmeJobRunner.lockMessages.manualRequest" => "正在申請憑證",
        "server.acmeJobRunner.lockMessages.autoRenew" => "正在自動續期憑證",
        "server.acmeJobRunner.flowFailed" => "憑證申請流程失敗: {message}",
        "server.acmeJobRunner.activeTaskRunning" => "目前已有 ACME 任務正在執行，請稍後再試",
        "server.acmeJobRunner.applicationChangedSkipped" => {
            "申請項域名已在執行期間發生變更，已跳過寫入舊憑證，請重新發起申請"
        }
        "server.acmeJobRunner.issuedButApplicationChanged" => {
            "憑證簽發成功，但由於申請項域名已變更，未寫入目前申請項"
        }
        "server.acmeJobRunner.issuedButCertReadFailed" => {
            "憑證簽發成功，但讀取憑證檔案失敗（請稍後重試或檢查 acme.sh 目錄）"
        }
        "server.acmeJobRunner.clearedDomainWorkingState" => {
            "已清理 acme.sh 域名工作目錄，憑證列表與續期由系統任務統一管理"
        }
        "server.acmeJobRunner.clearDomainWorkingStateFailed" => {
            "憑證已儲存，但清理 acme.sh 域名狀態失敗: {message}"
        }
        "server.acmeJobRunner.linkedLibrarySyncedGateway" => {
            "已同步已關聯的憑證庫項目，並刷新閘道憑證列表"
        }
        "server.acmeJobRunner.linkedLibraryUpdated" => "已更新已關聯的憑證庫項目",
        "server.acmeJobRunner.addedToLibraryAndSyncedGateway" => {
            "憑證簽發成功後已自動加入憑證庫，並刷新閘道憑證列表"
        }
        "server.acmeJobRunner.addedToLibrary" => "憑證簽發成功後已自動加入憑證庫",
        "server.acmeJobRunner.addToLibraryFailed" => {
            "憑證已簽發並儲存，但自動加入憑證庫失敗: {message}"
        }
        "server.acmeJobRunner.stoppedIgnoredProcessError" => "任務已停止，已忽略程序退出後的錯誤",
        "server.store.acme.domainRequired" => "域名不能為空",
        "server.store.acme.domainsRequired" => "域名列表不能為空",
        "server.store.acme.dnsProviderRequired" => "DNS 服務商不能為空",
        "server.store.acme.primaryDomainDuplicated" => {
            "主域名 {primaryDomain} 已存在於其他申請項中"
        }
        "server.store.acme.applicationNotFound" => "申請項不存在",
        "server.store.acme.noMatchingIssuedCertificate" => {
            "目前申請項還沒有與域名設定相符的已簽發憑證"
        }
        "server.store.ssl.certNotFound" => "憑證不存在",
        "server.store.ssl.certOrKeyInvalid" => "憑證或私鑰無效",
        "server.acmeDnsProviders.groups.common" => "常用",
        "server.acmeDnsProviders.groups.domestic" => "國內",
        "server.acmeDnsProviders.groups.international" => "國際",
        "server.acmeDnsProviders.groups.selfHostedAdvanced" => "自建/高級",
        "server.acmeDnsProviders.credentialSchemes.default" => "默認憑據",
        "server.acmeDnsProviders.fields.accountEmail" => "帳號信箱",
        "server.acmeDnsProviders.labels.aliyun" => "阿里雲 DNS",
        "server.acmeDnsProviders.labels.tencentCloudDnspod" => "騰訊雲 DNSPod (TencentCloud)",
        "server.acmeDnsProviders.labels.huaweiCloudDns" => "華為雲 DNS",
        "server.acmeDnsProviders.labels.jdCloudDns" => "京東雲 DNS",
        "server.acmeDnsProviders.labels.westCn" => "西部數碼",
        "server.acmeDnsProviders.requirements.optionalSuffix" => "；可選 {keys}",
        "server.acmeDnsProviders.requirements.orSeparator" => "；或 ",
        "server.acmePatches.duckdns.scriptMissing" => "未找到 DuckDNS DNS API 腳本: {path}",
        "server.acmePatches.duckdns.proxyApplied" => "已將 DuckDNS API 從 {from} 切換為 {to}",
        "server.subdomainMode.recommendationMissingBase" => {
            "尚未設定根域名或鑑權服務，暫時無法產生推薦憑證域名。"
        }
        "server.subdomainMode.recommendationWildcardSummary" => {
            "推薦申請 {rootDomain} 與 *.{rootDomain}，用於覆蓋根域名、鑑權服務和同一父域下的業務子域。"
        }
        "server.subdomainMode.authOutOfRootWarning" => {
            "目前鑑權服務 {authHost} 不在根域名 {rootDomain} 下，已額外加入精確域名；請確認所選 DNS 服務商能夠管理這些域名。"
        }
        "server.subdomainMode.recommendationSingleHostSummary" => {
            "尚未設定根域名，目前僅能推薦為鑑權服務 {authHost} 申請單域名憑證。"
        }
        "server.subdomainMode.wildcardSuggestion" => {
            "如果後續要統一覆蓋多個業務子域，建議先補充根域名後再申請 wildcard 憑證。"
        }
        "server.subdomainMode.configureRootOrAuth" => {
            "請先在子域模式裡設定根域名，或在 Host 映射中指定一條鑑權服務。"
        }
        "server.subdomainMode.authMissingWarning" => {
            "尚未指定鑑權服務，目前推薦結果只基於根域名推導。"
        }
        "server.subdomainMode.uncoveredHostMappingsWarning" => {
            "目前有 {count} 個 Host 映射不在推薦憑證的覆蓋範圍內，如需對外暴露，仍需額外憑證或調整域名規劃。"
        }
        "server.gatewayVisibility.customCidrInvalid" => "自訂 CIDR 格式不正確：{cidrs}",
        "server.gatewayVisibility.emptyEnabledConfig" => {
            "開啟可見性後，至少需要新增一個地區或一條自訂 CIDR"
        }
        "server.gatewayVisibility.syncFailed" => "同步閘道可見性設定失敗",
        "server.gatewayProxyHeaders.runTypes.direct" => "直連模式",
        "server.gatewayProxyHeaders.runTypes.reverseProxy" => "内网穿透",
        "server.gatewayProxyHeaders.runTypes.subdomain" => "子域模式",
        "server.gatewayProxyHeaders.unavailableReason" => "僅子域模式可用，目前為{mode}。",
        "server.gatewayProxyHeaders.syncFailed" => "同步閘道協議頭設定失敗",
        "server.gatewayHostResponse.runTypes.direct" => "直連模式",
        "server.gatewayHostResponse.runTypes.reverseProxy" => "内网穿透",
        "server.gatewayHostResponse.runTypes.subdomain" => "子域模式",
        "server.gatewayHostResponse.unavailableReason" => "僅子域模式可用，目前為{mode}。",
        "server.gatewayHostResponse.editSubdomainOnly" => "Host 響應僅可在子域映射模式下編輯",
        "server.gatewayHostResponse.updateFailedRolledBack" => "更新閘道 Host 響應失敗，已回滾設定",
        "server.gatewayHostResponse.restoreConfigFailed" => "恢復 Host 響應原始設定失敗",
        "server.gatewayHostResponse.restoreRuntimeFailed" => "恢復 Host 響應運行態失敗",
        "server.gatewayHostResponse.restoreGatewayRuntimeFailed" => "恢復閘道 Host 響應運行態失敗",
        "server.admin.rollback.failed" => "{message}；回滾失敗：{rollbackError}",
        "server.admin.rollback.restoreVisibilityConfigFailed" => "恢復可見性原始設定失敗",
        "server.admin.rollback.restoreVisibilityRuntimeFailed" => "恢復可見性運行時 CIDR 失敗",
        "server.admin.rollback.restoreGatewayVisibilityFailed" => "恢復閘道可見性運行態失敗",
        "server.admin.rollback.restoreProxyHeadersConfigFailed" => "恢復協議頭原始設定失敗",
        "server.admin.rollback.restoreProxyHeadersRuntimeFailed" => "恢復協議頭運行態失敗",
        "server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed" => {
            "恢復閘道協議頭運行態失敗"
        }
        "server.admin.gatewayVisibility.updateFailedRolledBack" => "更新閘道可見性失敗，已回滾設定",
        "server.admin.gatewayProxyHeaders.subdomainOnly" => "協議頭僅可在子域映射模式下編輯",
        "server.admin.gatewayProxyHeaders.updateFailedRolledBack" => {
            "更新閘道協議頭失敗，已回滾設定"
        }
        "server.admin.hostMappings.bookmarkFolderForRoot" => "{root} 子域映射",
        "server.admin.hostMappings.bookmarkFolderDefault" => "fn-knock 子域映射",
        "server.whitelist.regionAddFailed" => "按地區新增白名單失敗",
        "server.whitelist.regionRequired" => "請至少選擇一個地區",
        "server.whitelist.regionEmpty" => "所選地區未解析到可用 CIDR",
        "server.whitelist.regionNotFound" => "未找到地區白名單",
        "server.scanDiscovery.selectAtLeastOneCidr" => "請選擇至少一個本地 IPv4 掃描網段",
        "server.scanDiscovery.scanJobNotFound" => "掃描任務不存在或已過期",
        "server.dockerAdminPanel.resetHelp" => {
            "fn-knock 管理面板密碼重置工具\n\n用法:\n  fn-knock-reset-panel-password\n\n作用:\n  - 清除管理面板密碼\n  - 清除所有管理面板登入會話\n  - 清除登入失敗退避狀態\n\n執行完成後，下次訪問管理入口會重新進入「首次設定密碼」流程。"
        }
        "server.dockerAdminPanel.resetCleared" => "[fn-knock] 管理面板密碼狀態已清理",
        "server.dockerAdminPanel.resetNextVisit" => {
            "[fn-knock] 下次訪問管理入口時，需要重新設定管理面板密碼"
        }
        "server.dockerAdminPanel.resetFailed" => "[fn-knock] 清理管理面板密碼失敗:",
        _ => return None,
    })
}

fn ko_kr_message(key: &str) -> Option<&'static str> {
    Some(match key {
        "auth.autoIpGrantComment" => "로그인 후 자동 승인됨",
        "server.systemClock.unknown" => "알 수 없음",
        "server.systemClock.actionSeparator" => "; ",
        "server.systemClock.listSeparator" => ", ",
        "server.systemClock.duration.seconds" => "{seconds}초",
        "server.systemClock.duration.minutes" => "{minutes}분",
        "server.systemClock.duration.minutesSeconds" => "{minutes}분 {seconds}초",
        "server.systemClock.networkTimeUnavailable" => "네트워크에서 표준시를 가져올 수 없습니다.",
        "server.systemClock.sourceFetchFailed" => "{source}에서 시간을 가져오지 못했습니다.",
        "server.systemClock.missingDateHeader" => {
            "{source}이 사용 가능한 Date 응답 헤더를 반환하지 않았습니다."
        }
        "server.systemClock.invalidDateHeader" => {
            "{source}이 구문 분석할 수 없는 시간을 반환했습니다."
        }
        "server.systemClock.issues.timezone.title" => "시스템 시간대가 베이징 시간이 아닙니다.",
        "server.systemClock.issues.timezone.message" => {
            "현재 시스템 시간대는 {timezone}입니다. {expected}이어야 합니다."
        }
        "server.systemClock.issues.timeMismatch.title" => "시스템 시간이 온라인 확인과 다름",
        "server.systemClock.issues.timeMismatch.message" => {
            "시스템 시간은 온라인 확인과 {drift} 정도 다릅니다."
        }
        "server.systemClock.statusRefreshed" => "시스템 시간 상태가 새로 고쳐졌습니다.",
        "server.systemClock.timezoneSet" => "시스템 시간대를 {timezone}으로 설정했습니다.",
        "server.systemClock.missingZoneinfoFile" => "시스템 시간대 파일이 누락되었습니다: {path}",
        "server.systemClock.timezoneWritten" => "시스템 시간대 {timezone}을 작성했습니다.",
        "server.systemClock.clockAdjusted" => "시스템 시간이 조정되었습니다.",
        "server.systemClock.ntpEnabled" => "자동 NTP 시간 동기화 활성화",
        "server.systemClock.serviceRestarted" => "{service} 서비스를 다시 시작했습니다.",
        "server.acmeRoutes.domainsInvalid" => "도메인 목록이 비어 있거나 유효하지 않습니다.",
        "server.acmeRoutes.dnsTypeRequired" => "DNS 확인 유형이 누락되었습니다.",
        "server.acmeRoutes.unsupportedDnsProvider" => "지원되지 않는 DNS 공급자",
        "server.acmeRoutes.missingDnsCredentials" => {
            "DNS API 자격 증명이 누락되었습니다. 다음 옵션 중 하나를 입력하세요: {requirements}"
        }
        "server.acmeRoutes.installingRetryLater" => {
            "acme.sh 설치 중입니다. 잠시 후 다시 시도하세요."
        }
        "server.acmeRoutes.installFirst" => "먼저 acme.sh를 설치하세요",
        "server.acmeRoutes.multipleApplicationsUseNewApi" => {
            "요청 항목이 여러 개 있습니다. 새 API로 ACME 요청 항목을 관리하세요."
        }
        "server.acmeRoutes.applicationNotFound" => "요청 항목을 찾을 수 없습니다",
        "server.acmeRoutes.notFound" => "찾을 수 없음",
        "server.acmeRoutes.installingCannotDelete" => "acme.sh 설치 중이라 삭제할 수 없습니다",
        "server.acmeRoutes.installingCannotSwitchCa" => {
            "acme.sh 설치 중입니다. 아직 인증 기관을 전환할 수 없습니다."
        }
        "server.acmeRoutes.noMatchingIssuedCertificate" => {
            "이 요청 항목에는 도메인 구성과 일치하는 발급된 인증서가 없습니다"
        }
        "server.acmeRoutes.success" => "성공함",
        "server.acmeRoutes.dns01Only" => "DNS-01 확인 방식만 지원됩니다",
        "server.acmeRoutes.certNotFound" => "인증서를 찾을 수 없습니다",
        "server.acmeRoutes.certOrKeyInvalid" => "인증서 또는 개인 키가 유효하지 않습니다",
        "server.acme.alreadyInstalled" => "acme.sh가 이미 설치되어 있습니다.",
        "server.acme.installInProgress" => "설치 작업이 이미 진행 중입니다.",
        "server.acme.installSubmitted" => "설치 작업이 제출되었습니다.",
        "server.acme.issueSucceeded" => "인증서가 성공적으로 발급되었습니다.",
        "server.acmeService.waiting" => "작업 대기 중",
        "server.acmeService.ready" => "acme.sh 준비 완료",
        "server.acmeService.sendSignalFailed" => "{target}에 {signal} 전송 실패: {detail}",
        "server.acmeService.setDefaultCaFailed" => {
            "기본 인증 기관 설정 실패(종료 코드: {code}){brief}"
        }
        "server.acmeService.registerAccountFailed" => {
            "ACME 계정 등록 실패(종료 코드: {code}){brief}"
        }
        "server.acmeService.bundledZipMissing" => "내장 acmesh.zip 리소스를 찾을 수 없습니다",
        "server.acmeService.extractingBundled" => "내장 acme.sh 리소스 압축 해제 중...",
        "server.acmeService.unzipFailed" => "압축 해제 실패, 종료 코드: {code}",
        "server.acmeService.extractedAcmeMissing" => {
            "압축 해제는 성공했지만 acme.sh를 찾을 수 없습니다"
        }
        "server.acmeService.writingDataDir" => "데이터 디렉터리에 쓰는 중...",
        "server.acmeService.writtenAcmeMissing" => "쓰기 후 acme.sh를 찾을 수 없습니다",
        "server.acmeService.checkInstallFailed" => "설치 상태 확인 실패: {detail}",
        "server.acmeService.notInstalled" => "acme.sh가 설치되어 있지 않습니다",
        "server.acmeService.initializingBundled" => "내장 acme.sh 초기화 중...",
        "server.acmeService.registeringAccount" => "ACME 계정 등록 중...",
        "server.acmeService.savingDefaultCa" => "기본 인증 기관 저장 중...",
        "server.acmeService.installSuccess" => "설치 성공, 계정 이메일: {email}",
        "server.acmeService.installFailed" => "설치 실패: {detail}",
        "server.acmeService.installFirst" => "먼저 acme.sh를 설치하세요",
        "server.acmeService.installingCannotDelete" => "acme.sh 설치 중이라 삭제할 수 없습니다",
        "server.acmeService.deleted" => "acme.sh가 삭제되었습니다",
        "server.acmeService.deleteFailed" => "삭제 실패: {detail}",
        "server.acmeService.domainsRequired" => "도메인 목록은 필수입니다",
        "server.acmeService.dnsTypeRequired" => "DNS 확인 유형이 누락되었습니다",
        "server.acmeService.issueFailed" => "인증서 발급 실패(종료 코드: {code}){brief}",
        "server.acmeJobRunner.manualStop" => "ACME 작업이 사용자에 의해 수동으로 중지되었습니다",
        "server.acmeJobRunner.lockMessages.manualRequest" => "인증서 요청 중",
        "server.acmeJobRunner.lockMessages.autoRenew" => "인증서 자동 갱신 중",
        "server.acmeJobRunner.flowFailed" => "인증서 요청 흐름 실패: {message}",
        "server.acmeJobRunner.activeTaskRunning" => {
            "ACME 작업이 이미 실행 중입니다. 나중에 다시 시도하세요."
        }
        "server.acmeJobRunner.applicationChangedSkipped" => {
            "실행 중 요청 항목의 도메인이 변경되어 이전 인증서 쓰기를 건너뛰었습니다. 다시 요청하세요."
        }
        "server.acmeJobRunner.issuedButApplicationChanged" => {
            "인증서는 발급되었지만 요청 항목의 도메인이 변경되어 현재 요청 항목에 쓰지 않았습니다."
        }
        "server.acmeJobRunner.issuedButCertReadFailed" => {
            "인증서는 발급되었지만 인증서 파일 읽기에 실패했습니다. 나중에 다시 시도하거나 acme.sh 디렉터리를 확인하세요."
        }
        "server.acmeJobRunner.clearedDomainWorkingState" => {
            "acme.sh 도메인 작업 디렉터리를 정리했습니다. 인증서 목록과 갱신은 이제 시스템 작업에서 관리합니다."
        }
        "server.acmeJobRunner.clearDomainWorkingStateFailed" => {
            "인증서는 저장되었지만 acme.sh 도메인 상태 정리에 실패했습니다: {message}"
        }
        "server.acmeJobRunner.linkedLibrarySyncedGateway" => {
            "연결된 인증서 라이브러리 항목을 동기화하고 게이트웨이 인증서 목록을 새로 고쳤습니다"
        }
        "server.acmeJobRunner.linkedLibraryUpdated" => {
            "연결된 인증서 라이브러리 항목을 업데이트했습니다"
        }
        "server.acmeJobRunner.addedToLibraryAndSyncedGateway" => {
            "인증서 발급 후 인증서 라이브러리에 자동으로 추가하고 게이트웨이 인증서 목록을 새로 고쳤습니다"
        }
        "server.acmeJobRunner.addedToLibrary" => {
            "인증서 발급 후 인증서 라이브러리에 자동으로 추가했습니다"
        }
        "server.acmeJobRunner.addToLibraryFailed" => {
            "인증서는 발급 및 저장되었지만 인증서 라이브러리 추가에 실패했습니다: {message}"
        }
        "server.acmeJobRunner.stoppedIgnoredProcessError" => {
            "작업이 중지되어 프로세스 종료 오류를 무시했습니다"
        }
        "server.store.acme.domainRequired" => "도메인은 필수입니다",
        "server.store.acme.domainsRequired" => "도메인 목록은 필수입니다",
        "server.store.acme.dnsProviderRequired" => "DNS 공급자는 필수입니다",
        "server.store.acme.primaryDomainDuplicated" => {
            "기본 도메인 {primaryDomain}이 다른 요청 항목에 이미 있습니다"
        }
        "server.store.acme.applicationNotFound" => "요청 항목을 찾을 수 없습니다",
        "server.store.acme.noMatchingIssuedCertificate" => {
            "이 요청 항목에는 도메인 구성과 일치하는 발급된 인증서가 없습니다"
        }
        "server.store.ssl.certNotFound" => "인증서를 찾을 수 없습니다",
        "server.store.ssl.certOrKeyInvalid" => "인증서 또는 개인 키가 유효하지 않습니다",
        "server.acmeDnsProviders.groups.common" => "공통",
        "server.acmeDnsProviders.groups.domestic" => "중국",
        "server.acmeDnsProviders.groups.international" => "국제",
        "server.acmeDnsProviders.groups.selfHostedAdvanced" => "자체 호스팅/고급",
        "server.acmeDnsProviders.credentialSchemes.default" => "기본 자격 증명",
        "server.acmeDnsProviders.fields.accountEmail" => "계정 이메일",
        "server.acmeDnsProviders.labels.aliyun" => "Alibaba Cloud DNS",
        "server.acmeDnsProviders.labels.tencentCloudDnspod" => {
            "Tencent Cloud DNSPod (TencentCloud)"
        }
        "server.acmeDnsProviders.labels.huaweiCloudDns" => "Huawei Cloud DNS",
        "server.acmeDnsProviders.labels.jdCloudDns" => "JD Cloud DNS",
        "server.acmeDnsProviders.labels.westCn" => "West.cn",
        "server.acmeDnsProviders.requirements.optionalSuffix" => "; 선택 사항 {keys}",
        "server.acmeDnsProviders.requirements.orSeparator" => "; 또는 ",
        "server.acmePatches.duckdns.scriptMissing" => {
            "DuckDNS DNS API 스크립트를 찾을 수 없습니다: {path}"
        }
        "server.acmePatches.duckdns.proxyApplied" => "{from}에서 {to}로 DuckDNS API를 전환했습니다",
        "server.subdomainMode.recommendationMissingBase" => {
            "루트 도메인 또는 인증 서비스가 구성되지 않아 추천 인증서 도메인을 아직 생성할 수 없습니다."
        }
        "server.subdomainMode.recommendationWildcardSummary" => {
            "추천 도메인: {rootDomain} 및 *.{rootDomain}. 루트 도메인, 인증 서비스, 동일 상위 도메인의 업무 서브도메인을 포함합니다."
        }
        "server.subdomainMode.authOutOfRootWarning" => {
            "현재 인증 서비스 {authHost}가 루트 도메인 {rootDomain} 아래에 없어 정확한 도메인을 별도로 추가했습니다. 선택한 DNS 공급자가 이 도메인들을 관리할 수 있는지 확인하세요."
        }
        "server.subdomainMode.recommendationSingleHostSummary" => {
            "루트 도메인이 구성되지 않아 인증 서비스 {authHost}의 단일 도메인 인증서만 추천할 수 있습니다."
        }
        "server.subdomainMode.wildcardSuggestion" => {
            "나중에 여러 업무 서브도메인을 함께 포함하려면 wildcard 인증서를 요청하기 전에 루트 도메인을 추가하세요."
        }
        "server.subdomainMode.configureRootOrAuth" => {
            "먼저 서브도메인 모드에서 루트 도메인을 구성하거나 Host 매핑에 인증 서비스를 지정하세요."
        }
        "server.subdomainMode.authMissingWarning" => {
            "인증 서비스가 지정되지 않아 추천 결과는 루트 도메인만 기준으로 계산됩니다."
        }
        "server.subdomainMode.uncoveredHostMappingsWarning" => {
            "{count}개의 Host 매핑이 추천 인증서 범위 밖에 있습니다. 외부 노출이 필요하면 인증서를 추가하거나 도메인 계획을 조정하세요."
        }
        "server.gatewayVisibility.customCidrInvalid" => "맞춤 CIDR 형식이 잘못되었습니다. {cidrs}",
        "server.gatewayVisibility.emptyEnabledConfig" => {
            "가시성을 활성화한 후 하나 이상의 지역 또는 하나의 사용자 정의 CIDR을 추가하세요."
        }
        "server.gatewayVisibility.syncFailed" => {
            "게이트웨이 공개 상태 구성을 동기화하지 못했습니다."
        }
        "server.gatewayProxyHeaders.runTypes.direct" => "직접 모드",
        "server.gatewayProxyHeaders.runTypes.reverseProxy" => "역방향 프록시 모드",
        "server.gatewayProxyHeaders.runTypes.subdomain" => "하위 도메인 모드",
        "server.gatewayProxyHeaders.unavailableReason" => {
            "하위 도메인 모드만 사용할 수 있습니다. 현재 모드: {mode}."
        }
        "server.gatewayProxyHeaders.syncFailed" => {
            "게이트웨이 프록시 헤더 구성을 동기화하지 못했습니다."
        }
        "server.gatewayHostResponse.runTypes.direct" => "직접 모드",
        "server.gatewayHostResponse.runTypes.reverseProxy" => "역방향 프록시 모드",
        "server.gatewayHostResponse.runTypes.subdomain" => "하위 도메인 모드",
        "server.gatewayHostResponse.unavailableReason" => {
            "하위 도메인 모드만 사용할 수 있습니다. 현재 모드: {mode}."
        }
        "server.gatewayHostResponse.editSubdomainOnly" => {
            "Host 응답은 하위 도메인 매핑 모드에서만 편집할 수 있습니다."
        }
        "server.gatewayHostResponse.updateFailedRolledBack" => {
            "게이트웨이 Host 응답을 업데이트하지 못했습니다. 구성이 롤백되었습니다."
        }
        "server.gatewayHostResponse.restoreConfigFailed" => "Host 응답 구성을 복원하지 못했습니다.",
        "server.gatewayHostResponse.restoreRuntimeFailed" => {
            "Host 응답 런타임 상태를 복원하지 못했습니다."
        }
        "server.gatewayHostResponse.restoreGatewayRuntimeFailed" => {
            "게이트웨이 Host 응답 런타임 상태를 복원하지 못했습니다."
        }
        "server.admin.rollback.failed" => "{message}; 롤백 실패: {rollbackError}",
        "server.admin.rollback.restoreVisibilityConfigFailed" => {
            "가시성 구성을 복원하지 못했습니다."
        }
        "server.admin.rollback.restoreVisibilityRuntimeFailed" => {
            "가시성 런타임 CIDR을 복원하지 못했습니다."
        }
        "server.admin.rollback.restoreGatewayVisibilityFailed" => {
            "게이트웨이 가시성 런타임 상태를 복원하지 못했습니다."
        }
        "server.admin.rollback.restoreProxyHeadersConfigFailed" => {
            "프록시 헤더 구성을 복원하지 못했습니다."
        }
        "server.admin.rollback.restoreProxyHeadersRuntimeFailed" => {
            "프록시 헤더 런타임 상태를 복원하지 못했습니다."
        }
        "server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed" => {
            "게이트웨이 프록시 헤더 런타임 상태를 복원하지 못했습니다."
        }
        "server.admin.gatewayVisibility.updateFailedRolledBack" => {
            "게이트웨이 공개 상태를 업데이트하지 못했습니다. 구성이 롤백되었습니다."
        }
        "server.admin.gatewayProxyHeaders.subdomainOnly" => {
            "프록시 헤더는 하위 도메인 매핑 모드에서만 편집할 수 있습니다."
        }
        "server.admin.gatewayProxyHeaders.updateFailedRolledBack" => {
            "게이트웨이 프록시 헤더를 업데이트하지 못했습니다. 구성이 롤백되었습니다."
        }
        "server.admin.hostMappings.bookmarkFolderForRoot" => "{root} 하위 도메인 매핑",
        "server.admin.hostMappings.bookmarkFolderDefault" => "fn-knock 하위 도메인 매핑",
        "server.whitelist.regionAddFailed" => "지역 화이트리스트를 추가하지 못했습니다.",
        "server.whitelist.regionRequired" => "지역을 하나 이상 선택하세요.",
        "server.whitelist.regionEmpty" => "선택한 지역에서 사용할 수 있는 CIDR을 찾지 못했습니다.",
        "server.whitelist.regionNotFound" => "지역 화이트리스트를 찾을 수 없습니다.",
        "server.scanDiscovery.selectAtLeastOneCidr" => {
            "로컬 IPv4 스캔 범위를 하나 이상 선택하세요."
        }
        "server.scanDiscovery.scanJobNotFound" => "스캔 작업을 찾을 수 없거나 만료되었습니다.",
        "server.dockerAdminPanel.resetHelp" => {
            "fn-knock 관리자 패널 비밀번호 재설정 도구\n\n사용법:\n  fn-knock-reset-panel-password\n\n작업:\n  - 관리자 패널 비밀번호 지우기\n  - 모든 관리자 패널 로그인 세션 지우기\n  - 로그인 실패 백오프 상태 지우기\n\n완료 후 다음에 관리자 항목을 방문하면 최초 비밀번호 설정 흐름이 다시 시작됩니다."
        }
        "server.dockerAdminPanel.resetCleared" => {
            "[fn-knock] 관리자 패널 비밀번호 상태가 지워졌습니다."
        }
        "server.dockerAdminPanel.resetNextVisit" => {
            "[fn-knock] 다음에 관리자 항목을 방문할 때 관리자 패널 비밀번호를 다시 설정하세요."
        }
        "server.dockerAdminPanel.resetFailed" => {
            "[fn-knock] 관리자 패널 비밀번호를 지우지 못했습니다:"
        }
        _ => return None,
    })
}

fn ja_jp_message(key: &str) -> Option<&'static str> {
    Some(match key {
        "auth.autoIpGrantComment" => "ログイン後自動認証",
        "server.systemClock.unknown" => "不明",
        "server.systemClock.actionSeparator" => ";",
        "server.systemClock.listSeparator" => "、",
        "server.systemClock.duration.seconds" => "{seconds} 秒",
        "server.systemClock.duration.minutes" => "{minutes} 分",
        "server.systemClock.duration.minutesSeconds" => "{minutes}分 {seconds}秒",
        "server.systemClock.networkTimeUnavailable" => {
            "ネットワークから標準時刻を取得できませんでした"
        }
        "server.systemClock.sourceFetchFailed" => "{source}からの時間を取得できませんでした",
        "server.systemClock.missingDateHeader" => {
            "{source} 利用可能な Date 応答ヘッダーが返されませんでした"
        }
        "server.systemClock.invalidDateHeader" => "{source} は解析できない時間を返しました",
        "server.systemClock.issues.timezone.title" => {
            "システムのタイムゾーンは北京時間ではありません"
        }
        "server.systemClock.issues.timezone.message" => {
            "現在のシステムのタイムゾーンは{timezone}で、{expected}に設定する必要があります。"
        }
        "server.systemClock.issues.timeMismatch.title" => {
            "システム時刻がネットワーク検証結果と一致しません"
        }
        "server.systemClock.issues.timeMismatch.message" => {
            "現在のシステム時刻とネットワーク検証結果の差は約{drift}です。"
        }
        "server.systemClock.statusRefreshed" => "システム時間ステータスが更新されました",
        "server.systemClock.timezoneSet" => "システムのタイムゾーンは {timezone} に設定されました",
        "server.systemClock.missingZoneinfoFile" => {
            "システムにタイムゾーンファイル {path} がありません"
        }
        "server.systemClock.timezoneWritten" => {
            "システムのタイムゾーン {timezone} を書き込みました"
        }
        "server.systemClock.clockAdjusted" => "システム時刻を校正しました",
        "server.systemClock.ntpEnabled" => "NTP 自動時刻修正を有効化しました",
        "server.systemClock.serviceRestarted" => "{service} サービスを再起動しました",
        "server.acmeRoutes.domainsInvalid" => {
            "ドメイン名リストを空にすることはできません、または形式が無効です。"
        }
        "server.acmeRoutes.dnsTypeRequired" => "DNS 検証タイプがありません",
        "server.acmeRoutes.unsupportedDnsProvider" => {
            "サポートされていません DNS サービスプロバイダー"
        }
        "server.acmeRoutes.missingDnsCredentials" => {
            "DNS API 認証情報がありません。次のオプションのいずれかを入力してください: {requirements}"
        }
        "server.acmeRoutes.installingRetryLater" => {
            "acme.sh をインストール中です。しばらくしてから再試行してください。"
        }
        "server.acmeRoutes.installFirst" => "先に acme.sh をインストールしてください",
        "server.acmeRoutes.multipleApplicationsUseNewApi" => {
            "複数の申請項目があります。新しい API で ACME 申請項目を管理してください。"
        }
        "server.acmeRoutes.applicationNotFound" => "申請項目が存在しません",
        "server.acmeRoutes.notFound" => "見つかりません",
        "server.acmeRoutes.installingCannotDelete" => {
            "acme.sh をインストール中のため削除できません"
        }
        "server.acmeRoutes.installingCannotSwitchCa" => {
            "acme.sh をインストール中です。まだ認証局を切り替えられません。"
        }
        "server.acmeRoutes.noMatchingIssuedCertificate" => {
            "この申請項目にはドメイン設定に一致する発行済み証明書がありません"
        }
        "server.acmeRoutes.success" => "成功",
        "server.acmeRoutes.dns01Only" => "DNS-01 検証のみサポートされています",
        "server.acmeRoutes.certNotFound" => "証明書が見つかりません",
        "server.acmeRoutes.certOrKeyInvalid" => "証明書または秘密鍵が無効です",
        "server.acme.alreadyInstalled" => "acme.sh はすでにインストールされています",
        "server.acme.installInProgress" => "インストールタスクがすでに進行中です",
        "server.acme.installSubmitted" => "インストールタスクが送信されました",
        "server.acme.issueSucceeded" => "証明書が正常に発行されました",
        "server.acmeService.waiting" => "操作待ち",
        "server.acmeService.ready" => "acme.sh は準備完了です",
        "server.acmeService.sendSignalFailed" => {
            "{target} への {signal} 送信に失敗しました: {detail}"
        }
        "server.acmeService.setDefaultCaFailed" => {
            "デフォルト認証局の設定に失敗しました（終了コード: {code}）{brief}"
        }
        "server.acmeService.registerAccountFailed" => {
            "ACME アカウント登録に失敗しました（終了コード: {code}）{brief}"
        }
        "server.acmeService.bundledZipMissing" => "内蔵 acmesh.zip リソースが見つかりません",
        "server.acmeService.extractingBundled" => "内蔵 acme.sh リソースを展開しています...",
        "server.acmeService.unzipFailed" => "展開に失敗しました。終了コード: {code}",
        "server.acmeService.extractedAcmeMissing" => {
            "展開は成功しましたが acme.sh が見つかりません"
        }
        "server.acmeService.writingDataDir" => "データディレクトリへ書き込み中...",
        "server.acmeService.writtenAcmeMissing" => "書き込み後に acme.sh が見つかりません",
        "server.acmeService.checkInstallFailed" => "インストール状態の確認に失敗しました: {detail}",
        "server.acmeService.notInstalled" => "acme.sh はインストールされていません",
        "server.acmeService.initializingBundled" => "内蔵 acme.sh を初期化しています...",
        "server.acmeService.registeringAccount" => "ACME アカウントを登録しています...",
        "server.acmeService.savingDefaultCa" => "デフォルト認証局を保存しています...",
        "server.acmeService.installSuccess" => "インストール成功、アカウントメール: {email}",
        "server.acmeService.installFailed" => "インストール失敗: {detail}",
        "server.acmeService.installFirst" => "先に acme.sh をインストールしてください",
        "server.acmeService.installingCannotDelete" => {
            "acme.sh をインストール中のため削除できません"
        }
        "server.acmeService.deleted" => "acme.sh を削除しました",
        "server.acmeService.deleteFailed" => "削除に失敗しました: {detail}",
        "server.acmeService.domainsRequired" => "ドメインリストは必須です",
        "server.acmeService.dnsTypeRequired" => "DNS 検証タイプがありません",
        "server.acmeService.issueFailed" => "証明書発行に失敗しました（終了コード: {code}）{brief}",
        "server.acmeJobRunner.manualStop" => "ACME タスクはユーザーにより手動停止されました",
        "server.acmeJobRunner.lockMessages.manualRequest" => "証明書を申請しています",
        "server.acmeJobRunner.lockMessages.autoRenew" => "証明書を自動更新しています",
        "server.acmeJobRunner.flowFailed" => "証明書申請フローに失敗しました: {message}",
        "server.acmeJobRunner.activeTaskRunning" => {
            "現在 ACME のタスクが実行中です。後でもう一度お試しください。"
        }
        "server.acmeJobRunner.applicationChangedSkipped" => {
            "実行中に申請項目のドメインが変更されたため、古い証明書の書き込みをスキップしました。再度申請してください。"
        }
        "server.acmeJobRunner.issuedButApplicationChanged" => {
            "証明書は発行されましたが、申請項目のドメインが変更されたため現在の申請項目には書き込みませんでした。"
        }
        "server.acmeJobRunner.issuedButCertReadFailed" => {
            "証明書は発行されましたが、証明書ファイルの読み取りに失敗しました。後で再試行するか acme.sh ディレクトリを確認してください。"
        }
        "server.acmeJobRunner.clearedDomainWorkingState" => {
            "acme.sh のドメイン作業ディレクトリをクリアしました。証明書一覧と更新はシステムタスクで管理されます。"
        }
        "server.acmeJobRunner.clearDomainWorkingStateFailed" => {
            "証明書は保存されましたが、acme.sh ドメイン状態のクリアに失敗しました: {message}"
        }
        "server.acmeJobRunner.linkedLibrarySyncedGateway" => {
            "リンク済みの証明書ライブラリ項目を同期し、ゲートウェイ証明書一覧を更新しました"
        }
        "server.acmeJobRunner.linkedLibraryUpdated" => {
            "リンク済みの証明書ライブラリ項目を更新しました"
        }
        "server.acmeJobRunner.addedToLibraryAndSyncedGateway" => {
            "証明書発行後に証明書ライブラリへ自動追加し、ゲートウェイ証明書一覧を更新しました"
        }
        "server.acmeJobRunner.addedToLibrary" => "証明書発行後に証明書ライブラリへ自動追加しました",
        "server.acmeJobRunner.addToLibraryFailed" => {
            "証明書は発行・保存されましたが、証明書ライブラリへの追加に失敗しました: {message}"
        }
        "server.acmeJobRunner.stoppedIgnoredProcessError" => {
            "タスクは停止済みのため、プロセス終了エラーを無視しました"
        }
        "server.store.acme.domainRequired" => "ドメインは必須です",
        "server.store.acme.domainsRequired" => "ドメインリストは必須です",
        "server.store.acme.dnsProviderRequired" => "DNS プロバイダーは必須です",
        "server.store.acme.primaryDomainDuplicated" => {
            "プライマリドメイン {primaryDomain} は別の申請項目に既に存在します"
        }
        "server.store.acme.applicationNotFound" => "申請項目が見つかりません",
        "server.store.acme.noMatchingIssuedCertificate" => {
            "この申請項目にはドメイン設定に一致する発行済み証明書がありません"
        }
        "server.store.ssl.certNotFound" => "証明書が見つかりません",
        "server.store.ssl.certOrKeyInvalid" => "証明書または秘密鍵が無効です",
        "server.acmeDnsProviders.groups.common" => "よく使う",
        "server.acmeDnsProviders.groups.domestic" => "中国",
        "server.acmeDnsProviders.groups.international" => "国際",
        "server.acmeDnsProviders.groups.selfHostedAdvanced" => "セルフホスト / 高度",
        "server.acmeDnsProviders.credentialSchemes.default" => "デフォルト認証情報",
        "server.acmeDnsProviders.fields.accountEmail" => "アカウントメール",
        "server.acmeDnsProviders.labels.aliyun" => "Alibaba Cloud DNS",
        "server.acmeDnsProviders.labels.tencentCloudDnspod" => {
            "Tencent Cloud DNSPod (TencentCloud)"
        }
        "server.acmeDnsProviders.labels.huaweiCloudDns" => "Huawei Cloud DNS",
        "server.acmeDnsProviders.labels.jdCloudDns" => "JD Cloud DNS",
        "server.acmeDnsProviders.labels.westCn" => "West.cn",
        "server.acmeDnsProviders.requirements.optionalSuffix" => "；任意 {keys}",
        "server.acmeDnsProviders.requirements.orSeparator" => "；または ",
        "server.acmePatches.duckdns.scriptMissing" => {
            "DuckDNS DNS API スクリプトが見つかりません: {path}"
        }
        "server.acmePatches.duckdns.proxyApplied" => {
            "{from} から {to} に DuckDNS API を切り替えました"
        }
        "server.subdomainMode.recommendationMissingBase" => {
            "ルートドメインまたは認証サービスが未設定のため、推奨証明書ドメインをまだ生成できません。"
        }
        "server.subdomainMode.recommendationWildcardSummary" => {
            "推奨ドメイン: {rootDomain} と *.{rootDomain}。ルートドメイン、認証サービス、同じ親ドメイン配下の業務サブドメインをカバーします。"
        }
        "server.subdomainMode.authOutOfRootWarning" => {
            "現在の認証サービス {authHost} はルートドメイン {rootDomain} 配下ではないため、正確なドメインを別途追加しました。選択した DNS プロバイダーがこれらのドメインを管理できることを確認してください。"
        }
        "server.subdomainMode.recommendationSingleHostSummary" => {
            "ルートドメインが未設定のため、認証サービス {authHost} の単一ドメイン証明書のみ推奨できます。"
        }
        "server.subdomainMode.wildcardSuggestion" => {
            "後で複数の業務サブドメインをまとめてカバーする場合は、wildcard 証明書を申請する前にルートドメインを追加してください。"
        }
        "server.subdomainMode.configureRootOrAuth" => {
            "先にサブドメインモードでルートドメインを設定するか、Host マッピングで認証サービスを指定してください。"
        }
        "server.subdomainMode.authMissingWarning" => {
            "認証サービスが指定されていないため、推奨結果はルートドメインのみから算出されます。"
        }
        "server.subdomainMode.uncoveredHostMappingsWarning" => {
            "{count} 個の Host マッピングは推奨証明書の範囲外です。外部公開が必要な場合は証明書を追加するかドメイン計画を調整してください。"
        }
        "server.gatewayVisibility.customCidrInvalid" => {
            "カスタム CIDR の形式が正しくありません: {cidrs}"
        }
        "server.gatewayVisibility.emptyEnabledConfig" => {
            "可視性を有効にした後、少なくとも 1 つの地域またはカスタム CIDR を追加する必要があります"
        }
        "server.gatewayVisibility.syncFailed" => "ゲートウェイ可視性設定の同期に失敗しました",
        "server.gatewayProxyHeaders.runTypes.direct" => "ダイレクトモード",
        "server.gatewayProxyHeaders.runTypes.reverseProxy" => "リバースプロキシモード",
        "server.gatewayProxyHeaders.runTypes.subdomain" => "サブドメインモード",
        "server.gatewayProxyHeaders.unavailableReason" => {
            "サブドメインモードのみ利用できます。現在のモード: {mode}。"
        }
        "server.gatewayProxyHeaders.syncFailed" => {
            "ゲートウェイプロキシヘッダー設定の同期に失敗しました"
        }
        "server.gatewayHostResponse.runTypes.direct" => "ダイレクトモード",
        "server.gatewayHostResponse.runTypes.reverseProxy" => "リバースプロキシモード",
        "server.gatewayHostResponse.runTypes.subdomain" => "サブドメインモード",
        "server.gatewayHostResponse.unavailableReason" => {
            "サブドメインモードのみ利用できます。現在のモード: {mode}。"
        }
        "server.gatewayHostResponse.editSubdomainOnly" => {
            "Host 応答はサブドメインマッピングモードでのみ編集できます"
        }
        "server.gatewayHostResponse.updateFailedRolledBack" => {
            "ゲートウェイ Host 応答の更新に失敗しました。設定はロールバックされました。"
        }
        "server.gatewayHostResponse.restoreConfigFailed" => "Host 応答設定の復元に失敗しました",
        "server.gatewayHostResponse.restoreRuntimeFailed" => {
            "Host 応答ランタイム状態の復元に失敗しました"
        }
        "server.gatewayHostResponse.restoreGatewayRuntimeFailed" => {
            "ゲートウェイ Host 応答ランタイム状態の復元に失敗しました"
        }
        "server.admin.rollback.failed" => "{message}; ロールバックに失敗しました: {rollbackError}",
        "server.admin.rollback.restoreVisibilityConfigFailed" => "可視性設定の復元に失敗しました",
        "server.admin.rollback.restoreVisibilityRuntimeFailed" => {
            "可視性ランタイム CIDR の復元に失敗しました"
        }
        "server.admin.rollback.restoreGatewayVisibilityFailed" => {
            "ゲートウェイ可視性ランタイム状態の復元に失敗しました"
        }
        "server.admin.rollback.restoreProxyHeadersConfigFailed" => {
            "プロキシヘッダー設定の復元に失敗しました"
        }
        "server.admin.rollback.restoreProxyHeadersRuntimeFailed" => {
            "プロキシヘッダーランタイム状態の復元に失敗しました"
        }
        "server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed" => {
            "ゲートウェイプロキシヘッダーランタイム状態の復元に失敗しました"
        }
        "server.admin.gatewayVisibility.updateFailedRolledBack" => {
            "ゲートウェイの可視性を更新できませんでした。設定はロールバックされました。"
        }
        "server.admin.gatewayProxyHeaders.subdomainOnly" => {
            "プロキシヘッダーはサブドメインマッピングモードでのみ編集できます"
        }
        "server.admin.gatewayProxyHeaders.updateFailedRolledBack" => {
            "ゲートウェイプロキシヘッダーの更新に失敗しました。設定はロールバックされました。"
        }
        "server.admin.hostMappings.bookmarkFolderForRoot" => "{root} サブドメインマッピング",
        "server.admin.hostMappings.bookmarkFolderDefault" => "fn-knock サブドメインマッピング",
        "server.whitelist.regionAddFailed" => "地域ホワイトリストの追加に失敗しました",
        "server.whitelist.regionRequired" => "少なくとも 1 つの地域を選択してください",
        "server.whitelist.regionEmpty" => "選択した地域で使用可能な CIDR が見つかりませんでした",
        "server.whitelist.regionNotFound" => "地域ホワイトリストが見つかりませんでした",
        "server.scanDiscovery.selectAtLeastOneCidr" => {
            "少なくとも 1 つのローカル IPv4 スキャン範囲を選択してください"
        }
        "server.scanDiscovery.scanJobNotFound" => {
            "スキャンジョブが見つからないか、有効期限が切れています"
        }
        "server.dockerAdminPanel.resetHelp" => {
            "fn-knock 管理パネルパスワードリセットツール\n\n使用法:\n  fn-knock-reset-panel-password\n\n機能:\n  - 管理パネルのパスワードをクリア\n  - すべての管理パネルのログインセッションをクリア\n  - ログイン失敗バックオフ状態をクリア\n\n完了後、次回管理入口へアクセスすると初回パスワード設定フローが再開されます。"
        }
        "server.dockerAdminPanel.resetCleared" => {
            "[fn-knock] 管理パネルのパスワード状態をクリアしました"
        }
        "server.dockerAdminPanel.resetNextVisit" => {
            "[fn-knock] 次回管理入口にアクセスするときに管理パネルのパスワードを再設定してください"
        }
        "server.dockerAdminPanel.resetFailed" => {
            "[fn-knock] 管理パネルのパスワードをクリアできませんでした:"
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
