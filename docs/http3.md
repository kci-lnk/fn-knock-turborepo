# HTTP/3 入站网关

HTTP/3 默认关闭。在系统设置的网关设置中保存 HTTP/3 开关与公网 UDP 端口。
只有 Go 网关的 HTTPS 直连入口支持，回源仍使用 HTTP/1.1 或 HTTP/2。
协议模式为 `auto` 的域名可使用 HTTP/3，强制 `http1` / `http2` 的域名不参与。

## 部署

先部署有效的 TLS 证书。Go 在 TCP 网关的同号 UDP 端口监听，默认 `7999/udp`。
Docker Compose 已同时声明 TCP 和 UDP 映射；自定义 compose、NAS 防火墙、
OpenWrt、Windows 防火墙及公网 NAT 都需要放行实际使用的 UDP 端口。
例如公网 `443/udp` 转发至 `7999/udp`，同时保留原 TCP 转发。
IPv4 和 IPv6 的放行规则应分别核对。修改 compose 后需要重建容器才能应用映射。

广播端口为 `0` 时使用 HTTPS 请求 authority 的端口（未指定则为 443）；
若公网 UDP 端口不同，应填入公网 UDP 端口。Alt-Svc 缓存为 300 秒。
管理界面显示的是本机监听状态，不能证明公网 UDP 可达。
可在公网客户端使用支持 HTTP/3 的 `curl --http3-only https://域名/` 验证；
浏览器首次请求可能走 HTTP/2，收到 Alt-Svc 后再使用 h3。

现有 UDP 流转发占用网关端口时不能启用 HTTP/3；启用后不能增加冲突的流转发规则。
托管 FRP 强制入口模式暂停原生 HTTP/3；Cloudflare / fnOS 专用入口不广播本机服务。
不解析 UDP PROXY protocol，原有 TCP PROXY protocol 配置不变。

## 安全与回退

0-RTT、HTTP/3 Extended CONNECT、HTTP Datagrams 均关闭。WebSocket 沿用 TCP。
HTTP/3 的 IP 识别使用 QUIC 对端，不接受 CDN / X-Forwarded-For 伪造头。
跨 IP 迁移要求重新连接；新请求重新检查对端 IP 与协议策略。

关闭开关会排空 QUIC 请求（最多 15 秒）、释放 UDP socket 并停止广播；
HTTPS 响应发送 `Alt-Svc: clear`。TCP 入口一直保留。
UDP 启动失败不会关闭 TCP，管理界面会报告监听错误。

## 实现与依赖

共享 gRPC 契约提供独立 `GatewayHttp3Config` 与 `GatewayHttp3Status`。
Rust API 为 `GET/POST /api/admin/config/gateway/http3`，配置字段为
`enabled` 和 `advertised_port`（0–65535），状态含实际地址、错误及连接统计。
配置同时持久化于 Rust 设置和 Go 网关配置，Rust 启动时同步；更新失败保留原配置。

Go 最低版本为 1.26.0，工具链为 1.26.7。quic-go 固定为 v0.62.0，
源码在 Go 仓库 `third_party/quic-go`，保留 MIT 许可证。唯一产品补丁是
`DisableExtendedConnect`：上游默认总是声明 Extended CONNECT，网关需要关闭
它以保持未实现 HTTP/3 WebSocket 时的 TCP 行为。维护时必须跟踪上游安全更新。

## 验证记录

自动化测试、构建及本机基准结果见 [验证记录](http3-validation-2026-09-08.md)。当前开发机测试不能替代公网 NAT、
浏览器协议选择、低配 ARM 设备和真实丢包网络验证。不承诺 HTTP/3 一定更快。
