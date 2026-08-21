use serde_json::{Map, Value, json};

struct ErrorDocumentation {
    status: &'static str,
    description: &'static str,
}

struct OperationDocumentation {
    method: &'static str,
    path: &'static str,
    summary: &'static str,
    description: &'static str,
    success_description: &'static str,
    errors: &'static [ErrorDocumentation],
}

const BAD_REQUEST: ErrorDocumentation = ErrorDocumentation {
    status: "400",
    description: "请求参数不符合 SSL 操作的前置条件，例如证书与私钥无效、主机名为空或本地 CA 主机列表为空。",
};
const UNAUTHORIZED: ErrorDocumentation = ErrorDocumentation {
    status: "401",
    description: "缺少绑定专用 Bearer Token，或 Token 已轮换、已撤销或不属于当前绑定。",
};
const FORBIDDEN: ErrorDocumentation = ErrorDocumentation {
    status: "403",
    description: "共享目录中的目标文件存在，但当前进程没有读取权限。",
};
const NOT_FOUND: ErrorDocumentation = ErrorDocumentation {
    status: "404",
    description: "请求的证书、CA 文件、共享文件或证书库记录不存在。",
};
const CONFLICT: ErrorDocumentation = ErrorDocumentation {
    status: "409",
    description: "新证书的到期时间早于当前槽位中的证书，或 SSL 配置在部署期间发生并发变更；客户端应检查证书并重试。",
};
const PAYLOAD_TOO_LARGE: ErrorDocumentation = ErrorDocumentation {
    status: "413",
    description: "证书部署请求超过 1 MiB 限制。",
};
const INTERNAL_ERROR: ErrorDocumentation = ErrorDocumentation {
    status: "500",
    description: "SSL 配置、证书处理、本地 CA 或网关同步失败；写操作可能已经保存本地变更，具体以后续状态查询为准。",
};
const GATEWAY_SYNC_ERROR: ErrorDocumentation = ErrorDocumentation {
    status: "500",
    description: "本地 SSL 配置无法同步到网关。部署模式切换会恢复先前配置；其他写操作请通过状态接口确认最终部署结果。",
};
const BAD_GATEWAY: ErrorDocumentation = ErrorDocumentation {
    status: "502",
    description: "新证书无法下发到网关；fn-knock 会尝试恢复旧配置和旧网关证书，并在无法确认恢复时明确返回该状态，证书客户端应将本次部署标记为失败并重试。",
};
const LAN_CONFLICT: ErrorDocumentation = ErrorDocumentation {
    status: "409",
    description: "网关仅监听回环地址，或当前没有可复用的激活 SSL 证书；修正前置配置后才能启用局域网入口。",
};
const LAN_BAD_GATEWAY: ErrorDocumentation = ErrorDocumentation {
    status: "502",
    description: "无法读取或更新 Go 网关状态；服务会恢复之前的局域网配置，并在无法确认恢复时返回明确错误。",
};

const OPERATIONS: &[OperationDocumentation] = &[
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/status",
        summary: "查看 SSL 证书库与部署状态",
        description: "汇总本地证书库、当前激活证书、域名覆盖度和网关部署状态。`deploymentMode` 是当前生效模式，`configuredDeploymentMode` 是本地配置；网关暂时不可达时会在 `gateway_status.sync_error` 中返回原因。",
        success_description: "返回 SSL 证书库、覆盖分析和网关部署快照。",
        errors: &[INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/shared-files",
        summary: "列出 SSL 共享目录中的候选文件",
        description: "扫描已配置的 SSL 共享目录，最多返回 500 个文件、递归深度不超过 3 层。目录未配置或不可用时仍返回成功响应，并以 `available=false` 表示不可导入。",
        success_description: "返回共享目录可用性及可读取的候选文件元数据。",
        errors: &[],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/shared-files/content",
        summary: "读取 SSL 共享文件内容",
        description: "读取共享目录中的单个文本文件，用于在导入前检查 PEM 内容。`path` 必须是共享根目录下的相对路径；绝对路径、目录穿越、目录本身和超过 512 KiB 的文件都会被拒绝。返回内容可能包含私钥，请仅在受信任环境中使用。",
        success_description: "返回文件元数据及 UTF-8 文本内容。",
        errors: &[BAD_REQUEST, FORBIDDEN, NOT_FOUND],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/cert.pem",
        summary: "下载当前激活证书的 PEM",
        description: "下载当前激活证书的公开 PEM 链，不包含私钥。响应以附件形式返回，文件名由 `Content-Disposition` 指定。",
        success_description: "返回当前激活证书的 PEM 附件。",
        errors: &[NOT_FOUND, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/cert.zip",
        summary: "下载当前激活证书和私钥 ZIP",
        description: "下载当前激活证书与匹配私钥的 ZIP 包，包含 `server-cert.pem` 和 `server-key.pem`。该附件含有私钥，下载后必须按敏感凭据保管。",
        success_description: "返回包含当前激活证书和私钥的 ZIP 附件。",
        errors: &[NOT_FOUND, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/ca/status",
        summary: "查看本地 CA 状态",
        description: "检查本地根 CA 的证书和私钥是否都已初始化。未初始化时返回 `initialized=false`，并非错误；已初始化时附带根证书解析信息。",
        success_description: "返回本地根 CA 的初始化状态及证书信息。",
        errors: &[INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/ca/init",
        summary: "初始化本地根 CA",
        description: "创建本地根 CA 证书及私钥。请先完成该操作，再配置主机名并签发服务器证书；重复执行的具体结果以返回状态为准。",
        success_description: "已创建本地根 CA，并返回根证书信息。",
        errors: &[INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "delete",
        path: "/api/admin/ssl/ca",
        summary: "删除本地根 CA 文件",
        description: "删除本地根 CA 的证书和私钥文件。该操作不会清空已保存的 CA 主机名列表，也不会删除已签发并保存到证书库的服务器证书；后续签发前必须重新初始化 CA。",
        success_description: "已删除本地根 CA 文件。",
        errors: &[],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/ca/cert.pem",
        summary: "下载本地根 CA 证书",
        description: "下载本地根 CA 的公开 PEM 证书，不包含根私钥。可将此证书分发给需要信任本地 CA 的客户端。",
        success_description: "返回本地根 CA 的 PEM 附件。",
        errors: &[NOT_FOUND, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/ca/server-cert.zip",
        summary: "下载本地 CA 临时服务器证书 ZIP",
        description: "使用当前 CA 主机名列表即时签发服务器证书并以 ZIP 下载。该证书不会保存到证书库或激活；附件包含私钥，必须按敏感凭据保管。",
        success_description: "返回基于当前 CA 主机名列表签发的证书和私钥 ZIP 附件。",
        errors: &[BAD_REQUEST, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/ca/hosts",
        summary: "列出本地 CA 主机名",
        description: "返回本地 CA 签发服务器证书时使用的 SAN 主机名列表。列表为空时无法签发或下载本地 CA 服务器证书。",
        success_description: "返回本地 CA 主机名列表。",
        errors: &[INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/ca/hosts",
        summary: "添加本地 CA 主机名",
        description: "将一个非空主机名加入本地 CA 的 SAN 列表。重复值不会重复保存；该操作不会自动重新签发或部署证书。",
        success_description: "返回更新后的本地 CA 主机名列表。",
        errors: &[BAD_REQUEST, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "delete",
        path: "/api/admin/ssl/ca/hosts",
        summary: "移除或清空本地 CA 主机名",
        description: "传入 `value` 时移除对应主机名；传入 `all=true` 时清空整个列表。请求体省略、为空或无法解析时保持兼容行为并成功返回，不会修改列表。",
        success_description: "返回更新后的主机名列表；清空全部或无操作时只返回成功标记。",
        errors: &[INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/ca/issue",
        summary: "签发并部署本地 CA 服务器证书",
        description: "使用已初始化的本地根 CA 和当前主机名列表签发服务器证书，将其保存到证书库、设为激活证书并同步至网关。执行前必须初始化 CA 且至少配置一个主机名。",
        success_description: "已签发、保存并尝试部署本地 CA 服务器证书。",
        errors: &[BAD_REQUEST, GATEWAY_SYNC_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/certificates",
        summary: "导入或更新证书库条目",
        description: "保存手工、ACME 或本地 CA 来源的证书。`cert` 与 `key` 必须是相互匹配且可验证的完整 PEM；私钥仅写入保存，状态接口不会回显。未显式设置 `activate=false` 时会激活新条目；需要同步时会将配置下发到网关。",
        success_description: "已保存证书库条目，并返回其稳定标识符。",
        errors: &[BAD_REQUEST, GATEWAY_SYNC_ERROR],
    },
    OperationDocumentation {
        method: "delete",
        path: "/api/admin/ssl/certificates",
        summary: "清空 SSL 证书库",
        description: "永久删除证书库中的所有条目并清除当前激活证书，然后同步网关。此操作不可恢复；如需保留证书和私钥，请先下载备份。",
        success_description: "已清空证书库并尝试同步网关。",
        errors: &[GATEWAY_SYNC_ERROR],
    },
    OperationDocumentation {
        method: "delete",
        path: "/api/admin/ssl/certificates/{id}",
        summary: "删除指定证书库条目",
        description: "按证书库标识符删除条目。删除当前激活证书，或在 `multi_sni` 模式删除任一证书时，会同步网关；该删除不可恢复。",
        success_description: "已删除指定证书库条目，并在需要时同步网关。",
        errors: &[NOT_FOUND, GATEWAY_SYNC_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/activate",
        summary: "激活证书库中的证书",
        description: "将指定证书设为当前激活证书，并同步网关。在 `single_active` 模式中仅部署该证书；在 `multi_sni` 模式中它还会作为默认服务证书。",
        success_description: "已激活指定证书并尝试同步网关。",
        errors: &[NOT_FOUND, GATEWAY_SYNC_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/deployment-mode",
        summary: "切换 SSL 证书部署模式",
        description: "`single_active` 仅向网关部署当前激活证书；`multi_sni` 部署证书库并以激活证书作为默认项。切换到 `multi_sni` 且未指定激活证书时会选择证书库首项；网关同步失败会恢复之前的本地配置。",
        success_description: "已切换部署模式，并返回更新后的 SSL 状态快照。",
        errors: &[GATEWAY_SYNC_ERROR],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/external-bindings/lan",
        summary: "查看局域网证书推送设置",
        description: "返回管理员确认的 RFC1918 IPv4 允许列表、只读检测地址、网关端口与监听状态。局域网入口复用当前默认 SSL 证书，因此通过 IP 访问通常会发生证书名称不匹配。",
        success_description: "返回局域网证书推送的配置与可用状态。",
        errors: &[INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "put",
        path: "/api/admin/ssl/external-bindings/lan",
        summary: "配置局域网证书推送",
        description: "显式启用或停用局域网 HTTPS 保留入口，并保存最多 16 个 RFC1918 IPv4。启用时要求网关不是仅回环监听且已有默认 SSL 证书；不会新增端口、签发独立证书或暴露 Rust 管理监听器。同步失败会恢复原配置。",
        success_description: "已保存并同步局域网证书推送设置。",
        errors: &[BAD_REQUEST, LAN_CONFLICT, INTERNAL_ERROR, LAN_BAD_GATEWAY],
    },
    OperationDocumentation {
        method: "delete",
        path: "/api/admin/ssl",
        summary: "清除当前激活证书",
        description: "取消当前激活证书并同步网关，但保留证书库条目，以便之后重新激活。若需删除所有条目，请使用清空证书库接口。",
        success_description: "已清除当前激活证书并尝试同步网关。",
        errors: &[GATEWAY_SYNC_ERROR],
    },
    OperationDocumentation {
        method: "get",
        path: "/api/admin/ssl/external-bindings",
        summary: "列出外部证书部署绑定",
        description: "列出 Certd、acme.sh、lego、Certbot 等外部工具可使用的自动部署绑定及最近部署状态。响应不包含 Token、证书 PEM 或私钥；Token 只在创建或轮换时显示一次。",
        success_description: "返回外部证书部署绑定列表。",
        errors: &[INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/external-bindings",
        summary: "创建外部证书部署绑定",
        description: "创建一个稳定证书槽位和仅限该绑定使用的 Bearer Token。支持 `certd`、`acme_sh`、`lego` 和 `certbot`；适配器返回对应 Webhook 或部署钩子模板。Token 只在本次响应中显示，服务端仅持久化其哈希。首次成功部署在系统没有激活证书时自动激活，否则只加入证书库。",
        success_description: "已创建绑定，并一次性返回部署 Token 与所选客户端的接入模板。",
        errors: &[BAD_REQUEST, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "patch",
        path: "/api/admin/ssl/external-bindings/{id}",
        summary: "更新或停用外部部署绑定",
        description: "重命名绑定或切换启用状态。停用后部署入口立即拒绝该绑定的请求，但已经导入的证书会保留，避免中断正在使用的 HTTPS。",
        success_description: "返回更新后的绑定。",
        errors: &[BAD_REQUEST, NOT_FOUND, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "post",
        path: "/api/admin/ssl/external-bindings/{id}/rotate-token",
        summary: "轮换外部部署凭据",
        description: "立即废止旧 Token 并生成新 Token。新 Token 只在本次响应中显示；轮换后需同步更新证书客户端的部署配置。",
        success_description: "已轮换 Token，并一次性返回新凭据。",
        errors: &[NOT_FOUND, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "delete",
        path: "/api/admin/ssl/external-bindings/{id}",
        summary: "撤销外部证书部署绑定",
        description: "永久撤销该绑定及其 Token。对应的证书库条目默认保留，不会删除或停用正在运行的证书。",
        success_description: "已撤销绑定并保留已导入证书。",
        errors: &[NOT_FOUND, INTERNAL_ERROR],
    },
    OperationDocumentation {
        method: "put",
        path: "/api/integrations/certificates/{binding_id}",
        summary: "部署外部证书",
        description: "使用绑定专用 `Authorization: Bearer <token>` 将完整 PEM 证书链和匹配私钥推送到稳定槽位。接口验证证钥匹配、证书链签名与 CA 约束、有效期和请求大小；相同内容幂等成功，到期更早的证书返回 `409`。部署网关失败时尝试恢复旧配置并返回非 2xx。该路径不使用管理会话，只接受绑定专用 Token。",
        success_description: "证书已保存到稳定槽位，并在需要时同步到网关；`changed=false` 表示内容完全相同且未触发重载。",
        errors: &[
            BAD_REQUEST,
            UNAUTHORIZED,
            NOT_FOUND,
            CONFLICT,
            PAYLOAD_TOO_LARGE,
            INTERNAL_ERROR,
            BAD_GATEWAY,
        ],
    },
    OperationDocumentation {
        method: "put",
        path: "/__certificates__/{binding_id}",
        summary: "通过网关保留路径部署外部证书",
        description: "该保留路径可由 HTTPS 鉴权域或管理员显式允许的 RFC1918 IPv4 命中。局域网 IP 入口要求 HTTPS 和私网真实对端，并复用默认证书；调用工具需要按管理员选择使用 `-k` 处理名称不匹配。它与本机兼容接口使用相同的绑定专用 Bearer Token、1 MiB 限制、证书校验、同 SAN 接管、幂等与回滚逻辑；不接受管理会话，不暴露管理 API。绑定 Token 属于不限制 SAN 的证书管理员凭据。",
        success_description: "证书已保存并在需要时同步到网关；响应包含本次同 SAN 接管及被停用自动化的摘要。",
        errors: &[
            BAD_REQUEST,
            UNAUTHORIZED,
            NOT_FOUND,
            CONFLICT,
            PAYLOAD_TOO_LARGE,
            INTERNAL_ERROR,
            BAD_GATEWAY,
        ],
    },
];

const SCHEMA_DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "SslCertificateSaveBodyData",
        "导入或更新 SSL 证书库条目的请求。证书和私钥为敏感材料，私钥仅写入，不会在读取接口中返回。",
    ),
    (
        "SslCertificateActivateBodyData",
        "激活证书库中既有证书的请求。",
    ),
    ("SslDeploymentModeBodyData", "设置网关 SSL 部署模式的请求。"),
    (
        "SslCaHostBodyData",
        "向本地 CA 服务器证书 SAN 列表增加单个主机名的请求。",
    ),
    (
        "SslCaHostsDeleteBodyData",
        "从本地 CA 主机名列表移除单个项目或清空列表的可选请求体。",
    ),
    (
        "SslCertificateInfoData",
        "从 X.509 PEM 证书解析出的公开元数据。",
    ),
    (
        "SslSubdomainCoverageData",
        "单张证书对认证主机和建议域名的覆盖分析。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "证书库在当前部署模式下对建议域名的整体覆盖分析。",
    ),
    (
        "SslCertificateSummaryData",
        "证书库条目的公开摘要；不含证书 PEM 或私钥。",
    ),
    ("SslGatewayCertificateData", "网关当前部署的证书公开摘要。"),
    (
        "SslGatewayStatusData",
        "从网关读取的 SSL 部署状态；网关不可达时会提供同步错误。",
    ),
    (
        "SslStatusData",
        "SSL 模块的完整状态快照，汇总本地证书库、覆盖分析与网关部署结果。",
    ),
    (
        "SslSharedFileData",
        "SSL 共享目录中可供预览或导入的文件元数据。",
    ),
    (
        "SslSharedFilesData",
        "SSL 共享目录的可用性及其中的候选文件列表。",
    ),
    (
        "SslSharedFileContentData",
        "共享目录中单个文本文件的内容和元数据；内容可能包含私钥。",
    ),
    (
        "SslCaStatusData",
        "本地根 CA 是否已初始化及其公开证书信息。",
    ),
    (
        "SslCertificateSaveData",
        "成功保存证书库条目后返回的稳定标识符。",
    ),
    (
        "ExternalCertificateBindingCreateBodyData",
        "创建外部证书自动部署绑定的请求。",
    ),
    (
        "ExternalCertificateBindingUpdateBodyData",
        "重命名、启用或停用外部证书自动部署绑定的请求。",
    ),
    (
        "ExternalCertificateBindingData",
        "外部证书部署绑定的公开配置和最近部署状态，不包含 Token、PEM 或私钥。",
    ),
    (
        "ExternalCertificateBindingCredentialData",
        "创建或轮换绑定时一次性返回的凭据；离开响应后无法再次读取 Token。",
    ),
    (
        "ExternalCertificateDeployBodyData",
        "外部证书客户端推送完整证书链和私钥的请求。",
    ),
    (
        "ExternalCertificateDeployData",
        "外部证书部署结果及证书的非敏感元数据。",
    ),
];

const PROPERTY_DESCRIPTIONS: &[(&str, &str, &str)] = &[
    (
        "SslCertificateSaveBodyData",
        "id",
        "可选的证书库标识符。提供时更新同一条目；省略时会根据证书内容复用或创建标识符。",
    ),
    (
        "SslCertificateSaveBodyData",
        "label",
        "面向管理员显示的证书名称；省略时根据来源和主域名生成。",
    ),
    (
        "SslCertificateSaveBodyData",
        "source",
        "证书来源：`manual` 为手工导入，`acme` 为 ACME 签发，`ca` 为本地 CA 签发，`external` 为外部自动部署。",
    ),
    (
        "SslCertificateSaveBodyData",
        "source_provider",
        "外部来源提供方，例如 `certd`；仅 `source=external` 时保存。",
    ),
    (
        "SslCertificateSaveBodyData",
        "primary_domain",
        "可选主域名，会被规范化为小写；用于来源关联和展示。",
    ),
    (
        "SslCertificateSaveBodyData",
        "source_ref_id",
        "可选的上游来源关联标识，例如 ACME 应用标识。",
    ),
    (
        "SslCertificateSaveBodyData",
        "cert",
        "完整的 PEM X.509 证书链。必须与 `key` 匹配且可通过验证。",
    ),
    (
        "SslCertificateSaveBodyData",
        "key",
        "与 `cert` 匹配的完整 PEM 私钥。该字段仅写入，不会在任何状态或列表响应中返回。",
    ),
    (
        "SslCertificateSaveBodyData",
        "activate",
        "省略或为 `true` 时，保存后立即激活；仅显式传入 `false` 才保留为未激活条目。",
    ),
    (
        "SslCertificateActivateBodyData",
        "id",
        "要激活的证书库条目标识符。",
    ),
    (
        "SslDeploymentModeBodyData",
        "deployment_mode",
        "`single_active` 仅部署激活证书；`multi_sni` 部署整个证书库并使用激活证书作为默认项。",
    ),
    (
        "SslCaHostBodyData",
        "value",
        "要加入本地 CA 服务器证书 SAN 列表的非空 DNS 主机名。",
    ),
    (
        "SslCaHostsDeleteBodyData",
        "value",
        "要移除的单个主机名；与 `all=true` 一起提供时由 `all` 优先。",
    ),
    (
        "SslCaHostsDeleteBodyData",
        "all",
        "设为 `true` 时清空整个本地 CA 主机名列表；请求体省略时为兼容性无操作。",
    ),
    ("SslCertificateInfoData", "issuer", "颁发者可分辨名称。"),
    ("SslCertificateInfoData", "subject", "主题可分辨名称。"),
    (
        "SslCertificateInfoData",
        "valid_from",
        "证书生效时间，使用 OpenSSL 兼容的 UTC 文本格式。",
    ),
    (
        "SslCertificateInfoData",
        "valid_to",
        "证书到期时间，使用 OpenSSL 兼容的 UTC 文本格式。",
    ),
    (
        "SslCertificateInfoData",
        "dnsNames",
        "从证书 Subject Alternative Name 读取的 DNS 名称。",
    ),
    ("SslCertificateInfoData", "serialNumber", "证书序列号。"),
    (
        "SslSubdomainCoverageData",
        "status",
        "覆盖结论：`ready` 完整覆盖、`partial` 部分覆盖、`missing` 缺少可用证书。",
    ),
    (
        "SslSubdomainCoverageData",
        "auth_host",
        "需要受到证书覆盖的认证主机；未配置时为 `null`。",
    ),
    (
        "SslSubdomainCoverageData",
        "certificate_domains",
        "当前证书声明的 DNS 名称。",
    ),
    (
        "SslSubdomainCoverageData",
        "recommended_domains",
        "根据当前配置建议由证书覆盖的域名集合。",
    ),
    (
        "SslSubdomainCoverageData",
        "covered_recommended_domains",
        "已被当前证书覆盖的建议域名。",
    ),
    (
        "SslSubdomainCoverageData",
        "uncovered_recommended_domains",
        "尚未被当前证书覆盖的建议域名。",
    ),
    (
        "SslSubdomainCoverageData",
        "covered_hosts",
        "已覆盖的实际主机名。",
    ),
    (
        "SslSubdomainCoverageData",
        "uncovered_hosts",
        "尚未覆盖的实际主机名。",
    ),
    (
        "SslSubdomainCoverageData",
        "covers_auth_host",
        "当前证书是否覆盖认证主机。",
    ),
    (
        "SslSubdomainCoverageData",
        "warnings",
        "需要管理员处理的覆盖风险或配置提示。",
    ),
    (
        "SslSubdomainCoverageData",
        "summary",
        "面向界面显示的本地化覆盖摘要。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "status",
        "证书库整体覆盖结论：`ready`、`partial` 或 `missing`。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "deployment_mode",
        "用于计算整体覆盖度的有效部署模式。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "active_certificate_id",
        "当前激活证书的标识符；未激活时为 `null`。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "fully_covering_certificate_ids",
        "可单独覆盖全部建议域名的证书标识符。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "partially_covering_certificate_ids",
        "仅部分覆盖建议域名的证书标识符。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "combined_covering_certificate_ids",
        "在 `multi_sni` 模式中组合后可覆盖建议域名的证书标识符。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "suggested_certificate_id",
        "建议激活的证书标识符；无合适候选时为 `null`。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "can_auto_activate",
        "系统是否可以安全地自动激活建议证书。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "warnings",
        "证书库覆盖风险或选择建议。",
    ),
    (
        "SslCertificateLibraryCoverageData",
        "summary",
        "面向界面显示的本地化证书库覆盖摘要。",
    ),
    (
        "SslCertificateSummaryData",
        "id",
        "证书库条目的稳定标识符。",
    ),
    (
        "SslCertificateSummaryData",
        "label",
        "面向管理员显示的证书名称。",
    ),
    (
        "SslCertificateSummaryData",
        "source",
        "证书来源：手工导入、ACME、本地 CA 或外部自动部署。",
    ),
    (
        "SslCertificateSummaryData",
        "source_provider",
        "外部自动部署提供方，例如 `certd`；其他来源通常为 `null`。",
    ),
    (
        "SslCertificateSummaryData",
        "primary_domain",
        "可选主域名；未关联主域名时为 `null`。",
    ),
    (
        "SslCertificateSummaryData",
        "source_ref_id",
        "可选的上游来源关联标识。",
    ),
    (
        "SslCertificateSummaryData",
        "created_at",
        "证书库条目的创建时间。",
    ),
    (
        "SslCertificateSummaryData",
        "updated_at",
        "证书库条目的最近更新时间。",
    ),
    (
        "SslCertificateSummaryData",
        "certInfo",
        "从公开证书解析出的元数据；无法解析时为 `null`。",
    ),
    (
        "SslCertificateSummaryData",
        "is_active",
        "该条目是否为当前激活证书。",
    ),
    (
        "SslCertificateSummaryData",
        "coverage",
        "该单张证书对当前域名配置的覆盖分析。",
    ),
    (
        "SslGatewayCertificateData",
        "id",
        "网关已部署证书对应的证书库标识符；旧部署可能为空。",
    ),
    (
        "SslGatewayCertificateData",
        "label",
        "网关已部署证书的显示名称。",
    ),
    (
        "SslGatewayCertificateData",
        "domains",
        "网关报告的证书域名集合。",
    ),
    (
        "SslGatewayCertificateData",
        "is_default",
        "该证书是否是网关的默认服务证书。",
    ),
    (
        "SslGatewayStatusData",
        "enabled",
        "网关当前是否启用了 SSL 部署。",
    ),
    (
        "SslGatewayStatusData",
        "deployment_mode",
        "网关实际报告的部署模式。",
    ),
    (
        "SslGatewayStatusData",
        "certificates",
        "网关当前已部署的公开证书摘要。",
    ),
    (
        "SslGatewayStatusData",
        "sync_error",
        "无法读取或同步网关状态时的原因；正常情况下为 `null` 或省略。",
    ),
    (
        "SslStatusData",
        "enabled",
        "当前是否存在网关或本地可用的 SSL 部署。",
    ),
    (
        "SslStatusData",
        "activeCertId",
        "当前激活证书库条目的标识符；无激活证书时为 `null`。",
    ),
    (
        "SslStatusData",
        "deploymentMode",
        "当前生效的部署模式；网关已报告 `multi_sni` 时优先反映网关状态。",
    ),
    (
        "SslStatusData",
        "configuredDeploymentMode",
        "本地配置的部署模式，不受网关临时状态影响。",
    ),
    (
        "SslStatusData",
        "certInfo",
        "当前激活证书的公开解析信息；无激活证书时为 `null`。",
    ),
    (
        "SslStatusData",
        "certificates",
        "证书库的公开摘要列表，不包含 PEM 或私钥。",
    ),
    (
        "SslStatusData",
        "subdomain_coverage",
        "当前激活证书的域名覆盖分析。",
    ),
    (
        "SslStatusData",
        "library_coverage",
        "证书库在有效部署模式下的整体域名覆盖分析。",
    ),
    (
        "SslStatusData",
        "gateway_status",
        "网关的 SSL 部署状态或同步错误。",
    ),
    ("SslSharedFileData", "name", "文件名，不包含目录部分。"),
    (
        "SslSharedFileData",
        "relativePath",
        "相对于 SSL 共享目录的路径；可用于读取内容接口。",
    ),
    (
        "SslSharedFileData",
        "extension",
        "小写文件扩展名，带前导句点；没有扩展名时为空字符串。",
    ),
    ("SslSharedFileData", "size", "文件大小，单位为字节。"),
    ("SslSharedFileData", "modifiedAt", "文件的最近修改时间。"),
    (
        "SslSharedFilesData",
        "shareName",
        "SSL 共享目录在界面中使用的名称。",
    ),
    (
        "SslSharedFilesData",
        "available",
        "共享目录是否已经配置且可扫描。",
    ),
    (
        "SslSharedFilesData",
        "files",
        "可预览或导入的候选文件，按修改时间倒序排列。",
    ),
    ("SslSharedFileContentData", "file", "已读取文件的元数据。"),
    (
        "SslSharedFileContentData",
        "content",
        "去除 UTF-8 BOM 后的文本内容；可能包含私钥，禁止在不可信环境中记录或分享。",
    ),
    (
        "SslCaStatusData",
        "initialized",
        "本地根 CA 的证书和私钥是否均存在。",
    ),
    (
        "SslCaStatusData",
        "info",
        "已初始化时的根 CA 公开证书信息；否则为 `null`。",
    ),
    (
        "SslCertificateSaveData",
        "id",
        "已保存或更新的证书库条目标识符。",
    ),
    (
        "ExternalCertificateBindingCreateBodyData",
        "name",
        "管理员可识别的绑定名称，最长 80 个字符。",
    ),
    (
        "ExternalCertificateBindingCreateBodyData",
        "provider",
        "外部部署适配器：`certd`、`acme_sh`、`lego` 或 `certbot`；省略时默认使用 `certd`。",
    ),
    (
        "ExternalCertificateBindingUpdateBodyData",
        "name",
        "新的绑定显示名称。",
    ),
    (
        "ExternalCertificateBindingUpdateBodyData",
        "enabled",
        "是否允许该绑定继续推送证书；停用不会删除既有证书。",
    ),
    (
        "ExternalCertificateBindingData",
        "id",
        "绑定的稳定标识符，也是部署 URL 的一部分。",
    ),
    (
        "ExternalCertificateBindingData",
        "name",
        "绑定的管理员显示名称。",
    ),
    (
        "ExternalCertificateBindingData",
        "provider",
        "外部部署提供方。适配器只负责生成原生接入配置，验证、存储和网关事务由统一部署服务处理。",
    ),
    (
        "ExternalCertificateBindingData",
        "certificate_id",
        "该绑定始终原位更新的稳定证书库条目标识符。",
    ),
    (
        "ExternalCertificateBindingData",
        "enabled",
        "绑定是否允许继续部署。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_deployed_at",
        "最近一次成功或失败部署尝试的时间。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_result",
        "最近一次部署结果：`success`、`failed` 或因同 SAN 接管而产生的 `superseded`。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_error",
        "最近一次失败的截断错误信息；绝不包含 PEM、私钥或 Token。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_fingerprint_sha256",
        "最近成功部署证书的 SHA-256 指纹。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_valid_to",
        "最近成功部署证书的到期时间。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_dns_names",
        "最近成功部署证书的 DNS SAN。",
    ),
    (
        "ExternalCertificateBindingData",
        "deploy_path",
        "绑定专用的相对部署路径。",
    ),
    (
        "ExternalCertificateBindingData",
        "deploy_port",
        "fn-knock 本机兼容证书接收端口，取自运行时 `BACKEND_PORT`，与 `deploy_path` 组合为 127.0.0.1 回环地址。不要直接暴露到公网。",
    ),
    (
        "ExternalCertificateBindingData",
        "public_deploy_url",
        "鉴权域启用 HTTPS 时生成的推荐公网部署 URL；其他状态为 null。",
    ),
    (
        "ExternalCertificateBindingData",
        "public_deploy_status",
        "公网部署入口状态：`ready`、`auth_host_unconfigured` 或 `https_required`。",
    ),
    (
        "ExternalCertificateBindingData",
        "lan_deploy_urls",
        "显式启用且网关默认证书可用时，为每个允许的 RFC1918 IPv4 生成的 HTTPS 部署 URL。IP 访问预期存在名称不匹配，调用方需显式允许该错误。",
    ),
    (
        "ExternalCertificateBindingData",
        "lan_deploy_status",
        "局域网部署入口状态；不可用时不会生成 HTTP 替代 URL。",
    ),
    (
        "LanCertificateDeploymentUpdateBodyData",
        "enabled",
        "是否显式启用局域网 HTTPS 证书推送。",
    ),
    (
        "LanCertificateDeploymentUpdateBodyData",
        "addresses",
        "管理员确认的 RFC1918 IPv4 列表，去重后最多 16 个。",
    ),
    (
        "LanCertificateDeploymentData",
        "configured_addresses",
        "当前持久化并获准命中保留部署路由的地址。",
    ),
    (
        "LanCertificateDeploymentData",
        "enabled",
        "局域网 HTTPS 证书推送是否已显式启用。",
    ),
    (
        "LanCertificateDeploymentData",
        "detected_addresses",
        "只读检测到的候选宿主机地址；不会自动加入允许列表。",
    ),
    (
        "LanCertificateDeploymentData",
        "gateway_port",
        "局域网 HTTPS 入口复用的 Go 网关端口。",
    ),
    (
        "LanCertificateDeploymentData",
        "listener_scope",
        "Go 网关当前监听范围；`loopback` 状态不允许启用局域网入口。",
    ),
    (
        "LanCertificateDeploymentData",
        "status",
        "局域网入口的可操作状态。",
    ),
    (
        "ExternalCertificateBindingData",
        "setup_kind",
        "接入配置类型：`webhook` 用于 Certd，`deploy_hook` 用于命令行 ACME 客户端。",
    ),
    (
        "ExternalCertificateBindingData",
        "request_method",
        "Webhook 请求方法；仅 `webhook` 类型返回。",
    ),
    (
        "ExternalCertificateBindingData",
        "request_body_template",
        "Webhook JSON 模板；Certd 模板包含 `${crt}` 与 `${key}`，仅 `webhook` 类型返回。",
    ),
    (
        "ExternalCertificateBindingData",
        "success_marker",
        "Webhook 工具可用于判断响应成功的匹配字符串，仅 `webhook` 类型返回。",
    ),
    (
        "ExternalCertificateBindingData",
        "script_template",
        "适配器生成的部署钩子脚本，包含 URL 与 Token 占位符；仅 `deploy_hook` 类型返回。脚本使用 `jq` 安全编码 PEM，并用 `curl` 上传。",
    ),
    (
        "ExternalCertificateBindingData",
        "usage_instructions",
        "保存和注册部署钩子的简短说明，仅 `deploy_hook` 类型返回。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_replaced_certificate_count",
        "最近一次成功部署接管的同 SAN 证书库条目数。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_replaced_sources",
        "最近一次接管所替换的证书来源类型。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_disabled_external_binding_count",
        "最近一次接管自动停用的其他外部绑定数。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_disabled_acme_renewal_count",
        "最近一次接管自动停用的 ACME 自动续期数。",
    ),
    (
        "ExternalCertificateBindingData",
        "last_takeover_at",
        "最近一次成功接管同 SAN 证书的时间；尚未发生接管时为 null。",
    ),
    (
        "ExternalCertificateBindingCredentialData",
        "binding",
        "刚创建或刚轮换的绑定配置。",
    ),
    (
        "ExternalCertificateBindingCredentialData",
        "token",
        "只显示一次的绑定专用 Bearer Token；服务端仅保存哈希。",
    ),
    (
        "ExternalCertificateDeployBodyData",
        "cert",
        "包含叶证书及中间证书的完整 PEM 证书链。",
    ),
    (
        "ExternalCertificateDeployBodyData",
        "key",
        "与叶证书匹配的 PEM 私钥；仅写入，不会在响应或日志中回显。",
    ),
    (
        "ExternalCertificateDeployData",
        "binding_id",
        "本次使用的外部部署绑定。",
    ),
    (
        "ExternalCertificateDeployData",
        "certificate_id",
        "被创建或原位更新的稳定证书库条目。",
    ),
    (
        "ExternalCertificateDeployData",
        "changed",
        "证书或私钥是否发生变化；为 `false` 时不写配置、不重载网关。",
    ),
    (
        "ExternalCertificateDeployData",
        "gateway_applied",
        "本次请求是否实际完成了网关同步。",
    ),
    (
        "ExternalCertificateDeployData",
        "is_active",
        "该稳定槽位是否为当前激活证书。",
    ),
    (
        "ExternalCertificateDeployData",
        "fingerprint_sha256",
        "叶证书 DER 内容的 SHA-256 指纹。",
    ),
    (
        "ExternalCertificateDeployData",
        "valid_to",
        "证书到期时间。",
    ),
    (
        "ExternalCertificateDeployData",
        "dns_names",
        "证书声明的 DNS SAN。",
    ),
    (
        "ExternalCertificateDeployData",
        "replaced_certificate_count",
        "本次部署接管的同 SAN 证书库条目数。",
    ),
    (
        "ExternalCertificateDeployData",
        "replaced_sources",
        "本次部署接管的来源类型。",
    ),
    (
        "ExternalCertificateDeployData",
        "disabled_external_binding_count",
        "本次接管停用的其他外部绑定数。",
    ),
    (
        "ExternalCertificateDeployData",
        "disabled_acme_renewal_count",
        "本次接管停用的 ACME 自动续期数。",
    ),
];

pub(super) fn tag() -> Value {
    json!({
        "name": "ssl",
        "description": "SSL 证书库、共享文件导入、本地 CA 与外部证书自动部署管理。\n\n推荐流程：手工导入证书后激活并选择部署模式；初始化本地 CA、配置主机名后签发；或创建外部绑定，让 Certd、acme.sh、lego 或 Certbot 使用独立 Bearer Token 推送续期证书。\n\n`/api/admin/ssl/*` 需要同源管理面板会话；`/api/integrations/certificates/{binding_id}` 与网关上的 `/__certificates__/{binding_id}` 不使用管理会话，只接受绑定专用 Token。后者仅允许 HTTPS 鉴权域，或显式启用且通过来源校验的局域网 IP。下载 ZIP 或读取共享文件内容可能暴露私钥。"
    })
}

pub(super) fn apply(paths: &mut Map<String, Value>, components: &mut Map<String, Value>) {
    for documentation in OPERATIONS {
        let Some(operation) = paths
            .get_mut(documentation.path)
            .and_then(Value::as_object_mut)
            .and_then(|path| path.get_mut(documentation.method))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        operation.insert(
            "summary".to_string(),
            Value::String(documentation.summary.to_string()),
        );
        operation.insert(
            "description".to_string(),
            Value::String(documentation.description.to_string()),
        );
        document_responses(operation, documentation);
        document_request_examples(operation, documentation.method, documentation.path);
        document_parameter_examples(operation, documentation.method, documentation.path);
    }
    document_schemas(components);
}

fn document_responses(operation: &mut Map<String, Value>, documentation: &OperationDocumentation) {
    let Some(responses) = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if let Some(success) = responses.get_mut("200").and_then(Value::as_object_mut) {
        success.insert(
            "description".to_string(),
            Value::String(documentation.success_description.to_string()),
        );
        if let Some(examples) = success_examples(documentation.method, documentation.path) {
            insert_content_examples(success, examples);
        }
    }
    for error in documentation.errors {
        responses.insert(
            error.status.to_string(),
            documented_error_response(error.description),
        );
    }
    if let Some(default_response) = responses.get_mut("default").and_then(Value::as_object_mut)
        && default_response
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| description == "Standard fn-knock error response")
    {
        default_response.insert(
            "description".to_string(),
            Value::String(
                "未分类的 SSL 操作失败时返回标准错误信封；请结合 HTTP 状态、错误消息和 SSL 状态排查。"
                    .to_string(),
            ),
        );
    }
}

fn documented_error_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" },
                "example": {
                    "success": false,
                    "code": null,
                    "message": "请求未完成；请根据接口说明检查输入和当前 SSL 状态。"
                }
            }
        }
    })
}

fn document_request_examples(operation: &mut Map<String, Value>, method: &str, path: &str) {
    let Some(examples) = request_examples(method, path) else {
        return;
    };
    let Some(request_body) = operation
        .get_mut("requestBody")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    insert_content_examples(request_body, examples);
}

fn document_parameter_examples(operation: &mut Map<String, Value>, method: &str, path: &str) {
    let example = match (method, path) {
        ("get", "/api/admin/ssl/shared-files/content") => Some("certificates/example.pem"),
        ("delete", "/api/admin/ssl/certificates/{id}") => Some("ssl_example_2026"),
        ("patch" | "delete", "/api/admin/ssl/external-bindings/{id}")
        | ("post", "/api/admin/ssl/external-bindings/{id}/rotate-token") => {
            Some("a17f93f95c2d4e9db7d41b8122345678")
        }
        ("put", "/api/integrations/certificates/{binding_id}")
        | ("put", "/__certificates__/{binding_id}") => Some("a17f93f95c2d4e9db7d41b8122345678"),
        _ => None,
    };
    let Some(example) = example else {
        return;
    };
    let Some(parameters) = operation
        .get_mut("parameters")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for parameter in parameters {
        if let Some(schema) = parameter.get_mut("schema").and_then(Value::as_object_mut) {
            schema.insert("example".to_string(), Value::String(example.to_string()));
        }
    }
}

fn insert_content_examples(container: &mut Map<String, Value>, examples: Value) {
    let Some(content) = container.get_mut("content").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(json_content) = content
        .get_mut("application/json")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    json_content.insert("examples".to_string(), examples);
}

fn request_examples(method: &str, path: &str) -> Option<Value> {
    match (method, path) {
        ("post", "/api/admin/ssl/ca/hosts") => Some(json!({
            "addHost": {
                "summary": "添加单个主机名",
                "value": { "value": "panel.example.internal" }
            }
        })),
        ("delete", "/api/admin/ssl/ca/hosts") => Some(json!({
            "removeOne": {
                "summary": "移除单个主机名",
                "value": { "value": "old.example.internal" }
            },
            "clearAll": {
                "summary": "清空全部主机名",
                "value": { "all": true }
            }
        })),
        ("post", "/api/admin/ssl/activate") => Some(json!({
            "activate": {
                "summary": "激活证书库条目",
                "value": { "id": "ssl_example_2026" }
            }
        })),
        ("post", "/api/admin/ssl/deployment-mode") => Some(json!({
            "singleActive": {
                "summary": "只部署当前激活证书",
                "value": { "deployment_mode": "single_active" }
            },
            "multiSni": {
                "summary": "部署整个证书库",
                "value": { "deployment_mode": "multi_sni" }
            }
        })),
        ("post", "/api/admin/ssl/external-bindings") => Some(json!({
            "certd": {
                "summary": "创建 Certd 部署绑定",
                "value": { "name": "Certd example.com", "provider": "certd" }
            },
            "acmeSh": {
                "summary": "创建 acme.sh 部署钩子绑定",
                "value": { "name": "acme.sh example.com", "provider": "acme_sh" }
            },
            "lego": {
                "summary": "创建 lego 部署钩子绑定",
                "value": { "name": "lego example.com", "provider": "lego" }
            },
            "certbot": {
                "summary": "创建 Certbot 部署钩子绑定",
                "value": { "name": "Certbot example.com", "provider": "certbot" }
            }
        })),
        ("patch", "/api/admin/ssl/external-bindings/{id}") => Some(json!({
            "disable": {
                "summary": "暂时停用自动部署",
                "value": { "enabled": false }
            },
            "rename": {
                "summary": "修改显示名称",
                "value": { "name": "Certd production" }
            }
        })),
        _ => None,
    }
}

fn success_examples(method: &str, path: &str) -> Option<Value> {
    match (method, path) {
        ("get", "/api/admin/ssl/status") | ("post", "/api/admin/ssl/deployment-mode") => {
            Some(json!({
                "configured": {
                    "summary": "已部署的单证书模式",
                    "value": ssl_status_example()
                }
            }))
        }
        ("get", "/api/admin/ssl/shared-files") => Some(json!({
            "available": {
                "summary": "共享目录可用",
                "value": {
                    "success": true,
                    "message": null,
                    "data": {
                        "shareName": "fn-knock",
                        "available": true,
                        "files": [{
                            "name": "example.pem",
                            "relativePath": "certificates/example.pem",
                            "extension": ".pem",
                            "size": 2048,
                            "modifiedAt": "2026-08-16T08:00:00Z"
                        }]
                    }
                }
            }
        })),
        ("get", "/api/admin/ssl/ca/hosts") | ("post", "/api/admin/ssl/ca/hosts") => Some(json!({
            "hosts": {
                "summary": "已配置的主机名",
                "value": {
                    "success": true,
                    "message": null,
                    "data": ["panel.example.internal", "auth.example.internal"]
                }
            }
        })),
        ("post", "/api/admin/ssl/certificates") => Some(json!({
            "saved": {
                "summary": "保存后的证书库标识符",
                "value": {
                    "success": true,
                    "message": null,
                    "data": { "id": "ssl_example_2026" }
                }
            }
        })),
        ("post", "/api/admin/ssl/activate") => Some(json!({
            "activated": {
                "summary": "激活成功",
                "value": { "success": true, "message": null }
            }
        })),
        _ => None,
    }
}

fn ssl_status_example() -> Value {
    json!({
        "success": true,
        "message": null,
        "data": {
            "enabled": true,
            "activeCertId": "ssl_example_2026",
            "deploymentMode": "single_active",
            "configuredDeploymentMode": "single_active",
            "certificates": [{
                "id": "ssl_example_2026",
                "label": "example.internal",
                "source": "manual",
                "created_at": "2026-08-16T08:00:00Z",
                "updated_at": "2026-08-16T08:00:00Z",
                "is_active": true,
                "coverage": {
                    "status": "ready",
                    "auth_host": "auth.example.internal",
                    "certificate_domains": ["example.internal", "*.example.internal"],
                    "recommended_domains": ["auth.example.internal"],
                    "covered_recommended_domains": ["auth.example.internal"],
                    "uncovered_recommended_domains": [],
                    "covered_hosts": ["auth.example.internal"],
                    "uncovered_hosts": [],
                    "covers_auth_host": true,
                    "warnings": [],
                    "summary": "证书覆盖全部建议域名"
                }
            }],
            "subdomain_coverage": {
                "status": "ready",
                "auth_host": "auth.example.internal",
                "certificate_domains": ["example.internal", "*.example.internal"],
                "recommended_domains": ["auth.example.internal"],
                "covered_recommended_domains": ["auth.example.internal"],
                "uncovered_recommended_domains": [],
                "covered_hosts": ["auth.example.internal"],
                "uncovered_hosts": [],
                "covers_auth_host": true,
                "warnings": [],
                "summary": "证书覆盖全部建议域名"
            },
            "library_coverage": {
                "status": "ready",
                "deployment_mode": "single_active",
                "active_certificate_id": "ssl_example_2026",
                "fully_covering_certificate_ids": ["ssl_example_2026"],
                "partially_covering_certificate_ids": [],
                "combined_covering_certificate_ids": ["ssl_example_2026"],
                "suggested_certificate_id": "ssl_example_2026",
                "can_auto_activate": true,
                "warnings": [],
                "summary": "证书库已覆盖全部建议域名"
            },
            "gateway_status": {
                "enabled": true,
                "deployment_mode": "single_active",
                "certificates": [{
                    "id": "ssl_example_2026",
                    "label": "example.internal",
                    "domains": ["example.internal", "*.example.internal"],
                    "is_default": true
                }]
            }
        }
    })
}

fn document_schemas(components: &mut Map<String, Value>) {
    let Some(schemas) = components.get_mut("schemas").and_then(Value::as_object_mut) else {
        return;
    };
    for (schema, description) in SCHEMA_DESCRIPTIONS {
        set_schema_description(schemas, schema, description);
    }
    for (schema, property, description) in PROPERTY_DESCRIPTIONS {
        set_property_description(schemas, schema, property, description);
    }
}

fn set_schema_description(schemas: &mut Map<String, Value>, schema: &str, description: &str) {
    if let Some(schema) = schemas.get_mut(schema).and_then(Value::as_object_mut) {
        schema.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
}

fn set_property_description(
    schemas: &mut Map<String, Value>,
    schema: &str,
    property: &str,
    description: &str,
) {
    let Some(property) = schemas
        .get_mut(schema)
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(property))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    property.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
}
