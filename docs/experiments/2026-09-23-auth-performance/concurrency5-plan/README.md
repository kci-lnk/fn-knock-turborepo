# 最终 v5 的同规模并发补充短测

状态：`prepared_not_executed`。本目录仅准备计划，没有上传或执行负载，不修改冻结的 post5 或 scripts5。执行状态和失败证据只写入新输出目录。

新增原因：已有 c1 数据使用 N1，c64 使用 N100，不能与正式 c16/N1000 拼成同规模曲线。补充 c1、c16、c64 三档，连 c16 也重做相同 warm5/load15 协议，保持规模和测量时长一致。正式收益仍只来自原六对主矩阵；本补充每档只有一对，不提供收益验收或默认参数调整依据。

| 项目 | 固定值 |
| --- | --- |
| 比较 | 最终 variants5 的 base5 → candidate5 |
| 并发顺序 | c1 → c16 → c64，串行；每档 8 trials，共 24 trials |
| 路由 | bootstrap、session_hit、grant_hit、auto_ip_hit |
| 每路由 | 1 pair，baseline 后 candidate，各 warm5 秒 / load15 秒 |
| 种子 | TOTP；accounts=1000、sessions=1000、ordinary grants=100 |
| Mobility | 冻结 seed.py 显式 `session_ip_mobility_enabled=False`，不依赖产品默认 |
| 缓存 | positive/unauthorized TTL 均为0，测量前后读取运行值确认 |
| 编译和 reader | 两端 Rust z；候选 reader1；旧基线原单 reader |
| 运行参数 | Tokio workers=2；normal capacity 无覆盖；profile/recovery 关闭 |
| 客户端 | CLI clients=2，最多2个 Worker；c1 实际1个，c16为8+8，c64为32+32 |

每个 trial 由原 harness 创建独立进程和数据库。1 pair 固定 AB 顺序，不能估计顺序偏差；各档连续执行也不能排除随时间变化的机器噪声。24 trials 的 warm/load 请求窗口合计480秒，进程启动、健康采样及清理会增加总用时。无有限续期场景，`--renewals64` 不生成续期 token，实际种子字段须为0。

`variants5.json` 与 `/tmp/fn-knock-auth-variants5-20260923.json` 逐字节相同。`manifest.json` 固定两仓源码、四个产品二进制 SHA256、12 个 scripts5 文件 SHA256、配置和本目录文件 SHA256；另单独固定 `authperf-control` 的 SHA256，在 preflight 及实际 harness manifest 中核对。候选 Rust 为 `5fddf896eaf9b1cf3eb300c08315320498c943b8`、Go 为 `4d15fa32764e26df58b16930d0e2503a90880002`；基线为 Rust `d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4`、Go `92d4c0cb5495d57801d52893a8f0e8496a1c9182`。沿用匹配的 release 构建和链接参数，不重编译。

后续由操作者将本目录原样放到目标 Linux 主机的 `/tmp/fn-knock-auth-performance-20260923/concurrency5-plan/`，在 post5 和其他负载完全停止后才执行。以下命令是交付说明，本次未运行：

```sh
cd /tmp/fn-knock-auth-performance-20260923/concurrency5-plan
bash run-concurrency5.sh show
bash run-concurrency5.sh preflight
bash run-concurrency5.sh run \
  /tmp/fn-knock-auth-performance-20260923/concurrency-v5-supplement-UNIQUE
```

将 `UNIQUE` 换为新的批次名；输出目录必须不存在。默认使用 PATH 中的 Node；若 post5 指定了 `AUTH_PERF_NODE`，此处沿用相同可执行文件。默认 action 是 show，不启动负载；preflight 只检查本地文件和 SHA。runner 与 post5 共享 `/tmp/fn-knock-auth-post-experiment.lock`，并拒绝已有 harness；不会等待、终止其他实验或自动重试。SIGINT/SIGTERM 只转发给本 runner 当前拥有的 harness，由原 harness 清理其隔离进程。

每档完成后先检查恰好4路由×2角色×pair0的8条结果、配置/种子/源码及制品身份、实际 runtime env、TTL读回和质量；再调用 `scripts5/check-auth-performance.mjs`，**不传** `--require-six-pairs` 或 `--require-improvement`。统计和语义判断继续复用既有工具，附加检查只防止缺整路由或混用规模/身份。任何请求、质量、进程、身份、完整性或 checker 失败立即停止，保留原始 result/manifest/log，不补跑、不跳过失败路由，不创建后续并发档。

结果位于新批次的 `c1/`、`c16/`、`c64/`，旁边保存 `.run.log`、`.contract.json`、`.validity.json`、`.report.md`。仅三档都完成才有 `completed-utc.txt`；`completed-groups.txt` 记录已完成前缀。读取图表时必须保留同规模、同协议和单对探索标识。c64 若出现503或压力，应保留为有效观察到的失败并诊断，不能降低并发重跑后替换它。
