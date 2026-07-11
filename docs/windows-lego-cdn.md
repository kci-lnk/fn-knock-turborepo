# Windows Lego CDN 资源

Windows 安装包不包含 Lego。管理员在“ACME / SSL”页面点击“下载并初始化”后，服务端从 fn-knock CDN 获取资源。

## 必须上传的文件

```text
https://cdn.fnknock.cn/alldata/lego/v5.2.0/windows/x86_64/lego_v5.2.0_windows_amd64.zip
https://cdn.fnknock.cn/alldata/lego/windows/x86_64/stable.json
```

不需要上传 `stable.json.sig`。ZIP 应直接使用 Lego 官方 `v5.2.0` Windows AMD64 发布包，不能修改内部文件。

`stable.json`：

```json
{
  "schema_version": 1,
  "resource": "lego",
  "version": "5.2.0",
  "platform": "windows",
  "architecture": "x86_64",
  "file_name": "lego_v5.2.0_windows_amd64.zip",
  "url": "https://cdn.fnknock.cn/alldata/lego/v5.2.0/windows/x86_64/lego_v5.2.0_windows_amd64.zip",
  "sha256": "<ZIP 文件的小写 SHA-256>",
  "size": 0,
  "executable": "lego.exe",
  "license": "MIT",
  "source": "https://github.com/go-acme/lego/releases/tag/v5.2.0"
}
```

将 `size` 替换为 ZIP 的实际字节数。客户端固定校验 CDN 域名、版本、平台、架构、文件名、URL、大小、SHA-256、安全 ZIP 路径以及 `lego.exe --version`。

PowerShell 生成字段：

```powershell
$file = Get-Item .\lego_v5.2.0_windows_amd64.zip
$file.Length
(Get-FileHash -Algorithm SHA256 $file.FullName).Hash.ToLowerInvariant()
```

CDN 必须保留原始 JSON/ZIP 字节、正确返回 `Content-Length`，并禁止将 ZIP 请求重写为 HTML 错误页。
