# 鉴权性能实验结果

记录 6 个trial；0 个无效。所有变化按candidate相对baseline计算。

| 路由 / 并发 / 候选 / cache TTL / 规模 | 完整对数 | 成功吞吐变化 | P99变化 | 吞吐95%区间 | P99 95%区间 | Go峰值RSS变化 | Rust峰值RSS变化 | 合计峰值RSS变化 |
| --- | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: |
| bootstrap/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 1 | +13.5% | +1.5% | 不足6对 | 不足6对 | +3.4% | +19.8% | +12.0% |
| session_hit/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 1 | -9.5% | +11.9% | 不足6对 | 不足6对 | -0.2% | +34.7% | +17.8% |
| auto_ip_hit/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0 | 1 | +60.9% | -35.7% | 不足6对 | 不足6对 | +1.9% | +39.0% | +21.6% |

存在不足六对或无效trial；这些数字只用于检查工具与发现线索，不能作为已证实的优化收益。

| 路由 / 角色 | req/s | P99 ms | Go CPU ms/千成功请求 | Rust CPU ms/千成功请求 | Go 峰值→结束 RSS MiB | Rust 峰值→结束 RSS MiB | 失败请求 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: |
| bootstrap/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 139.2 | 135.0 | 412.5 | 9274.5 | 37.94 → 37.94 | 41.16 → 40.79 | 0 |
| bootstrap/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 158.0 | 137.0 | 534.8 | 9838.3 | 39.23 → 39.23 | 49.32 → 47.84 | 0 |
| session_hit/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 188.4 | 109.0 | 556.7 | 8329.8 | 38.79 → 38.79 | 41.44 → 40.79 | 0 |
| session_hit/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 170.4 | 122.0 | 731.7 | 8780.5 | 38.70 → 38.70 | 55.80 → 54.50 | 0 |
| auto_ip_hit/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/baseline | 83.3 | 224.0 | 835.3 | 20092.8 | 37.21 → 37.21 | 42.17 → 41.39 | 0 |
| auto_ip_hit/16/candidate/cache-0/profile-off/sessions-1000/accounts-1000/grants-100/total-grants-100/renewals-0/candidate | 134.0 | 144.0 | 673.5 | 10732.1 | 37.92 → 37.92 | 58.61 → 58.61 | 0 |

SQLite采样最大队列深度 n/a，最大队列等待 n/ams，最大活动操作 n/ams。

超时phase事件：未记录。6份原始trial文件当前不可读。

此工具的正常负载结果不等于故障恢复测试。Rust暂停/退出、SQLite写锁和上游断开恢复应单独故障注入并记录恢复截止时间。RSS为负载期间峰值和结束值，没有主动触发GC，也不代表空闲30秒后的保留量。

grant_renewal是每token仅执行一次的有限批次；短smoke中的CPU tick量化和少量尾延迟样本不适合衡量收益。phase只反映已记录的超时事件，空结果不表示SQLite等待为零。
