# fn-knock for macOS

fn-knock 支持 macOS 13 及以上版本，分别提供 Intel (`amd64`) 和 Apple Silicon (`arm64`) 压缩包，不提供 `.app` 或 `.pkg`。

```sh
curl -fsSL https://cdn.fnknock.cn/macos/install.sh | sudo bash
```

安装后使用 `sudo knock` 管理启动、停止、配置、升级、回滚和卸载。管理面板默认仅监听 `127.0.0.1:7991`，macOS 运行时不支持 iptables 或主机防火墙管理；网页终端通过用户配置的 SSH 目标提供。

## 文件位置

- 程序：`/Library/Application Support/FnKnock`
- 配置：`/Library/Application Support/FnKnock/config/fn-knock.env`
- 数据：`/Library/Application Support/FnKnock/data`
- 日志：`/Library/Logs/FnKnock`
- LaunchDaemon：`/Library/LaunchDaemons/cn.fnknock.service.plist`

## 未签名发行说明

macOS 压缩包不使用 Apple Developer ID 签名或公证。手动从浏览器下载时，应先核对 GitHub Release 中的 `SHA256SUMS`。如果校验无误但 Gatekeeper 因 quarantine 阻止运行，可对已解压目录手动执行 `xattr -dr com.apple.quarantine fn-knock`；安装器不会自动移除 quarantine。

## 单独补发 macOS 版本

GitHub Actions 中的 `macOS Supplemental Release` 可为当前稳定版本单独构建并补发 Intel、Apple Silicon 两个包。手动运行时，`publish=false` 只构建并生成 7 天有效的发布计划；确认后以 `publish=true` 重新运行才会修改线上状态。

补发严格要求仓库 `version.json`、现有 GitHub Release 标签和线上 `latest.json.version` 三者一致。流程仅替换 `latest.json.packages.macos`，保留根字段和其他平台；同版本 COS 或 GitHub 资产若已存在但 SHA-256 不同会拒绝覆盖。Mac 单独补发不能推进稳定版版本号，新版本必须使用完整的 `release.yml` 发布。
