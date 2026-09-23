# audit6 smoke 与 TTL0 六对独立复核

状态：**passed**。只读采集与离线复算，不启动业务服务、不重新运行实验。

固定计划 SHA256 `87e69d8e1dedce80e7ddb6ea574fa1145975123cdca60b3b662f0637a78002ce`；baseline Go4d15fa3 / Rust5fdd，candidate Go66998225 / Rust5fdd（仅嵌入新 gateway commit 重建）。两角色 reader1/z/Tokio2、正常 capacity unset，c16、2 clients、N1000 accounts/sessions、grants100、TOTP、mobility=false；本报告两组 TTL 均为 0。

## smoke

状态：passed；已写 18 行，validation 通过 18 行。

Exact contract：True；冻结 checker 退出 0；远端 gate 与离线复算一致：True；outcome 已采集制品哈希一致：True。
warm/load 成功请求 21,780/66,105，失败 0/0；load client event-loop delay 最大 20.021 ms，启动延迟最大 3.227 ms，采样间隔最大 212.525 ms。四项 load quality 全通过：True。

| 路由 | baseline/candidate 成功数 | 状态码 |
|---|---:|---|
| bootstrap | 1207 / 1202 | 200 / 200 |
| session_api | 942 / 904 | 200 / 200 |
| session_hit | 1502 / 1472 | 200 / 200 |
| grant_hit | 3056 / 3046 | 200 / 200 |
| auto_ip_hit | 544 / 528 | 200 / 200 |
| auto_ip_miss | 672 / 657 | 302 / 302 |
| challenge | 9598 / 9531 | 200 / 200 |
| session_miss | 7507 / 7502 | 302 / 302 |
| grant_miss | 7631 / 8604 | 302 / 302 |

302 仅对应预期登录重定向；JSON/API 和业务 origin 分别由冻结 harness 验证。短 smoke 只说明有效性。

## session-ttl0

状态：passed；已写 12 行，validation 通过 12 行。

Exact contract：True；冻结 checker 退出 0；远端 gate 与离线复算一致：True；outcome 已采集制品哈希一致：True。
warm/load 成功请求 117,776/356,342，失败 0/0；load client event-loop delay 最大 20.218 ms，启动延迟最大 1.354 ms，采样间隔最大 214.185 ms。四项 load quality 全通过：True。

| pair / 次序 | RPS base→fix | Δ RPS | P99 ms base→fix | Δ P99 | 合计峰值 MiB base→fix | Δ RSS | CPU ms/成功 base→fix (Go+Rust) |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 AB | 494.42→496.37 | +0.40% | 42→42 | +0.00% | 80.30→79.71 | -0.73% | 3.361→3.326 |
| 2 BA | 494.19→498.16 | +0.80% | 42→43 | +2.38% | 79.04→80.96 | +2.43% | 3.366→3.331 |
| 3 AB | 491.62→499.03 | +1.51% | 43→42 | -2.33% | 79.38→81.13 | +2.20% | 3.368→3.317 |
| 4 BA | 495.84→486.79 | -1.83% | 42→43 | +2.38% | 81.33→80.56 | -0.95% | 3.352→3.396 |
| 5 AB | 494.29→496.30 | +0.41% | 43→43 | +0.00% | 79.81→81.22 | +1.77% | 3.348→3.326 |
| 6 BA | 495.19→494.82 | -0.08% | 42→43 | +2.38% | 80.08→79.55 | -0.66% | 3.344→3.334 |

冻结 compareRuns 的六对变化中位数：RPS +0.40%，P99 +1.19%，合计 RSS +0.55%。门槛分别 ≥−5%、≤+10%、≤+5%；不要求额外改善，也不把单对越线误写成整组失败。

冻结工具的配对 bootstrap 95% 区间：RPS -0.95% 至 +1.16%；P99 -1.16% 至 +2.38%。本批两区间均包含零，不声称修复带来显著加速。

| 角色绝对值中位数 | Go CPU ms/成功 | Rust CPU ms/成功 | 合计平均核数 | Go 峰值 MiB | Rust 峰值 MiB |
|---|---:|---:|---:|---:|---:|
| baseline | 0.453 | 2.904 | 1.659 | 41.77 | 38.27 |
| candidate | 0.448 | 2.879 | 1.652 | 41.78 | 38.73 |

CPU 与单进程 RSS 的每对原始值及变化完整保存在 SUMMARY.json；没有为 CPU 增加新验收门槛。

## 范围限制

- 只分析本批的 smoke 和 TTL0；TTL1、soak、recovery 不在本报告范围。
- TTL0 六对评价正确性修复相对上一优化候选的增量非回退，不是 d4→优化版收益。
- 稳态 session 命中负载不主动触发注销；不能替代 logout/singleflight 并发正确性回归。
- RSS 使用 Go/Rust 各自负载采样峰值之和，不代表同一时刻峰值；role 中位数和 paired change 中位数不同。
- CPU ms/成功请求 = cpu_ms_per_1000_success / 1000；平均核数 = CPU 秒 / 实际负载秒，包含进程后台工作，不是单次 Rust 请求的纯CPU。
- client/fixture 进程在 case 内复用，产品进程逐 trial 重启；采样 health_ok 不等于所有缓存健康组件均 healthy。
