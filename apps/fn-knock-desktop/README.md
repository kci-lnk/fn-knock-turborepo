# FnKnock for Windows

Windows x86_64 版由一个完整签名安装包交付，运行时包含三个进程：

- `fn-knock.exe`：Tauri v2 状态窗口、托盘和更新入口；退出不停止网关。
- `fn-knock-service.exe`：唯一注册到 SCM 的 `FnKnock` 服务，账户为 `NT SERVICE\FnKnock`。
- `fn-knock-gateway.exe`：由 Rust 服务监督并加入 kill-on-close Job Object 的 Go 数据面。

程序安装在 `%ProgramFiles%\FnKnock\current`。配置、SQLite、证书、WAF、日志、状态和回滚数据位于 `%ProgramData%\FnKnock`。管理、Rust API、认证、Go gRPC 和代理的默认端口依次为 `7991`、`7998`、`7997`、`7996`、`7999`；新安装的代理默认只监听 loopback。

首次打开桌面端时，先确认代理端口、监听范围和 Domain/Private 防火墙状态，随后进入现有管理台设置管理密码。忘记密码时，在管理员 PowerShell 中运行：

```powershell
& "$env:ProgramFiles\FnKnock\current\fn-knock-service.exe" reset-panel-password
```

桌面端只在 SCM 报告 `FnKnock` 为 Running、管理端口的监听 PID 与服务 PID 一致，并且 `readyz` 的版本、控制协议及五项组件状态全部匹配时加载管理台。运行期间身份连续失配会直接销毁管理 WebView，避免停止服务后的 localhost 端口接管。

## 原生 Windows 构建

发布门禁只在 Windows Server 2022 x64 runner 上执行：

```powershell
npm ci
./scripts/fn-knock-windows.ps1 -Mode Build -GoRepository C:\src\Go-Reauth-Proxy
```

构建需要用 40 位提交 SHA 锁定 Go 仓库，并从根目录 `version.json` 注入统一版本。CI 会重新生成 Go protobuf stub 并拒绝协议漂移，再运行 Go/Rust/Vue/Tauri 测试、Windows 服务崩溃恢复 smoke test 和 release build。

签名顺序固定为：

1. 用 Azure Artifact Signing 给 GUI、Rust 服务和 Go 网关做 Authenticode + RFC3161 时间戳。
2. 从已签名的三个 EXE 生成 per-machine NSIS setup。
3. 给最终 setup 做 Authenticode + RFC3161 时间戳。
4. 对最终字节生成 Tauri updater `.sig` 和 SHA-256；此后不得改动 setup。

`scripts/fn-knock-windows-finalize.ps1` 输出固定的五个发布文件。Updater 私钥只允许来自 CI secret，客户端和发布校验器只使用公钥。
