# 鉴权性能实验结果

记录 20 个trial；0 个无效。所有变化按candidate相对baseline计算。

| 路由 / 并发 / 候选 / cache TTL / 规模 | 完整对数 | 成功吞吐变化 | P99变化 | 吞吐95%区间 | P99 95%区间 | Go峰值RSS变化 | Rust峰值RSS变化 | 合计峰值RSS变化 |
| --- | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: |
| bootstrap/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +20.8% | +0.0% | 不足6对 | 不足6对 | +0.5% | +0.2% | +0.3% |
| challenge/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +0.6% | +0.0% | 不足6对 | 不足6对 | -0.0% | +0.2% | +0.1% |
| session_api/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +37.2% | -10.0% | 不足6对 | 不足6对 | +0.1% | +0.9% | +0.5% |
| session_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +27.4% | -25.0% | 不足6对 | 不足6对 | -1.6% | -1.3% | -1.5% |
| session_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +4.2% | +0.0% | 不足6对 | 不足6对 | +1.1% | -1.4% | -0.1% |
| grant_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +147.9% | -37.5% | 不足6对 | 不足6对 | +4.8% | -1.2% | +1.8% |
| grant_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +37.7% | -20.0% | 不足6对 | 不足6对 | -1.1% | -0.8% | -1.0% |
| grant_renewal/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-117/renewals-8 | 1 | +101.3% | -50.0% | 不足6对 | 不足6对 | +3.0% | -1.1% | +1.0% |
| auto_ip_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +12.8% | -16.7% | 不足6对 | 不足6对 | +1.2% | +0.3% | +0.8% |
| auto_ip_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0 | 1 | +28.3% | -20.0% | 不足6对 | 不足6对 | +0.7% | -1.2% | -0.2% |

存在不足六对或无效trial；这些数字只用于检查工具与发现线索，不能作为已证实的优化收益。

| 路由 / 角色 | req/s | P99 ms | Go CPU ms/千成功请求 | Rust CPU ms/千成功请求 | Go 峰值→结束 RSS MiB | Rust 峰值→结束 RSS MiB | 失败请求 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: |
| bootstrap/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 237.8 | 7.0 | 546.2 | 3123.2 | 34.71 → 34.42 | 33.61 → 33.61 | 0 |
| bootstrap/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 287.1 | 7.0 | 498.8 | 2679.8 | 34.87 → 34.87 | 33.68 → 33.68 | 0 |
| challenge/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 1010.1 | 3.0 | 419.0 | 422.3 | 36.59 → 36.22 | 33.18 → 33.18 | 0 |
| challenge/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 1016.0 | 3.0 | 416.7 | 419.9 | 36.58 → 36.39 | 33.24 → 33.24 | 0 |
| session_api/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 164.7 | 10.0 | 646.5 | 4989.9 | 34.51 → 34.51 | 33.77 → 33.77 | 0 |
| session_api/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 226.0 | 9.0 | 560.5 | 3539.8 | 34.55 → 34.55 | 34.09 → 33.85 | 0 |
| session_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 249.3 | 8.0 | 815.5 | 2593.6 | 36.95 → 36.95 | 34.11 → 34.10 | 0 |
| session_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 317.6 | 6.0 | 787.0 | 1920.3 | 36.37 → 36.37 | 33.65 → 33.65 | 0 |
| session_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 622.9 | 4.0 | 561.8 | 941.7 | 36.08 → 36.08 | 33.59 → 33.53 | 0 |
| session_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 649.2 | 4.0 | 554.4 | 898.4 | 36.46 → 36.46 | 33.11 → 32.98 | 0 |
| grant_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 203.1 | 8.0 | 852.5 | 3623.0 | 34.98 → 34.98 | 33.46 → 33.42 | 0 |
| grant_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 503.4 | 5.0 | 681.7 | 1085.4 | 36.65 → 36.65 | 33.05 → 33.05 | 0 |
| grant_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 475.8 | 5.0 | 567.2 | 1344.5 | 36.40 → 36.40 | 33.55 → 33.50 | 0 |
| grant_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 655.0 | 4.0 | 544.3 | 854.5 | 35.99 → 35.99 | 33.27 → 33.13 | 0 |
| grant_renewal/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-117/renewals-8/baseline | 86.4 | 14.0 | 1250.0 | 10000.0 | 33.99 → 33.99 | 33.73 → 33.73 | 0 |
| grant_renewal/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-117/renewals-8/candidate | 173.8 | 7.0 | 0.0 | 3750.0 | 35.02 → 35.02 | 33.37 → 33.37 | 0 |
| auto_ip_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 390.1 | 6.0 | 751.5 | 1614.0 | 36.21 → 36.21 | 33.69 → 33.68 | 0 |
| auto_ip_hit/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 439.8 | 5.0 | 727.3 | 1356.1 | 36.66 → 36.66 | 33.80 → 33.80 | 0 |
| auto_ip_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/baseline | 451.6 | 5.0 | 597.8 | 1512.9 | 35.68 → 35.68 | 33.60 → 33.60 | 0 |
| auto_ip_miss/1/candidate/cache-0/profile-off/sessions-100/accounts-100/grants-100/total-grants-100/renewals-0/candidate | 579.4 | 4.0 | 540.5 | 1052.3 | 35.94 → 35.94 | 33.19 → 33.19 | 0 |

SQLite采样最大队列深度 n/a，最大队列等待 n/ams，最大活动操作 n/ams。

超时phase事件：未记录。20份原始trial文件当前不可读。

此工具的正常负载结果不等于故障恢复测试。Rust暂停/退出、SQLite写锁和上游断开恢复应单独故障注入并记录恢复截止时间。RSS为负载期间峰值和结束值，没有主动触发GC，也不代表空闲30秒后的保留量。

grant_renewal是每token仅执行一次的有限批次；短smoke中的CPU tick量化和少量尾延迟样本不适合衡量收益。phase只反映已记录的超时事件，空结果不表示SQLite等待为零。
