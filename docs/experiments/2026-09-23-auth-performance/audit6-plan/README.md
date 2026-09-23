# audit6：注销缓存修复后 Linux 对照准备包

以 `manifest.json.status` 区分两个制备阶段：**template_pending_identities_do_not_execute** 表示候选身份仍有占位；本地 finalize 成功后产生 **prepared_not_executed** 的独立 ready 包。两种状态都不表示已经上传或执行。`preflight` 和 `run` 会在占用锁、创建结果或启动负载前拒绝尚未冻结的占位身份。

比较目的：评价 Go 注销失效/旧 RPC 回填及 singleflight 代际修复的增量代价。baseline 是先前优化候选 **Go 4d15fa3 + Rust 5fddf896** 的 candidate5 原制品，**不是 d4 原始 Rust / Go92 基线**。候选 Rust 源码仍为 5fdd，仅 `FN_KNOCK_GATEWAY_COMMIT` 随修复后的 Go commit 变化并重建。既有 273 个 trial 保持独立，不与本批合并。正确性来自专门的并发回归测试；本批稳定 session 负载不主动注销，不能代替竞态回归。

## 固定协议

所有阶段 c16、最多两个客户端 worker（本批均实际为两个）、1000 accounts、1000 sessions、100 ordinary grants、TOTP、显式 mobility=false（冻结 seed 实现）、reader1、Rust z/fat-LTO/1 codegen unit、Tokio2。正常桥接容量环境变量缺省；仅 recovery 显式 32。positive 与 unauthorized cache TTL 均按表设置并在 trial 前后读回验证。不更改产品默认值。profiling 始终关闭。

| 串行阶段 | 路由 | pair / roles | warm/load 秒 | TTL | trials / 判断 |
|---|---|---|---|---|---|
| smoke | bootstrap, session_api, session_hit, grant_hit, auto_ip_hit, auto_ip_miss, challenge, session_miss, grant_miss | 1 / 双角色 | 1/3 | 0 | 18；只检查有效性 |
| session-ttl0 | session_hit | 6 / 双角色 | 20/60 | 0 | 12；原六对非回退门槛 |
| session-ttl1 | session_hit | 6 / 双角色 | 20/60 | 1 | 12；原六对非回退门槛 |
| soak-ttl1 | session_hit | 1 / 仅 candidate | 20/1800 | 1 | 1；30 分钟完成及原有效性检查 |
| recovery | auto_ip_hit | 1 / 双角色 | 1/3 | 0 | 2；capacity32，原饱和恢复协议 |

合计 **45 trials**，预计约 65 分钟加启动、采样收尾及检查时间。每个 case 固定 pair→route→AB/BA；六对恰好 12 行，smoke 恰好 18 行，其他恰好 1/2 行。recovery 的正常 warm/load 是 c16，内部 burst 为 c64/2 秒，之后 c1 要求在 10 秒内完成连续 2 秒成功窗口。恢复计时从 burst worker 清理结束后开始，包含成功窗口，不代表首次请求恢复延迟。

两组六对仅调用冻结 `check-auth-performance.mjs --require-six-pairs`，**不加 `--require-improvement`**：配对变化的中位数 RPS ≥ −5%、P99 ≤ +10%、Go/Rust 各自采样 RSS 峰值之和 ≤ +5%，RSS 缺失失败。无需额外 +10% 吞吐或 −15% P99 收益。不是每对都不得回退；合计 RSS 也不是两进程同时峰值。smoke/recovery 用无 flags 的有效性检查；soak 不送入需要配对的 checker。

soak 由冻结 `auth-performance-soak.mjs` 读取其单条原始 trial 生成 summary，再显式要求完成标志 true，并核对 results 的时长、成功数和质量。资源趋势单独展示，不新增 RSS 增长阈值，也不宣称无泄漏。recovery 记录 burst 503 数和压力是否被实际观察；沿用原 passed 协议，不事后新增 503 数量验收门槛。若没有 503，只可说该负载下恢复 probe 完成，不能称已验证实际饱和。health_ok 也不等于所有缓存健康组件均 healthy。

## 本地冻结步骤（不会运行实验）

1. 将 `candidate-input.example.json` 复制到包外，填入 root 提供的完整 Go commit、Go/Rust SHA256、两个二进制绝对路径及对应 build manifest 绝对路径。支持现有 Go 单角色 `build.json` 和 Rust `compiler-build-manifest.json` 格式；不是 roles[] 总 manifest。
2. 本地执行：

```sh
python3 -B /tmp/fn-knock-auth-audit6-prep/finalize.py \
  --identity /absolute/candidate-input.json \
  --out /tmp/fn-knock-auth-audit6-ready
python3 -B /tmp/fn-knock-auth-audit6-ready/check-plan.py verify
bash /tmp/fn-knock-auth-audit6-ready/run-audit6.sh show
```

finalize 拒绝已有输出，读取并校验 baseline/candidate 四制品实际字节与 builder 记录及 ELF64 x86-64 头；核对两版 Go 的 `-buildvcs=false -trimpath -s -w` 及 Version=2.4.15/各自 Commit，核对 Rust 源码 5fdd、各自 gateway commit、locked/release/target/z/fat/1 和同 toolchain/Cargo.lock。新包复制四份 build manifest，固定两份 config、所有文件 SHA、四制品 SHA/大小及 12 个冻结工具 SHA、control SHA。源码到制品关系依赖已保存 builder 声明及构建命令；hash 本身不证明编译过程可信，不把 metadata 当独立来源证明。

包内不复制二进制、数据库、缓存或 pyc。`manifest.json` 不自哈希；finalize 打印它的最终 SHA，root 审阅后作为外部冻结身份。prepared_not_executed 描述制备时状态，实际执行结果另存到输出目录。

## Linux 执行入口（本次未执行）

root 审阅冻结后，将 ready 包原样安装到：

`/tmp/fn-knock-auth-performance-20260923/audit6-plan`

候选制品路径固定在同 root 的 `candidate6/{go-reauth-proxy,server-admin-rs}`。沿用只读 `scripts5` 与既有 control/assets，不修改 scripts5、post5、以前数据或冻结包。

```sh
bash /tmp/fn-knock-auth-performance-20260923/audit6-plan/run-audit6.sh preflight
bash /tmp/fn-knock-auth-performance-20260923/audit6-plan/run-audit6.sh run
```

无默认执行、无按阶段跳转、无输出路径覆盖、无恢复运行或重试。输出固定新目录 `.../audit6-final`，已有即拒绝。使用所有实验共用的 `/tmp/fn-knock-auth-post-experiment.lock` 非阻塞 flock；发现另一 harness 活跃即退出，绝不终止其他实验。TERM/INT 只转发给本 runner 当前的隔离 wrapper（该 wrapper exec 到 Node，由 harness 清理自己的进程和 namespace）。正常容量、reader、Tokio、allocator 和 Go GC 相关继承环境先清除，再由冻结 config 显式设置。

任一 harness、exact contract、gate、report 失败便停止后续 case。即使负载或 gate 失败，仍尽量生成当前已保存 results 的 report、contract/gate stderr 和 `{case}.outcome.json`，并保留 batch outcome；不会改门槛或自动补跑。退出 20/21/22/23 分别表示 harness/contract/gate/report，24 表示整批目录/进程复用检查失败，3 表示锁或冲突，2 表示用法或已有输出，130/143 表示中断。

## 离线复核

`check-plan.py contract CASE LOCAL_CASE_DIR` 只读取 manifest/results：完整 config/options、角色 source/build/env、四制品和八个运行时工具 SHA/control、exact trial 数量/顺序、seed、前后 TTL、正常 warm/load 零错及质量、进程 PID/start_ticks 一致和每 trial 新进程、recovery 完成窗口。离线输入可在任意本地目录；其 manifest 记录的原远端 config/output 路径必须匹配冻结计划。

```sh
python3 -B /path/to/frozen/audit6-plan/check-plan.py contract session-ttl0 /local/session-ttl0
node /path/to/scripts5/check-auth-performance.mjs /local/session-ttl0/results.json --require-six-pairs
node /path/to/scripts5/report-auth-performance.mjs /local/session-ttl0/results.json
```

本地 `test-contract.py` / `verify-gates.mjs` 是显式 synthetic-contract-only 校验，不连接服务、不启动进程负载、不产生性能结论。模板状态只允许查看和这些离线校验，禁止 run。
