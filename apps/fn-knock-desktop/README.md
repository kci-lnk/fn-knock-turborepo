# fn-knock for Windows

Windows x86_64 版由一个明确标记为 unsigned 的 NSIS 安装包交付，运行时包含三个进程：

- `fn-knock.exe`：纯 Rust + Win32 API 管理程序、托盘和更新入口；release 构建默认请求管理员权限，退出不停止网关。
- `fn-knock-service.exe`：唯一注册到 SCM 的 `FnKnock` 服务，账户为 `NT SERVICE\FnKnock`。
- `fn-knock-gateway.exe`：由 Rust 服务监督并加入 kill-on-close Job Object 的 Go 数据面。

程序安装在 `%ProgramFiles%\Knock 敲门`。配置、SQLite、证书、WAF、日志、状态和回滚数据位于 `%ProgramData%\FnKnock`。管理、Rust API、认证、Go gRPC 和代理的默认端口依次为 `7991`、`7998`、`7997`、`7996`、`7999`；代理入口默认监听所有 IPv4 网络接口（`0.0.0.0`）。

桌面管理程序通过 SCM API 启停和重启服务，端口配置采用原子写入、就绪检查与失败回滚。清除管理密码也由管理程序完成，子进程统一使用 `CREATE_NO_WINDOW`，不会弹出 CMD 或 PowerShell 窗口。7991 管理后台在系统浏览器中打开，桌面程序不包含 WebView 或前端运行时。

管理程序原生支持中文简体、中文正體、English、한국어与日本語。首次启动默认跟随 Windows 首选 UI 语言，不受支持的系统语言回退到中文简体；窗口右上角的语言按钮可即时切换，也可恢复“跟随 Windows”。手动选择按 Windows 用户保存在 `%APPDATA%\FnKnock\windows-manager-locale.txt`，不会改写服务端配置。

## 原生 Windows 构建

本机开发和安装测试可从仓库根目录一键生成 unsigned NSIS 安装包：

```powershell
npm run fn-knock:windows:build
```

命令允许工作树包含当前开发改动，并要求系统已安装 NSIS 3；也可通过 `FN_KNOCK_MAKENSIS` 指定 `makensis.exe`。安装包输出到 `dist\windows\fn-knock-<version>-windows-x86_64-unsigned-setup.exe`。

发布门禁只在 Windows Server 2022 x64 runner 上执行：

```powershell
npm ci
./scripts/fn-knock-windows.ps1 -Mode Build -BundleInstaller -RequireCleanTree -GoRepository C:\src\Go-Reauth-Proxy
```

统一发布工作流在开始时解析并冻结 `Go-Reauth-Proxy/main` 的 40 位提交 SHA，并从根目录 `version.json` 注入统一版本。CI 会重新生成 Go protobuf stub 并拒绝协议漂移，再运行 Go、Rust、Vue、原生管理程序、Windows 服务崩溃恢复和安装/卸载 smoke test。

发布顺序固定为：

1. 在原生 Windows Server 2022 x64 runner 上构建 GUI、Rust 服务、Go 网关和 `rust-acmesh.exe`。
2. 原生 NSIS 3 脚本生成 per-machine setup；桌面程序和安装器均不依赖 WebView 或其他 GUI 框架。
3. 执行运行时、安装和卸载 smoke test，并校验 MZ/PE 头。
4. 对最终字节生成 SHA-256；文件名和元数据均保留 `unsigned`，不得伪装为已签名包。

`scripts/fn-knock-windows-finalize.ps1` 输出 EXE、SHA-256、unsigned `release.json` 和 unsigned `updater.json` 作为发布汇总输入；GitHub Release 仅公开 EXE，校验信息统一收录于根级 `release-manifest.json` 和 `SHA256SUMS`。Windows 安装包同时内置 `rust-acmesh.exe`，用于 DNS-01 证书申请。
