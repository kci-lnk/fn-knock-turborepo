# Rust 最终产品第二次有界正确性审计

2026-09-23；审计基线 `d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4`，最终产品 `5fddf896eaf9b1cf3eb300c08315320498c943b8`。本轮只读源码及已有本地测试日志；未修改产品、编译、执行测试或访问远端。下列源文件经 `git diff 5fddf896 -- ...` 核对无产品差异。

**结论：本轮没有确认新的、可达的授权正确性缺陷，没有依据打断冻结制品的 c64 六对复测。** 这是指定范围的静态审计结果，不是对所有并发交错的证明。此前 IP JSON 兼容缺陷已包含于 5fdd，本轮不把已修问题或请求 raw 留存问题再列为缺陷。

## 请求内凭据及配置

| 审计项 | 旧/新路径与判断 | 证据位置 |
|---|---|---|
| 全部 JSON 的有效性 | 旧路径先将整个 raw 解析为 `Value`；新普通数组逐元素解析 `Value`，找到目标后仅停止选择，仍解析剩余元素，末尾再执行 `deserializer.end()`。因此损坏后缀、尾随字符、无效数字/Unicode、超深 JSON 不会因提前命中而被接受。 | `apps/server-admin-rs/src/auth/request_context/json_scan.rs:11,33`；`storage/redis_store/auth/accounts.rs:339` |
| serde_json 的 RawValue 特例 | 顶层非 `[` 的文本仍先 `Value::deserialize`，若其私有包装解析出数组，则遍历该数组；数组后缀也用 `Value`，没有自写跳过器。与旧路径的私有键语义一致。这个罕见顶层包装仍完整分配数组，不具备普通数组的低留存特性；这是性能边界，不是授权差异。 | `auth/request_context/json_scan.rs:15`；`auth/request_context/tests.rs:516` |
| 重复 ID、无效项、缺失项 | 首次 lookup 按数组顺序找到首个成功标准化且 ID 相等的记录；第二个不同 ID 查询时从同一 raw 构造索引，`entry(...).or_insert(...)` 仍保留首个有效记录。无效的第一个重复项不会遮蔽后续有效项；缺失结果和未查询 ID 不触发数据库重新读取。 | `auth/request_context.rs:68,86,106`；`storage/redis_store/auth/reads.rs:221,242` |
| 标准化及错误行为 | TOTP 沿用旧 `normalize_totp_credential_value`，包括 JS 字符串转换、trim、scope/host/stream 处理；account 沿用 `AuthAccount` 反序列化及 `normalize_auth_account`。account 整体坏 JSON 返回错误，TOTP 整体坏 JSON 得到空结果，非数组保持空列表。新代码没有改为严格结构体流式解析，因此对象重复字段仍保留 `Value` 的末字段语义。 | `storage/redis_store/auth/compat.rs:57,68`；`storage/redis_store/auth/helpers.rs:62,69`；`auth/request_context.rs:95,112` |
| 默认时间 | 新快照只生成一次 `normalized_at`，缺失/空 createdAt 以该值标准化，account 缺失 updatedAt 仍回退到 createdAt；已有时间字段保持原样。与旧路径逐次 `now_iso()` 的时刻可能不同，属于明确的请求内一致性选择；授权 scope、host 和有效记录选择没有依赖这些默认时间。不能宣称旧/新生成时间逐字节相同。 | `auth/request_context.rs:61`；`storage/redis_store/auth/reads.rs:229,250`；`storage/redis_store/auth/helpers.rs:74` |
| passkey membership | 所需 TOTP ID 投影使用与旧 TOTP 有效性相同的 id/secret 条件；查找命中后仍验证完整 JSON。请求集合为空时不解析无用 TOTP 内容，但仍执行 raw 加载及历史迁移；旧 TOTP 坏 JSON 本来也不报错。 | `auth/request_context.rs:189,212`；`storage/redis_store/auth/reads.rs:209`；`auth/passkey.rs:1137` |
| legacy TOTP 迁移 | 新读先检查现代键及 legacy secret，现代键存在时保持优先（包括空白或坏 JSON）；仅现代键不存在且 legacy secret 非空时调用旧 `get_totps()` 迁移，保留 passkey 关联写入，不另写一套迁移语义。 | `storage/redis_store/auth/reads.rs:186`；`storage/redis_store/auth/accounts.rs:339` |
| 配置更新与请求隔离 | 每个最外层 scope 获取当前 publisher 的 `Arc<Value>`；内层 scope 复用同一个 context。accounts、TOTP 各自第一次成功加载时才固定 raw。后续请求创建新 context，没有全局认证结果或 TTL 缓存。正常配置写 API 继续发布新配置，写/恢复路径的原有 `get_config` 不受此缓存替代。 | `auth/request_context.rs:124,139,145,166`；`auth/routes/bridge.rs:440`；`storage/redis_store/core/config_store.rs:235`；`storage/redis_store/core/value_ops.rs:329` |

这里没有采用“命中就停止验证 JSON”、未修复的 custom Discard、`HashMap::insert` 覆盖首个 ID、或命中失败后重新读取数据库等容易造成语义偏移的写法。

## Grant JOIN、修复与 CAS

1. **LEFT JOIN 保留旧列表的候选集合和错误标记。** 原 N+1 先遍历所有 zset member 再逐个读取 live grant；新查询从同一 `kv_zset` 出发，以两次 LEFT JOIN 取 raw/TTL。非法 member 即使无关联 grant 仍被观察并置 invalid；合法但悬空、非 string、无 TTL、过期或坏 JSON 的 grant 仍跳过；仅 live grant 的无效 score 置 invalid。排序和 digest 规范化保持不变。见 `storage/typed_subdomain_grant.rs:413`，旧实现保留为差分 oracle：`storage/typed_subdomain_grant/tests.rs:48`。每条 grant 的 TTL 判断仍在 SQLite executor 内进行，没有在排队前固定时间。

2. **没有把同步全 host 检查移到后台。** 健康路径在只读事务内比较权威与 shadow 并返回权威 raw；不一致路径取得 Immediate 写事务后重新 `compare_key_tx`、按当前 repair keys 修复，再 `live_auth_string_tx`。不会返回在 writer 队列等待前保存的旧 raw；删除、替换、host 改变或过期均在写事务中重新观察。见 `storage/typed_subdomain_grant.rs:189,290,328`。本次把原来的 shadow 校验与后续 grant read 合并为同一读取事务；它是该次读取的一致性边界，不保证事务结束后状态不再变化。

3. **续期不是无条件 upsert。** `renew_expiring_string_with_zset_limit` 传入先前读取的 exact raw；执行 EVAL 事务时 `string_get_tx` 先 purge 当前已过期 key，并检查 string kind，再比较 raw；不相等返回 false。只有比较成功才写新值和 index。见 `storage/redis_store/core/value_ops.rs:283`、`storage/redis_compat/eval.rs:80`、`storage/redis_compat/primitives.rs:206`。grant route 对 false 再次 `authorize_existing(..., false)`，既不复建撤销值，也不会把并发其他续期误当成功且覆盖它。见 `auth/routes/subdomain_grant.rs:504,521`。

4. **inspection 复用限于同次 bridge/preflight。** `Option<bool>` 的 false 表示已成功观察“不存在”，None 表示没有可复用结果或上次读取出错；错误不会直接被当作授权。verify 的 grant 路径仍经 `authorize_existing` 再读权威，未复用旧 raw/授权对象。见 `auth/routes/bridge.rs:619,661,697`、`auth/routes/preflight.rs:162`、`auth/routes/subdomain_grant.rs:315,444`。

## 已有运行证据及没有承诺的边界

本次只核对已保存日志，**没有重新运行**。`docs/experiments/2026-09-23-auth-performance/results/rust-final4-tests.log:2134` 的最终进程汇总为 2102 passed、0 failed、10 ignored；对应 Clippy 日志 `rust-final4-clippy.log:2` 完成。该日志中隔离子进程的预期失败输出不应替代最终父进程汇总。

针对本次范围，日志明确包含以下通过项：

- `scope_reuses_first_valid_credential_and_next_request_observes_revocation`、`config_is_consistent_within_scope_and_refreshed_between_requests`、`unqueried_ids_and_missing_results_use_the_original_snapshot`（`auth/request_context/tests.rs:20,82,194`）。
- `selective_credential_reads_match_legacy_normalization_and_json_validation`、`skipped_json_suffix_has_the_same_validation_as_serde_value`、`raw_value_wrappers_match_whole_value_parsing_before_and_after_first_match`（同文件 `:305,486,516`）。
- `multiple_owner_lookup_scans_at_most_twice_and_releases_raw_snapshot`、`membership_projection_migrates_legacy_totp_without_retaining_credentials`（同文件 `:454,253`）。
- `batched_active_entries_preserve_legacy_validation_and_orphans`、`batched_active_entries_reject_invalid_scores_only_for_live_grants`（`storage/typed_subdomain_grant/tests.rs:90,130`）。
- `subdomain_grant_auth_repair_rereads_revoked_authority_after_writer_wait`、`subdomain_grant_renewal_does_not_restore_a_revoked_or_replaced_value`、`subdomain_grant_renewal_rechecks_authority_after_writer_admission`（`storage/redis_store/tests/aggregates.rs:4,62,140`；最后一项包含 revoke/replace/expire 三种排队后变更）。
- `reused_preflight_inspection_does_not_replace_final_revocation_check`（`auth/routes/subdomain_grant.rs:710`）。

仍需保留以下精确范围限制，均不足以作为新缺陷或本轮改产品的理由：

- 配置是在 scope 建立时取得**已发布**快照，accounts/TOTP 在各自首次读取时取得快照；不是同一数据库事务、也不是同一时刻。请求中途删除权限/凭据不保证当前请求追溯撤销，新请求在其首次读取时观察当时已提交凭据。直接外部 SQL 改配置而未经过 publisher 不在“下一请求立即看见已发布更新”的承诺内。
- Session 没有加入请求缓存，IP owner 最终确认保留 `get_session_value → Value → LoginSession` 权威重读；但预先计算的 `normal_access` 跨 preflight/verify 复用在 d4 已存在，不能声称整个 RPC 与注销线性一致。单个 RPC 中交错权限删除和全部来源 fallback 的组合测试仍未独立覆盖，现有测试分别覆盖其组成边界。
- `PREFLIGHT_ONLY` 不包含最终 verify 的再读；inspection 复用的布尔值可能在之后失效。已有网关授权缓存窗口也仍存在。没有证据支持将这些语义写成“每次返回前都重验 grant”。
- exact-raw CAS 不是代际 CAS：删除后以完全相同 bytes 重新创建的 ABA 不能被区别；key 的 TTL 由事务执行时检查，但不是对整个 HTTP 响应期间的永久有效性保证。既有文档已明确此边界，本轮没有引入新长期认证缓存。
- 单 ID 快照仍保留 raw 至 scope 结束，多个 ID 晋升后改留规范化索引；可能增加并发 RSS，但不改变认证输入来源。本轮 c64 实测结果应独立决定性能结论，不能以静态容量估算证明原因，也不能为省内存直接 clear 后重新读取凭据。

本轮不提出产品补丁或新增全面测试；若未来实现提前释放 snapshot、改 JSON visitor、改变配置发布路径、扩大 inspection 复用范围或引入版本 CAS，应针对对应新边界增加最小回归，而不是沿用本次结论。
