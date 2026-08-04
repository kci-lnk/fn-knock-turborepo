# fn-knock for macOS

fn-knock 支持 macOS 13 及以上版本，分别提供 Intel (`amd64`) 和 Apple Silicon (`arm64`) 压缩包，不提供 `.app` 或 `.pkg`。

```sh
curl -fsSL https://cdn.fnknock.cn/macos/install.sh | sudo bash
```

安装后使用 `sudo knock` 管理启动、停止、配置、升级、回滚和卸载。管理面板默认仅监听 `127.0.0.1:7991`，macOS 运行时不支持 iptables、主机防火墙管理或网页终端。

## 文件位置

- 程序：`/Library/Application Support/FnKnock`
- 配置：`/Library/Application Support/FnKnock/config/fn-knock.env`
- 数据：`/Library/Application Support/FnKnock/data`
- 日志：`/Library/Logs/FnKnock`
- LaunchDaemon：`/Library/LaunchDaemons/cn.fnknock.service.plist`

## 未签名发行说明

macOS 压缩包不使用 Apple Developer ID 签名或公证。手动从浏览器下载时，应先核对 GitHub Release 中的 `SHA256SUMS`。如果校验无误但 Gatekeeper 因 quarantine 阻止运行，可对已解压目录手动执行 `xattr -dr com.apple.quarantine fn-knock`；安装器不会自动移除 quarantine。
