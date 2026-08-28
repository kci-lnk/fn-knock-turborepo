use serde_json::{Map, Value, json};

struct TagDocumentation {
    name: &'static str,
    title: &'static str,
    description: &'static str,
}

const TAGS: &[TagDocumentation] = &[
    TagDocumentation {
        name: "acme",
        title: "ACME 证书管理",
        description: "管理 ACME 客户端、证书申请、部署和运行任务。",
    },
    TagDocumentation {
        name: "auth",
        title: "认证与账户",
        description: "管理管理端认证模式、账户、凭据和访问范围。",
    },
    TagDocumentation {
        name: "auth-ldap",
        title: "LDAP 认证",
        description: "管理 LDAP 身份提供者、邀请和账户绑定。",
    },
    TagDocumentation {
        name: "auth-oidc",
        title: "OIDC 认证",
        description: "管理 OIDC/OAuth 身份提供者、邀请和账户绑定。",
    },
    TagDocumentation {
        name: "backoff",
        title: "登录退避",
        description: "查看和重置登录失败后的退避与封锁状态。",
    },
    TagDocumentation {
        name: "cidr",
        title: "CIDR 地域选择",
        description: "提供 CIDR 访问控制所需的地域、城市和运营商数据。",
    },
    TagDocumentation {
        name: "cloudflared",
        title: "Cloudflare 隧道",
        description: "管理 Cloudflare Tunnel 进程、凭据、配置、协调任务和优选。",
    },
    TagDocumentation {
        name: "config",
        title: "系统配置",
        description: "读取和更新管理端、网关、安全及平台功能的配置。",
    },
    TagDocumentation {
        name: "configuration",
        title: "代理与映射配置",
        description: "管理主机映射、反向代理、流映射和子域模式。",
    },
    TagDocumentation {
        name: "dashboard",
        title: "仪表盘",
        description: "读取管理端仪表盘的实时统计、流量和活跃 IP 数据。",
    },
    TagDocumentation {
        name: "ddns",
        title: "动态 DNS",
        description: "管理 DDNS 提供者、目标、网络接口、日志和任务状态。",
    },
    TagDocumentation {
        name: "deep-monitor",
        title: "深度监控",
        description: "创建和检查深度监控会话、捕获事件及其归档数据。",
    },
    TagDocumentation {
        name: "events",
        title: "系统事件",
        description: "查询、清理和维护管理端系统事件记录。",
    },
    TagDocumentation {
        name: "traces",
        title: "全链路追踪",
        description: "按统一 Trace ID 聚合网关请求、WAF、系统事件和通知投递记录。",
    },
    TagDocumentation {
        name: "firewall",
        title: "防火墙",
        description: "执行防火墙规则清理、重置和相关运行时操作。",
    },
    TagDocumentation {
        name: "frpc",
        title: "FRPC 隧道",
        description: "管理 FRPC 配置、实例、运行状态和日志。",
    },
    TagDocumentation {
        name: "gateway-logs",
        title: "网关日志",
        description: "配置、检索和分析网关访问日志。",
    },
    TagDocumentation {
        name: "general-blacklist",
        title: "通用黑名单",
        description: "维护全局 IP 黑名单及其运行状态。",
    },
    TagDocumentation {
        name: "health",
        title: "服务健康检查",
        description: "探测管理端服务是否可用。",
    },
    TagDocumentation {
        name: "ip-location",
        title: "IP 归属地",
        description: "批量查询 IP 地址归属地信息。",
    },
    TagDocumentation {
        name: "maintenance",
        title: "备份与维护",
        description: "管理备份导入导出、自动备份和受确认保护的数据维护。",
    },
    TagDocumentation {
        name: "notifications",
        title: "通知中心",
        description: "管理通知提供者、规则、触发记录和投递记录。",
    },
    TagDocumentation {
        name: "panel-sync",
        title: "NAS 面板同步",
        description: "管理 Sun-Panel、OneNav 与 Van Nav 的单向链接同步。",
    },
    TagDocumentation {
        name: "panel",
        title: "管理面板会话",
        description: "管理 Docker 管理面板的初始化、登录、密码和会话状态。",
    },
    TagDocumentation {
        name: "passkeys",
        title: "通行密钥",
        description: "管理账户绑定的 Passkey 凭据。",
    },
    TagDocumentation {
        name: "runtime-health",
        title: "运行时健康",
        description: "检查服务组件健康、诊断信息、日志和网关内存状态。",
    },
    TagDocumentation {
        name: "scan",
        title: "资产扫描",
        description: "配置并运行网络资产发现和扫描任务。",
    },
    TagDocumentation {
        name: "scanner",
        title: "扫描器",
        description: "管理扫描器设置、结果和黑名单记录。",
    },
    TagDocumentation {
        name: "security",
        title: "安全概览",
        description: "读取管理端安全能力与当前保护状态。",
    },
    TagDocumentation {
        name: "sessions",
        title: "会话管理",
        description: "查询、撤销和维护认证会话及其移动性信息。",
    },
    TagDocumentation {
        name: "ssh-security",
        title: "SSH 安全",
        description: "管理 SSH 防护配置、登录记录和封锁记录。",
    },
    TagDocumentation {
        name: "system",
        title: "系统服务",
        description: "管理系统资源、时钟、二进制组件和运行时服务。",
    },
    TagDocumentation {
        name: "system-events",
        title: "内部系统事件",
        description: "供内部组件发布系统事件的接口。",
    },
    TagDocumentation {
        name: "terminal",
        title: "Web 终端",
        description: "管理 Web 终端运行时能力和交互会话。",
    },
    TagDocumentation {
        name: "totp",
        title: "TOTP 凭据",
        description: "管理 TOTP 凭据、访问范围、导入导出和 Passkey 绑定。",
    },
    TagDocumentation {
        name: "update",
        title: "系统更新",
        description: "检查、下载、确认和应用系统更新。",
    },
    TagDocumentation {
        name: "waf",
        title: "Web 应用防火墙",
        description: "管理 WAF 配置、规则文件、事件和日志。",
    },
    TagDocumentation {
        name: "whitelist",
        title: "白名单",
        description: "维护访问白名单、地区分组和域名解析记录。",
    },
    TagDocumentation {
        name: "wol",
        title: "网络唤醒",
        description: "管理 Wake-on-LAN 中继、目标设备和集成状态。",
    },
];

pub(super) fn tags() -> Vec<Value> {
    TAGS.iter()
        .map(|documentation| {
            json!({
                "name": documentation.name,
                "description": documentation.description
            })
        })
        .collect()
}

pub(super) fn apply(paths: &mut Map<String, Value>) {
    for (path, path_item) in paths {
        let Some(path_item) = path_item.as_object_mut() else {
            continue;
        };
        for (method, operation) in path_item {
            if !matches!(method.as_str(), "get" | "post" | "put" | "patch" | "delete") {
                continue;
            }
            let Some(operation) = operation.as_object_mut() else {
                continue;
            };
            let Some(tag) = operation
                .get("tags")
                .and_then(Value::as_array)
                .and_then(|tags| tags.first())
                .and_then(Value::as_str)
            else {
                continue;
            };
            if tag == "ssl" {
                continue;
            }
            let Some(documentation) = tag_documentation(tag) else {
                continue;
            };

            let summary = operation_summary(method, path, documentation);
            if operation
                .get("summary")
                .and_then(Value::as_str)
                .is_none_or(|current| {
                    current.trim().is_empty() || is_generated_summary(current, method, path)
                })
            {
                operation.insert("summary".to_string(), Value::String(summary));
            }
            if operation
                .get("description")
                .and_then(Value::as_str)
                .is_none_or(|current| current.trim().is_empty())
            {
                operation.insert(
                    "description".to_string(),
                    Value::String(operation_description(
                        method,
                        path,
                        operation,
                        documentation,
                    )),
                );
            }
            apply_response_descriptions(operation, method, path, documentation);
        }
    }
}

fn tag_documentation(tag: &str) -> Option<&'static TagDocumentation> {
    TAGS.iter().find(|documentation| documentation.name == tag)
}

fn is_generated_summary(summary: &str, method: &str, path: &str) -> bool {
    summary == format!("{} {path}", method.to_ascii_uppercase())
}

fn operation_summary(method: &str, path: &str, documentation: &TagDocumentation) -> String {
    let (subject, action) = operation_subject(path, documentation);
    if let Some(action) = action {
        return format!("{action}{subject}");
    }
    let verb = match method {
        "get" => "查看",
        "post" => "提交",
        "put" => "更新",
        "patch" => "修改",
        "delete" => "删除",
        _ => "处理",
    };
    format!("{verb}{subject}")
}

fn operation_description(
    method: &str,
    path: &str,
    operation: &Map<String, Value>,
    documentation: &TagDocumentation,
) -> String {
    let behavior = match method {
        "get" => "用于读取当前状态、配置或导出内容，不会主动修改服务配置。",
        "post" => "用于提交操作或创建、更新服务状态；执行结果以响应中的数据和消息为准。",
        "put" => "用于提交完整更新；未提供的字段是否保留以该接口的请求 schema 为准。",
        "patch" => "用于对已有资源进行局部更新；仅提交需要变更的字段。",
        "delete" => "用于删除、清理或撤销资源。执行前请确认目标和可能不可恢复的影响。",
        _ => "用于管理该模块的服务资源。",
    };
    let request_note = if operation.get("requestBody").is_some() {
        "请求体字段、必填项和可选值请以 Swagger 展开的 schema 为准。"
    } else {
        "该操作不要求 JSON 请求体。"
    };
    format!(
        "{}。`{} {path}` {} {} {}",
        documentation.description,
        method.to_ascii_uppercase(),
        behavior,
        request_note,
        response_note(operation)
    )
}

fn apply_response_descriptions(
    operation: &mut Map<String, Value>,
    method: &str,
    path: &str,
    documentation: &TagDocumentation,
) {
    let summary = operation_summary(method, path, documentation);
    let Some(responses) = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    for (status, response) in responses {
        let Some(response) = response.as_object_mut() else {
            continue;
        };
        let should_replace = response
            .get("description")
            .and_then(Value::as_str)
            .is_none_or(|description| !contains_chinese(description));
        if should_replace {
            response.insert(
                "description".to_string(),
                Value::String(response_description(status, response, &summary)),
            );
        }
    }
}

fn response_description(status: &str, response: &Map<String, Value>, summary: &str) -> String {
    match status {
        "200" | "201" => success_response_description(response, summary),
        "202" => format!("已受理「{summary}」请求；请按响应中的任务或状态信息继续查询。"),
        "204" => format!("「{summary}」已成功完成，不返回响应正文。"),
        "400" => "请求参数或当前资源状态不符合接口要求；请检查必填字段和前置条件。".to_string(),
        "401" => "当前会话未通过认证或已失效；请先登录后重试。".to_string(),
        "403" => "当前会话无权执行此操作，或目标资源受安全策略限制。".to_string(),
        "404" => "请求的资源或关联配置不存在，或当前不可用。".to_string(),
        "409" => "当前资源状态与操作冲突；请刷新状态后重试。".to_string(),
        "422" => "请求内容无法通过业务校验；请根据错误消息修正后重试。".to_string(),
        "429" => "请求过于频繁；请遵循响应提示的退避时间后重试。".to_string(),
        "default" => {
            "接口处理失败时返回标准错误信封；请结合 HTTP 状态、错误消息和服务日志排查。".to_string()
        }
        status if status.starts_with('5') => {
            "服务处理失败；请检查依赖服务和运行日志后重试。".to_string()
        }
        _ => format!("「{summary}」未成功完成；请根据 HTTP 状态和错误消息处理。"),
    }
}

fn success_response_description(response: &Map<String, Value>, summary: &str) -> String {
    let media_types = response
        .get("content")
        .and_then(Value::as_object)
        .map(|content| content.keys().map(String::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if media_types.contains(&"application/zip") {
        return format!(
            "「{summary}」成功，返回 ZIP 下载附件；请从 Content-Disposition 获取文件名。"
        );
    }
    if media_types.contains(&"application/x-pem-file") {
        return format!(
            "「{summary}」成功，返回 PEM 下载附件；请从 Content-Disposition 获取文件名。"
        );
    }
    if media_types.contains(&"application/octet-stream") {
        return format!(
            "「{summary}」成功，返回二进制下载附件；请从 Content-Disposition 获取文件名。"
        );
    }
    if media_types.contains(&"text/event-stream") {
        return format!("「{summary}」成功，返回实时事件流；连接中断后需要重新订阅。");
    }
    if media_types.contains(&"text/html") {
        return format!("「{summary}」成功，返回 HTML 内容。");
    }
    format!("「{summary}」成功，返回标准管理端 JSON 信封；具体 data 结构请查看响应 schema。")
}

fn contains_chinese(value: &str) -> bool {
    !value.is_ascii()
}

fn response_note(operation: &Map<String, Value>) -> &'static str {
    let Some(content) = operation
        .get("responses")
        .and_then(|responses| responses.get("200"))
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
    else {
        return "成功响应的具体格式请以接口定义为准。";
    };
    if content
        .keys()
        .any(|media_type| media_type != "application/json")
    {
        "成功时返回附件或其他非 JSON 内容，媒体类型和下载文件名请查看响应定义。"
    } else {
        "成功响应通常使用标准管理端 JSON 信封，具体 `data` 结构请查看响应 schema。"
    }
}

fn operation_subject(
    path: &str,
    documentation: &TagDocumentation,
) -> (String, Option<&'static str>) {
    let path = path
        .strip_prefix("/api/admin/")
        .or_else(|| path.strip_prefix("/api/internal/"))
        .unwrap_or(path);
    let mut segments = path
        .split('/')
        .skip(1)
        .filter(|segment| !segment.starts_with('{'))
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let action = segments.last().and_then(|segment| action_label(segment));
    if action.is_some() {
        segments.pop();
    }
    let resource = segments
        .last()
        .map(|segment| segment_label(segment))
        .filter(|segment| !segment.is_empty());
    let subject = resource
        .map(|resource| format!("{}{}", documentation.title, resource))
        .unwrap_or_else(|| documentation.title.to_string());
    (subject, action)
}

fn action_label(segment: &str) -> Option<&'static str> {
    match segment {
        "activate" => Some("激活"),
        "apply" => Some("应用"),
        "bind" => Some("绑定"),
        "cancel" => Some("取消"),
        "check" => Some("检查"),
        "check-and-download" => Some("检查并下载"),
        "change" => Some("修改"),
        "clear" => Some("清空"),
        "complete" => Some("完成"),
        "control" => Some("接管"),
        "deploy" => Some("部署"),
        "drain" => Some("排空"),
        "download" => Some("下载"),
        "extend" => Some("延长"),
        "export" => Some("导出"),
        "import" => Some("导入"),
        "input" => Some("输入"),
        "init" | "initialize" => Some("初始化"),
        "install" => Some("安装"),
        "issue" => Some("签发"),
        "login" => Some("登录"),
        "logout" => Some("退出"),
        "pair" => Some("配对"),
        "poll" => Some("轮询"),
        "preview" => Some("预览"),
        "probe" => Some("探测"),
        "probe-host-key" => Some("探测主机密钥"),
        "reclaim" => Some("释放"),
        "refresh" => Some("刷新"),
        "reset" => Some("重置"),
        "restart" => Some("重启"),
        "resize" => Some("调整大小"),
        "resolve" => Some("解析"),
        "rotate-psk" => Some("轮换预共享密钥"),
        "setup" => Some("设置"),
        "shutdown" => Some("关机"),
        "start" => Some("启动"),
        "stop" => Some("停止"),
        "switch" => Some("切换"),
        "sync" => Some("同步"),
        "test" => Some("测试"),
        "test-connection" => Some("测试连接"),
        "toggle" => Some("切换"),
        "upload" => Some("上传"),
        "wake" => Some("唤醒"),
        _ => None,
    }
}

fn segment_label(segment: &str) -> String {
    match segment {
        "access-entry" => "访问入口".to_string(),
        "access-scopes" => "访问范围".to_string(),
        "accounts" => "账户".to_string(),
        "active" => "活动任务".to_string(),
        "active-ips" => "活跃 IP".to_string(),
        "stream-active-ips" => "流映射活跃 IP".to_string(),
        "advanced_auth" => "高级认证".to_string(),
        "analytics" => "分析".to_string(),
        "appearance" => "外观".to_string(),
        "applications" => "应用".to_string(),
        "archive" => "归档".to_string(),
        "attachments" => "附件".to_string(),
        "auth_credential_settings" => "认证凭据设置".to_string(),
        "auto_https" => "自动 HTTPS".to_string(),
        "auto_manage_firewall" => "自动管理防火墙".to_string(),
        "automatic" => "自动备份".to_string(),
        "backups" | "backup" => "备份".to_string(),
        "basic_auth_probe" => "基础认证探测".to_string(),
        "batch" => "批量查询".to_string(),
        "blacklist" => "黑名单".to_string(),
        "bindings" => "绑定".to_string(),
        "bookmarks" => "书签".to_string(),
        "blocks" => "封锁记录".to_string(),
        "bootstrap" => "引导初始化".to_string(),
        "by-plan" => "按计划筛选".to_string(),
        "capabilities" => "能力".to_string(),
        "connections" => "面板连接".to_string(),
        "catalog" => "目录".to_string(),
        "captcha" => "验证码".to_string(),
        "certificate" => "证书".to_string(),
        "certificates" => "证书".to_string(),
        "certs" => "证书".to_string(),
        "cidrs" => "CIDR 列表".to_string(),
        "cities" => "城市".to_string(),
        "client-settings" => "客户端设置".to_string(),
        "clock" => "系统时钟".to_string(),
        "cloudflared" => "Cloudflare 组件".to_string(),
        "comment" => "备注".to_string(),
        "confirm" => "确认信息".to_string(),
        "config" => "配置".to_string(),
        "custom" => "自定义规则文件".to_string(),
        "dashboard_display" => "仪表盘显示".to_string(),
        "data" => "数据".to_string(),
        "dates" => "日期范围".to_string(),
        "default_route" => "默认路由".to_string(),
        "default_tunnel" => "默认隧道".to_string(),
        "credential" | "credentials" => "凭据".to_string(),
        "deliveries" => "投递记录".to_string(),
        "details" => "详情".to_string(),
        "diagnostics" => "诊断信息".to_string(),
        "directory" => "目录".to_string(),
        "dnsmasq" => "DNS 转发组件".to_string(),
        "discover-settings" => "发现设置".to_string(),
        "discover-targets" => "发现目标".to_string(),
        "dns-providers" => "DNS 提供者".to_string(),
        "domains" => "域名".to_string(),
        "draft" => "草稿配置".to_string(),
        "entries" => "日志条目".to_string(),
        "enabled" => "启用状态".to_string(),
        "events" => "事件".to_string(),
        "fallback" => "备用配置".to_string(),
        "files" => "文件".to_string(),
        "firewall" => "防火墙".to_string(),
        "firewall_additional_ports" => "防火墙附加端口".to_string(),
        "false-positive" => "误报反馈".to_string(),
        "fnos" => "飞牛 OS".to_string(),
        "fnos_certificate_sync" => "飞牛证书同步".to_string(),
        "fnos_connect_waf" => "飞牛 WAF 连接".to_string(),
        "fnos_network_tuning" => "飞牛网络调优".to_string(),
        "fnos_port_icon_hijack" => "飞牛端口图标接管".to_string(),
        "fnos_share_bypass" => "飞牛共享绕过".to_string(),
        "frp" => "FRP 组件".to_string(),
        "gateway" => "网关".to_string(),
        "gateway-memory" => "网关内存".to_string(),
        "health" => "健康状态".to_string(),
        "host-response" => "主机响应".to_string(),
        "host_mapping_catalog" => "主机映射目录".to_string(),
        "host_mappings" => "主机映射".to_string(),
        "hosts" => "主机".to_string(),
        "host-mappings" => "主机映射".to_string(),
        "interfaces" => "网络接口".to_string(),
        "instances" => "实例".to_string(),
        "invitations" => "邀请".to_string(),
        "ip_location_api" => "IP 归属地服务".to_string(),
        "jobs" => "任务".to_string(),
        "library" => "证书库".to_string(),
        "list" => "列表".to_string(),
        "live" => "实时状态".to_string(),
        "local-relay" => "本地中继".to_string(),
        "locale" => "语言区域".to_string(),
        "login-logs" => "登录记录".to_string(),
        "logs" => "日志".to_string(),
        "manifest" => "规则清单".to_string(),
        "metadata" => "元数据".to_string(),
        "mode" => "模式".to_string(),
        "mobility" => "会话迁移".to_string(),
        "optimization" => "优选配置".to_string(),
        "overview" => "概览".to_string(),
        "password" => "密码".to_string(),
        "path-whitelist" => "路径白名单".to_string(),
        "passkeys" => "通行密钥".to_string(),
        "payload" => "事件载荷".to_string(),
        "provider" => "提供者".to_string(),
        "providers" => "提供者".to_string(),
        "protocol_mapping_feature" => "协议映射功能".to_string(),
        "provinces" => "省份".to_string(),
        "proxy-headers" => "代理请求头".to_string(),
        "proxy-protocol" => "PROXY 协议".to_string(),
        "proxy_mappings" => "代理映射".to_string(),
        "proxy_protocol_force" => "强制 PROXY 协议".to_string(),
        "public-check" => "公网检测".to_string(),
        "realtime" => "实时数据".to_string(),
        "recommended" => "推荐规则".to_string(),
        "reconcile" => "协调任务".to_string(),
        "refresh_titles" => "标题刷新".to_string(),
        "regions" => "地区分组".to_string(),
        "relays" => "中继".to_string(),
        "request" => "申请".to_string(),
        "resource" => "资源".to_string(),
        "rules" => "规则".to_string(),
        "runs" => "同步记录".to_string(),
        "run_mode_prompt_preferences" => "运行模式提示偏好".to_string(),
        "run_type" => "运行类型".to_string(),
        "scans" => "扫描任务".to_string(),
        "selector" => "选择器".to_string(),
        "sessions" => "会话".to_string(),
        "settings" => "设置".to_string(),
        "smart_connect" => "智能连接".to_string(),
        "ssh" => "SSH".to_string(),
        "state" => "运行状态".to_string(),
        "status" => "状态".to_string(),
        "stats" => "统计数据".to_string(),
        "stream_mappings" => "流映射".to_string(),
        "subdomain-access" => "子域访问范围".to_string(),
        "subdomain-recommendation" => "子域建议".to_string(),
        "subdomain_mode" => "子域模式".to_string(),
        "system" => "系统规则".to_string(),
        "targets" => "目标".to_string(),
        "test-cidr" => "CIDR 测试".to_string(),
        "test-ip-lookup" => "IP 查询测试".to_string(),
        "totp" => "TOTP 凭据".to_string(),
        "triggers" => "触发记录".to_string(),
        "users" => "用户".to_string(),
        "visibility" => "可见性".to_string(),
        "whitelist" => "白名单".to_string(),
        "web-status" => "Web 状态".to_string(),
        "wol_feature" => "网络唤醒功能".to_string(),
        _ => segment.replace(['-', '_'], " "),
    }
}
