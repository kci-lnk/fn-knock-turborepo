# fn-knock for Windows

Windows x86_64 版由一个完整签名安装包交付，运行时包含三个进程：

- `fn-knock.exe`：纯 Rust + Win32 API 管理程序、托盘和更新入口；release 构建默认请求管理员权限，退出不停止网关。
- `fn-knock-service.exe`：唯一注册到 SCM 的 `FnKnock` 服务，账户为 `NT SERVICE\FnKnock`。
- `fn-knock-gateway.exe`：由 Rust 服务监督并加入 kill-on-close Job Object 的 Go 数据面。

程序安装在 `%ProgramFiles%\Knock 敲门`。配置、SQLite、证书、WAF、日志、状态和回滚数据位于 `%ProgramData%\FnKnock`。管理、Rust API、认证、Go gRPC 和代理的默认端口依次为 `7991`、`7998`、`7997`、`7996`、`7999`；代理入口默认监听所有 IPv4 网络接口（`0.0.0.0`）。

桌面管理程序通过 SCM API 启停和重启服务，端口配置采用原子写入、就绪检查与失败回滚。清除管理密码也由管理程序完成，子进程统一使用 `CREATE_NO_WINDOW`，不会弹出 CMD 或 PowerShell 窗口。7991 管理后台在系统浏览器中打开，桌面程序不包含 WebView 或前端运行时。

## 原生 Windows 构建

本机开发和安装测试可从仓库根目录一键生成 unsigned NSIS 安装包：

```powershell
npm run fn-knock:windows:build
```

命令允许工作树包含当前开发改动，并要求系统已安装 NSIS 3；也可通过 `FN_KNOCK_MAKENSIS` 指定 `makensis.exe`。安装包输出到 `dist\windows\fn-knock-<version>-windows-x86_64-unsigned-setup.exe`，只用于本机验证，不可作为正式发布包。

发布门禁只在 Windows Server 2022 x64 runner 上执行：

```powershell
npm ci
./scripts/fn-knock-windows.ps1 -Mode Build -BundleInstaller -RequireCleanTree -GoRepository C:\src\Go-Reauth-Proxy
```

构建需要用 40 位提交 SHA 锁定 Go 仓库，并从根目录 `version.json` 注入统一版本。CI 会重新生成 Go protobuf stub 并拒绝协议漂移，再运行 Go、Rust、Vue、原生管理程序、Windows 服务崩溃恢复和安装/卸载 smoke test。

签名顺序固定为：

1. 用 Azure Artifact Signing 给 GUI、Rust 服务和 Go 网关做 Authenticode + RFC3161 时间戳。
2. 原生 NSIS 3 脚本从已签名的三个 EXE 生成 per-machine setup；桌面程序和安装器均不依赖 WebView 或其他 GUI 框架。
3. 给最终 setup 做 Authenticode + RFC3161 时间戳。
4. 对最终字节生成 SHA-256；此后不得改动 setup。

`scripts/fn-knock-windows-finalize.ps1` 输出 EXE、SHA-256、`release.json` 和 `updater.json` 四个发布文件。Windows 安装包同时内置并签名 `rust-acmesh.exe`，用于 DNS-01 证书申请。
