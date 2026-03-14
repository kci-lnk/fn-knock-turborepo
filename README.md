# fn-knock-turborepo

`fn-knock` 的 Turborepo 工作区，包含：

- `apps/server-admin` (Node.js 后端)
- `apps/server-admin-view` (管理前端)
- `apps/server-auth-view` (认证前端)
- `apps/fn-knock` (FPK 打包目录)

## 一键彻底部署（推荐）

在仓库根目录执行：

```bash
npm run fn-knock:deploy
```

该命令会按顺序完成：

1. 本地构建打包资源（前端 + 后端产物同步到 `apps/fn-knock/app`）
2. 上传到远端，分别生成 `amd64` / `arm64` 两个 FPK，并拉回本地
3. 卸载旧版本并安装/启动远端生成的 `amd64` FPK
4. 校验远端安装文件 `index.cgi` 的哈希是否与本地一致

远端双架构打包时会自动做差异化处理：

- `amd64` 包保留 `go-reauth-proxy-linux-amd64`，删除 `go-reauth-proxy-linux-arm64`
- `arm64` 包保留 `go-reauth-proxy-linux-arm64`，删除 `go-reauth-proxy-linux-amd64`
- `amd64` 包会把 `manifest` 中的 `platform` 改成 `x86`
- `arm64` 包会把 `manifest` 中的 `platform` 改成 `arm`
- `arm64` 包会把 `manifest` 中的 `install_dep_apps` 改成 `nodejs_v20`
- `amd64` 包仍保留 `install_dep_apps=nodejs_v20:redis`

## 分步命令

```bash
# 仅构建本地打包目录
npm run fn-knock:build-package

# 本地构建 + 远端双架构 fnpack 打包 + 拉回 FPK
npm run fn-knock:fpk:remote

# 远端安装并查看运行日志
npm run fn-knock:install:remote

# 校验安装内容（hash + 关键脚本片段）
npm run fn-knock:verify:remote
```

## 可配置环境变量

部署脚本支持以下环境变量覆盖默认值：

- `FN_KNOCK_REMOTE_HOST`，默认 `root@192.168.31.98`
- `FN_KNOCK_REMOTE_DIR`，默认 `/tmp/fn-knock-fpk`
- `FN_KNOCK_APP_NAME`，默认 `fn-knock`
- `FN_KNOCK_LOCAL_APP_DIR`，默认 `apps/fn-knock`
- `FN_KNOCK_LOCAL_FPK_PATH`，默认 `apps/fn-knock/dist/fn-knock.fpk`
  实际会拉回为 `apps/fn-knock/dist/fn-knock-amd64.fpk` 和 `apps/fn-knock/dist/fn-knock-arm64.fpk`

示例：

```bash
FN_KNOCK_REMOTE_HOST=root@192.168.31.99 npm run fn-knock:deploy
```

## 依赖要求

- 本机可用 `node`, `npm`, `rsync`, `ssh`, `scp`
- 远端可用 `fnpack`, `appcenter-cli`
- 本机对远端 root SSH 免密（或已完成可交互认证）
