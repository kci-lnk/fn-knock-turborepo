type RouteConfig = {
  detail?: Record<string, unknown>;
  [key: string]: unknown;
};

const defaultDocResponses = {
  200: {
    description: "Successful response",
  },
} as const;

export const routeDoc = (summary: string, description?: string) => ({
  detail: {
    summary,
    ...(description ? { description } : {}),
    responses: defaultDocResponses,
  },
});

export const withRouteDoc = <T extends RouteConfig>(
  summary: string,
  config?: T,
  description?: string,
) =>
  ({
    ...(config ?? {}),
    detail: {
      ...(config?.detail ?? {}),
      summary,
      ...(description ? { description } : {}),
      ...(!("response" in (config ?? {})) &&
      !(config?.detail && "responses" in config.detail)
        ? { responses: defaultDocResponses }
        : {}),
    },
  }) as T & {
    detail: {
      summary: string;
      description?: string;
    };
  };

export const hideFromDocs = {
  detail: {
    hide: true,
  },
} as const;

export const adminOpenApiTags = [
  {
    name: "Admin",
    description: "管理端核心配置、运行模式、防火墙、会话与备份接口。",
  },
  {
    name: "Dashboard",
    description: "首页统计、实时流量和安全概览接口。",
  },
  {
    name: "System",
    description: "系统时间、入口信息、基础组件安装状态等接口。",
  },
  {
    name: "SSL",
    description: "SSL 证书库、本地 CA、网关部署和证书下载接口。",
  },
  {
    name: "ACME",
    description: "ACME 客户端、证书申请任务与证书部署接口。",
  },
  {
    name: "Whitelist",
    description: "IP 白名单查询、创建、删除和备注接口。",
  },
  {
    name: "Backoff",
    description: "登录失败退避与封禁状态接口。",
  },
  {
    name: "Scanner",
    description: "扫描器设置与黑名单接口。",
  },
  {
    name: "Assets",
    description: "局域网资源扫描与发现接口。",
  },
  {
    name: "DDNS",
    description: "DDNS 状态、配置、日志和手动测试接口。",
  },
  {
    name: "Gateway Logs",
    description: "网关请求日志配置与查询接口。",
  },
  {
    name: "WAF",
    description: "WAF 配置、网关运行状态、事件拉取与日志查询接口。",
  },
  {
    name: "Notifications",
    description: "通知提供商、规则、触发器与投递记录接口。",
  },
  {
    name: "Events",
    description: "系统事件查询与清理接口。",
  },
  {
    name: "Internal Events",
    description: "内部系统事件写入接口。",
  },
  {
    name: "Tunnel - FRP",
    description: "FRP 客户端配置、状态、日志与轮询接口。",
  },
  {
    name: "Tunnel - Cloudflared",
    description: "Cloudflared 配置、状态、日志与轮询接口。",
  },
  {
    name: "Terminal",
    description: "Web 终端会话与附件接口。",
  },
  {
    name: "SSH Security",
    description: "SSH 登录日志、防火墙封锁同步和封锁解除接口。",
  },
  {
    name: "CIDR",
    description: "CIDR 地域数据与查询接口。",
  },
  {
    name: "IP Location",
    description: "IP 地理位置批量查询接口。",
  },
  {
    name: "Update",
    description: "更新检查、下载、安装与确认接口。",
  },
] as const;

export const authOpenApiTags = [
  {
    name: "Auth",
    description: "认证引导、验证码、TOTP 登录、会话校验与登出接口。",
  },
  {
    name: "Auth - Passkey",
    description: "Passkey 登录、绑定令牌、注册选项与注册校验接口。",
  },
  {
    name: "Auth - OIDC",
    description: "外部登录提供商、授权跳转、绑定邀请与 OIDC 回调接口。",
  },
] as const;
