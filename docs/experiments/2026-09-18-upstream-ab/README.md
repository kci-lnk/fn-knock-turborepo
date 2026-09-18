# 192.168.31.98 隔离 A/B 实验

这些脚本是本次调查的可复核实验记录，不是生产启动脚本。固定工作目录为 `/tmp/fn-knock-ab-20260918`，要求 Linux root、Python 3、gcc、iproute2、unshare。所有测试必须在独立网络命名空间内执行，避免误连正式网关或修改正式防火墙。不要复制正式业务数据库；本次只使用全新实例生成的合成数据。

## 制品与变体

`artifacts.json` 记录正式 FPK 的 SHA256、release-manifest 主仓库和 Go 提交。FPK 解出 app.tgz 后，分别放在 `v2412/`、`v2415/`。两个 Go 正式制品均使用 Go 1.26.7。

- 同版本组：Go 和 Rust 都是正式 FPK 原始二进制。
- 分配器组：运行目标设置为 `linux`，以关闭 fnOS 宿主专用任务；两个 Tokio worker、认证桥并发 32 固定。通过 `GLIBC_TUNABLES` 设置 tcache，并用 `allocator.c` 拦截唯一的 M_MMAP_THRESHOLD 参数，原样转调真实 mallopt。日志打印实际阈值。因此它隔离的是 FPK 分配器组合，不是完整 FPK 宿主行为。
- 原样交叉组合会被 bundle version 校验拒绝，不能直接测试。`mixed.py` 的 Go 为对应提交的实验重建，**只通过 ldflags 修改版本/commit 身份标识以通过隔离实验的启动校验**；Rust 仍用正式制品。此类 Go 不可部署，也不能声称是正式发布包。

重建命令（分别在 Go `2f52858` / `e3a7fe7` 导出的源码目录运行）：

```sh
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -trimpath \
  -ldflags '-s -w -X go-reauth-proxy/pkg/version.Version=2.4.15 -X go-reauth-proxy/pkg/version.Commit=e3a7fe7470c6f3df1881fd619b528f3715964d2f' \
  -o /tmp/fn-knock-ab-20260918/old-as-new ./cmd/server
# 新源码配旧 Rust：Version=2.4.12，Commit=2f528589de1b49618d64edbdc91fbda20a8bc131，输出 new-as-old。
```

## 负载与观测

`matrix.py` / `mixed.py`：真实 Go → Rust 认证入口，轮询 bootstrap、session、captcha config、challenge、OIDC provider 列表、认证 HTML。未登录 session 的 401 是预期结果。每组 16 并发 20 秒、64 并发 20 秒、空闲 3 秒后 8 并发 3 秒。每 200 ms 采样 CPU ticks、RSS、FD，记录状态码、上游错误分类、连接异常和延迟。进程在组间重启，重复旧/新关键组检查顺序漂移。

`business.py`：从本次 pilot 全新 SQLite 数据库复制合成模板，加入 513 条映射（只有 business.local 被请求）；这不是业务机器配置。混合上述认证负载及 HTTPS/HTTP2 上游的 64 KiB 正文、100 ms 慢响应、分块响应。预检要求服务端确实观察到 HTTP/2.0。正常负载后关闭独立上游监听，执行 5 次请求，再恢复监听执行 5 次，检查错误分类和恢复能力。

`upstream.go`：只监听命名空间内的 28081/28082，后一个是本实验的启停控制。使用 Go httptest TLS 证书，不涉及真实飞牛服务和证书。

为可比性，各组串行运行；不要同时执行不同脚本。路径 `runs/<label>` 必须不存在，防止覆盖证据。首次预检将 session 401 误当失败，修正预期后重跑；该预检没有进入负载统计。原样交叉组合的版本拒绝也不计作请求故障。业务预检首次仅修改 typed config 而未同步 legacy shadow，被程序一致性检查恢复；修正合成数据种子后重跑，并强制验证上游 HTTP/2 响应以防误测无映射路径。该预检未进入负载统计。

```sh
gcc -shared -fPIC -O2 allocator.c -ldl -o allocator.so
# 在 Linux 命名空间内：
unshare -n sh -c 'ip link set lo up; python3 /tmp/fn-knock-ab-20260918/matrix.py old_old new_new_legacyalloc new_new_tcache0 new_new_fpkalloc new_new_fpkalloc_repeat old_old_repeat'
unshare -n sh -c 'ip link set lo up; python3 /tmp/fn-knock-ab-20260918/mixed.py'
# business 需在同一个 namespace 中先运行 upstream，再运行 business.py，并用 trap 结束 fixture。
```

限制：短时合成、闭环负载；不是真实账号登录、所有高级认证规则、真实终端 SSH、DNS、多天运行或证书恢复的全覆盖测试。loopback 客户端亦不能代表外网 WAF/认证策略。Python 客户端和服务共用测试机 CPU，延迟数字只用于同机对照，不作为产品性能指标。没有正式系统服务重启和数据迁移。

## DNS 和本机构建补测

`dns.py` 是隔离 namespace 内的最小 UDP DNS：仅对 ab-upstream.test 返回 loopback A，AAAA 返回空答案；故障标记存在时返回 SERVFAIL。`run-dns.sh` 额外使用私有 mount namespace，将测试 resolv.conf 绑定到子命名空间的 /etc/resolv.conf；宿主配置前后 SHA256 校验相同。`dns-matrix.py` 对两份正式发布组合执行短负载，然后断开旧上游连接、在上游仍可用时注入 DNS 故障，之后恢复 DNS。结果用于验证 dns_temporary 分类及恢复，不用于吞吐比较。

`machine.py` 使用从当前机器复制出的 Rust 二进制及正式 v2.4.15 Go，独立数据和端口，分配参数仍固定为新 FPK 组合。当前机器 Rust 的 SHA256 为 f3268ae61b7ef1f04fcfbdb1c284bbe94126acf4c7f96401bdbf5d9010f8b035；正式 FPK Rust 为 518ab1dce2ece0f1dd51af09bf689725cc1c05c8c93bfd51a03d7af58ddf324d。两者 rustc 均为 1.96.0，但前者 Homebrew clang/LLD 21.1.8，后者 clang/LLD 21.1.0。当前机器构建的源提交不能仅凭这些信息确定。

追加鉴权失效实验：`auth-failure.py` 与 `auth-failure-2412.json` / `auth-failure-2415.json`。正式旧新版在 Rust 暂停/恢复/退出时的三条鉴权访问路径对照，详见主调查报告末节；这 24 次阶段探测不计入上方正常负载请求总数。重点结果：独立鉴权域名反代 Rust AuthPort，在 Rust 退出时确实显示 20005；鉴权 RPC 则返回 Authentication Service Unavailable。

追加外部鉴权/数据库竞争：`auth-pressure.py` 与 `auth-pressure-repeat.py`，原始结果为对应 `auth-pressure-unthrottled-*.json` 和 `auth-pressure-repeat-*.json`。合成配置关闭反代限流，64 并发测试；pressure 脚本含 SQLite 写锁故障，repeat 仅正常负载并记录异常样本。没有生产配置变更。正常四组共 37624 请求、2 个 503、无 20005 或退出；锁竞争两版均产生超时/错误，详见主报告。
