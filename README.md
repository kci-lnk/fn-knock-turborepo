<p align="center">
  <a href="https://www.fnknock.cn/">
    <img src="./assets/fn-knock.webp" alt="fn-knock" width="860">
  </a>
</p>

<p align="center">
  <a href="https://www.fnknock.cn/"><img alt="官网" src="https://img.shields.io/badge/%E5%AE%98%E7%BD%91-fnknock.cn-2563eb?style=flat-square"></a>
  <a href="https://docs.fnknock.cn/"><img alt="文档" src="https://img.shields.io/badge/%E6%96%87%E6%A1%A3-docs.fnknock.cn-16a34a?style=flat-square"></a>
  <a href="https://hub.docker.com/r/kcilnk/fn-knock"><img alt="Docker Hub" src="https://img.shields.io/badge/Docker%20Hub-kcilnk%2Ffn--knock-2496ed?style=flat-square&logo=docker&logoColor=white"></a>
  <img alt="Node.js" src="https://img.shields.io/badge/Node.js-%3E%3D18-339933?style=flat-square&logo=node.js&logoColor=white">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-111827?style=flat-square">
</p>

# fn-knock

`fn-knock` 是一套用于私有服务暴露场景的访问网关和管理面板，适合部署在 NAS、家庭服务器或 Docker 主机前面。

它把反向代理、登录鉴权、证书、DDNS、白名单、WAF、隧道和运行状态放进同一个面板里，重点解决“服务能安全访问，也方便日常维护”的问题。

## 链接

- 官网：[https://www.fnknock.cn/](https://www.fnknock.cn/)
- 文档：[https://docs.fnknock.cn/](https://docs.fnknock.cn/)
- Docker Hub：[kcilnk/fn-knock](https://hub.docker.com/r/kcilnk/fn-knock)

## 主要功能

- 网关代理：反向代理、认证前置、访问日志、网关配置管理
- 登录鉴权：管理后台登录、OIDC、Passkey、验证码等认证能力
- 域名证书：ACME 证书申请、SSL 配置、DDNS 管理
- 网络安全：IP 白名单、WAF、登录退避、SSH 安全相关配置
- 隧道集成：Cloudflared、frpc 的配置、启动、日志和状态查看
- 运维面板：系统监控、在线终端、更新检查、通知和运行日志
- 多形态发布：Docker 镜像、飞牛 FPK、Windows x86_64 原生服务与桌面管理端

## 快速开始

直接使用镜像：

```bash
docker pull kcilnk/fn-knock:latest
```

完整 Docker Compose 部署方式见 [deploy/docker/README.md](./deploy/docker/README.md)，推荐线上环境按文档配置数据卷、端口和 IPv6 网段。

本地开发：

```bash
npm install
npm run dev
```

构建与检查：

```bash
npm run build
npm run check-types
npm run lint

# 全部构建
FN_KNOCK_FORCE_ARTIFACT_REBUILD=1 bun run fn-knock:deploy-all
```

## 仓库结构

| 路径                     | 说明                             |
| ------------------------ | -------------------------------- |
| `apps/server-admin-rs`   | Rust 管理后端服务                |
| `apps/server-admin-view` | Vue 管理后台                     |
| `apps/server-auth-view`  | 认证页前端                       |
| `apps/fn-knock-desktop`  | Windows Tauri 状态端与 NSIS 配置 |
| `apps/fn-knock`          | 飞牛 FPK 打包目录                |
| `apps/fn-knock-docker`   | Docker 版 FPK 打包目录           |
| `deploy/docker`          | Docker 镜像、Compose 和发布脚本  |
| `packages/admin-shared`  | 管理端共享组件                   |
| `packages/frontend-core` | 前端共享 API、认证和工具代码     |
| `packages/ui-vue`        | Vue UI 基础组件                  |

## 常用命令

| 命令                                  | 用途                        |
| ------------------------------------- | --------------------------- |
| `npm run dev`                         | 启动开发服务                |
| `npm run build`                       | 构建全部应用和包            |
| `npm run check-types`                 | 类型检查                    |
| `npm run lint`                        | 运行 lint                   |
| `npm run fn-knock:assemble-runtime`   | 组装共享运行时目录          |
| `npm run fn-knock:build-package`      | 构建 FPK 打包目录           |
| `npm run fn-knock:deploy`             | 构建、远端打包并部署 FPK    |
| `npm run fn-knock:docker:build`       | 本地构建 Docker 镜像        |
| `npm run fn-knock:docker:up`          | 启动本地 Docker 环境        |
| `npm run fn-knock:docker:hub-publish` | 发布 Docker Hub 镜像        |
| `npm run fn-knock:windows:test`       | Windows 原生测试与构建检查  |
| `npm run fn-knock:windows:build`      | Windows x86_64 完整发布构建 |

## 开发提示

- 需要 Node.js `>= 18`，仓库使用 npm workspace 和 Turborepo。
- FPK / Docker 打包会读取 `Go-Reauth-Proxy` 的网关二进制；默认查找相邻目录 `../Go-Reauth-Proxy`，也可以通过 `FN_KNOCK_GO_REAUTH_PROXY_DIR` 指定路径。
- Windows 版的架构、数据目录、服务命令和签名顺序见 [apps/fn-knock-desktop/README.md](./apps/fn-knock-desktop/README.md)。发布包必须由原生 Windows Server 2022 x64 CI 生成。
- 远端部署脚本带有本地开发默认值，正式使用前请按自己的机器修改 `FN_KNOCK_REMOTE_HOST`、`FN_KNOCK_DOCKER_REMOTE_HOST` 等环境变量。

## 支持项目

如果 `fn-knock` 对你有帮助，可以请作者喝杯咖啡，支持项目继续维护。

<p align="center">
  <img src="./assets/QR_PAY.JPG" alt="赞助二维码" width="260">
</p>

## License

[MIT](./LICENSE)
