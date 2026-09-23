# 鉴权性能实验结果

记录 108 个trial；0 个无效。所有变化按candidate相对baseline计算。

| 路由 / 并发 / 候选 / cache TTL / 规模 | 完整对数 | 成功吞吐变化 | P99变化 | 吞吐95%区间 | P99 95%区间 | Go峰值RSS变化 | Rust峰值RSS变化 | 合计峰值RSS变化 |
| --- | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: |
| bootstrap/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | +171.2% | -59.5% | +166.8% … +180.3% | -61.0% … -58.8% | -0.6% | -7.3% | -4.0% |
| session_api/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | +254.3% | -67.2% | +246.9% … +259.6% | -67.5% … -66.3% | -0.2% | -4.2% | -2.3% |
| session_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | +163.9% | -62.4% | +158.3% … +167.1% | -63.0% … -61.6% | -0.2% | -8.1% | -4.1% |
| grant_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | +276.8% | -66.2% | +265.0% … +278.8% | -66.7% … -64.2% | -0.2% | -1.9% | -1.1% |
| auto_ip_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | +100.1% | -52.2% | +92.2% … +103.7% | -52.8% … -51.0% | +1.2% | -12.0% | -5.6% |
| auto_ip_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | +71.6% | -53.5% | +62.3% … +74.0% | -58.6% … -51.9% | -0.8% | -9.9% | -5.3% |
| challenge/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | -0.6% | +0.0% | -3.0% … +1.2% | +0.0% … +0.0% | -1.5% | +0.2% | -0.5% |
| session_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | -0.1% | +0.0% | -2.2% … +3.8% | +0.0% … +0.0% | -0.3% | -0.7% | -0.6% |
| grant_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 6 | +127.0% | -45.7% | +122.0% … +136.8% | -48.9% … -43.2% | +0.6% | -0.8% | +0.0% |

| 路由 / 角色 | req/s | P99 ms | Go CPU ms/千成功请求 | Rust CPU ms/千成功请求 | Go 峰值→结束 RSS MiB | Rust 峰值→结束 RSS MiB | 失败请求 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: |
| bootstrap/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 146.8 | 129.5 | 490.5 | 9017.4 | 39.23 → 38.87 | 42.70 → 42.20 | 0 |
| bootstrap/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 398.5 | 52.0 | 413.6 | 3195.5 | 38.99 → 38.62 | 39.48 → 38.91 | 0 |
| session_api/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 87.1 | 216.5 | 569.7 | 16799.7 | 39.07 → 38.97 | 42.51 → 41.84 | 0 |
| session_api/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 307.7 | 71.0 | 440.5 | 5421.9 | 38.96 → 38.53 | 40.64 → 37.71 | 0 |
| session_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 188.2 | 112.0 | 593.9 | 8439.4 | 40.23 → 39.86 | 41.75 → 40.94 | 0 |
| session_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 493.3 | 42.0 | 455.4 | 2903.1 | 40.17 → 39.80 | 38.33 → 35.20 | 0 |
| grant_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 275.7 | 68.5 | 537.8 | 3564.5 | 40.25 → 39.88 | 36.05 → 36.05 | 0 |
| grant_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 1037.6 | 23.0 | 539.7 | 1017.9 | 40.19 → 39.85 | 35.40 → 35.39 | 0 |
| auto_ip_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 81.8 | 233.5 | 787.4 | 20265.3 | 39.25 → 39.14 | 42.79 → 41.88 | 0 |
| auto_ip_hit/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 164.0 | 111.0 | 477.7 | 6813.6 | 39.65 → 39.41 | 37.75 → 36.88 | 0 |
| auto_ip_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 127.4 | 186.0 | 445.0 | 12805.2 | 39.25 → 39.01 | 41.82 → 40.75 | 0 |
| auto_ip_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 216.9 | 85.5 | 649.3 | 5235.6 | 38.89 → 38.78 | 37.80 → 37.37 | 0 |
| challenge/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 3225.0 | 10.0 | 248.8 | 295.1 | 40.18 → 39.31 | 34.90 → 34.90 | 0 |
| challenge/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 3212.3 | 10.0 | 248.1 | 298.2 | 39.63 → 38.79 | 35.03 → 35.02 | 0 |
| session_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 2550.4 | 13.0 | 273.8 | 521.7 | 40.58 → 39.71 | 35.40 → 35.40 | 0 |
| session_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 2594.0 | 13.0 | 271.1 | 515.5 | 40.30 → 39.74 | 35.11 → 35.00 | 0 |
| grant_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 1174.6 | 23.0 | 349.4 | 972.1 | 39.88 → 39.53 | 35.40 → 35.40 | 0 |
| grant_miss/16/candidate5/cache-0/profile-off/recovery-off/credential-totp/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 2677.8 | 12.0 | 255.4 | 489.2 | 40.02 → 39.46 | 35.09 → 35.08 | 0 |

SQLite primary采样最大队列深度 n/a，最大队列等待 n/ams，最大活动操作 n/ams。这些health字段只统计primary executor，不代表auth reader单slot或全池；auth pool饱和时仍可能为零。

超时phase事件：未记录。108份原始trial文件当前不可读。

正常负载结果不等于故障恢复测试；可选recovery-probe仅测试请求burst后的恢复，不覆盖Rust暂停/退出、SQLite写锁或上游断开。失败trial的failure_health保留清理前最后一次管理快照，快照自身错误单独记录。RSS为负载期间峰值和结束值，没有主动触发GC，也不代表空闲30秒后的保留量。

grant_renewal是每token仅执行一次的有限批次；短smoke中的CPU tick量化和少量尾延迟样本不适合衡量收益。phase只反映已记录的超时事件，空结果不表示SQLite等待为零。
