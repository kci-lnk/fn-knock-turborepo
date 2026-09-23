# 鉴权优化的正确性边界与测试索引

本页记录语义覆盖及已完成的验证检查点。性能收益、RSS 门槛和持续负载结果另见 [README](README.md) 与 `results/`；功能测试通过不能替代性能门槛。

## 验证状态

| 检查点 | 已完成验证 | 证据与限制 |
| --- | --- | --- |
| Rust `60c95286b78d784c288c5060e8a52339e65eabf6` | `cargo clippy --all-targets -- -D warnings` 通过；`cargo test --lib -- --test-threads=2`：2098 passed、0 failed、10 ignored | [Clippy 日志](results/rust-final2-clippy.log)、[完整测试日志](results/rust-final2-tests.log)。这是已完成完整回归的低留存候选检查点，**不是**性能 A/B 的原始基线 `d4f8805f`。 |
| Rust `32d1aa53`：JSON 兼容修复 | `cargo test --lib auth::request_context -- --test-threads=2`：11 passed、0 failed | 本机局部日志 `/tmp/fn-knock-auth-json-value-compat-tests.log`；覆盖 RawValue 包装及损坏后缀，且已由下方 `c65503d6` 的最终完整检查覆盖。 |
| Rust `fde0e280`：IP 候选投影 | 局部测试：mobility 28 passed；storage auth_reads 5 passed | 本机日志 `/tmp/fn-knock-ip-candidate-mobility-tests-20260923.log`、`/tmp/fn-knock-ip-candidate-auth-reads-tests-20260923.log`；含 1000 个非匹配 session、重复匹配、bootstrap 0/1/2 owner 和投影后撤销。 |
| Rust `c65503d6cdf49336cdad922c1f7cb2046cf318a8`：CAS 排队重验及上述修复 | Clippy `--all-targets -- -D warnings` 通过；`cargo test --lib -- --test-threads=2`：**2101 passed、0 failed、10 ignored**，117.82 秒 | [完整测试日志](results/rust-final3-tests.log)、[Clippy 日志](results/rust-final3-clippy.log)，两个进程均退出 0；包含 RawValue、候选投影/唯一 owner 及 CAS 排队新增测试。 |

完整测试日志中，`runtime_health::tests::panic_hook_captures_and_redacts_unhandled_panic` 故意启动一个 panic 子进程，该子进程打印 `FAILED`，随后父测试为 `ok`；以上结论以最末尾整个测试进程的汇总及退出码为准。

## 已通过完整检查点的 Rust 测试索引

以下名称均可在链接文件内精确检索；除单独注明外，已包含在 `60c95286` 的完整库测试中。

| 边界 | 测试名与文件 | 实际保证 |
| --- | --- | --- |
| 配置发布与请求快照 | [request_context/tests.rs][context-tests]：`config_is_consistent_within_scope_and_refreshed_between_requests`；`public_captcha_reads_keep_legacy_precedence_and_observe_updates` | 同请求配置固定，下个请求读取已发布更新；CAPTCHA 兼容来源优先级保持。 |
| 凭据撤销与重复 ID | [request_context/tests.rs][context-tests]：`scope_reuses_first_valid_credential_and_next_request_observes_revocation`；`unqueried_ids_and_missing_results_use_the_original_snapshot` | 重复 ID 取首个有效凭据；未查询 ID 和缺失结果仍来自同一原始快照；下个请求看到已提交删除。 |
| 低留存与多 owner 复杂度 | [request_context/tests.rs][context-tests]：`one_id_and_membership_reads_do_not_retain_the_full_normalized_collection`；`multiple_owner_lookup_scans_at_most_twice_and_releases_raw_snapshot`；`credential_id_reads_match_legacy_lists_at_multiple_sizes` | 单 ID 与 passkey membership 不保留整张标准化列表；第二个不同 ID 晋升索引并释放 raw；1000 个 ID 的标准化调用为 1001 次；1/100/1000 凭据与旧列表读取差分一致。 |
| JSON、标准化与迁移 | [request_context/tests.rs][context-tests]：`selective_credential_reads_match_legacy_normalization_and_json_validation`；`skipped_json_suffix_has_the_same_validation_as_serde_value`；`membership_projection_migrates_legacy_totp_without_retaining_credentials` | TOTP 损坏为空、accounts 损坏报错；验证整个后缀、未知字段、深度限制、数值范围、重复 ID、缺失字段及默认时间；保留 legacy TOTP/passkey 关联迁移。RawValue 的额外修复及测试状态见下一节。 |
| 并发注销与旧 session | [auth/mobility/tests.rs][mobility-tests]：`concurrent_logout_is_a_barrier_for_active_whitelist_and_bindings`；`revoked_borrowed_session_cannot_recreate_active_ip_or_whitelist`；`logout_collects_whitelist_left_pending_before_publication` | 注销与 8 个 IP 更新并发后不残留 session、active-IP、白名单及 binding；持有旧 session 的写路径不能复建授权；处理中白名单也被注销收集。 |
| 当前权限与跨域拒绝 | [routes/tests.rs][route-tests]：`automatic_ip_grant_respects_owner_subdomain_scope`；[routes/handlers.rs][handlers]：`password_login_revalidation_uses_current_permissions_and_checks_credential_identity`；[routes/bridge.rs][bridge]：`canonical_stream_ip_respects_grant_mode_credentials_ipv6_and_session_union` | 自动 IP 授权遵守允许域/拒绝域；密码写入前重验权限与身份；stream 检查协议端口权限、缺失凭据、IPv6 和同 IP 多 session 权限并集。 |
| 策略更新使 grant 失效 | [routes/bridge.rs][bridge]：`returned_cookie_probe_bypasses_preflight_and_becomes_one_persistent_grant`；[routes/subdomain_grant.rs][grant-route]：`match_validation_is_host_policy_and_group_scoped` | 拒绝跨 host、伪造 token、过期 grant；禁用策略、轮换 policy version、删除 groups 后的新检查失效。 |
| TTL 与时间窗 | [storage/auth_reads.rs][read-tests]：`authorization_string_and_hash_ttls_are_checked_after_reader_admission`；[routes/bridge.rs][bridge]：`stream_scope_keeps_canonical_ip_and_expires_only_drift_ips_with_mobility`；[storage/aggregates.rs][aggregate-tests]：`subdomain_grant_expiry_restore_and_clear_keep_aggregate_exact` | binding/hash 在 reader 排队期间到期后返回空；只读过滤不清理 KV；漂移 IP 时间窗、session 绝对过期和 grant 到期/恢复/清空语义保持。 |
| KV 权威与 shadow 修复 | [storage/mobility.rs][storage-mobility-tests]：`auth_session_reads_repair_shadow_but_never_authorize_typed_only_state`；[storage/aggregates.rs][aggregate-tests]：`subdomain_grant_auth_repair_rereads_revoked_authority_after_writer_wait`；`subdomain_grant_repairs_whole_aggregate_and_typed_failure_rolls_back` | typed-only session 不授权；同步修复损坏 shadow；grant 修复等待 writer 后重读撤销状态；typed 写失败回滚。 |
| 写锁和 primary 阻塞期间读取 | [auth/mobility/tests.rs][mobility-tests]：`stable_session_read_paths_work_while_sqlite_writer_is_locked`；[storage/auth_reads.rs][read-tests]：`authorization_metadata_does_not_queue_behind_primary`；[storage/aggregates.rs][aggregate-tests]：`matched_subdomain_grant_read_bypasses_the_primary_executor` | SQLite `BEGIN IMMEDIATE` 下稳定 session 读取可完成；primary executor 阻塞不拖住元数据和 shadow 已匹配的 grant 读取。 |
| 取消、reader 饱和及恢复 | [auth_read_pool.rs][reader-pool]：`pooled_readers_retain_admission_and_checkpoint_guards_after_cancellation`；[redis_compat/tests/migrations.rs][migration-tests]：`canceled_auth_reader_waiter_is_never_submitted_to_sqlite`；`canceled_exclusive_call_retains_checkpoint_gate_until_sqlite_finishes` | 2/4 reader 全占用后，取消调用者不提前释放执行槽/checkpoint gate；取消 waiter 不执行 SQL；实际 closure 完成后读取和 exclusive 调用恢复。 |
| 取消与诊断统计 | [redis_compat/tests/operations.rs][operation-tests]：`sqlite_operation_capture_excludes_cancelled_admission_waiters`；`sqlite_operation_capture_survives_http_cancellation_until_closure_finishes`；`sqlite_operation_capture_keeps_old_execution_out_of_new_generation` | admission 与 SQL 执行分开计数；调用者取消不冒充执行取消；旧 generation 不污染新捕获。 |
| Bridge 截止时间和恢复 | [routes/bridge.rs][bridge]：`bridge_capacity_waits_for_an_in_flight_request`；`bridge_capacity_wait_respects_its_budget`；`bridge_capacity_waiters_share_one_absolute_deadline`；`bridge_response_send_respects_its_headroom_budget` | 满载等待、统一绝对截止时间、响应队列超时及排空后的恢复。 |
| Grant CAS 与 inspection 复用 | [storage/aggregates.rs][aggregate-tests]：`subdomain_grant_renewal_does_not_restore_a_revoked_or_replaced_value`；[routes/subdomain_grant.rs][grant-route]：`reused_preflight_inspection_does_not_replace_final_revocation_check` | 旧 raw 不能覆盖替换值或复建删除值；preflight 复用旧检查后，最终 authorize 仍拒绝已撤销 grant。 |
| IP 规范化、排序、候选再确认 | [auth/mobility/tests.rs][mobility-tests]：`batched_ip_owners_preserve_http_stream_normalization_and_revocation`；`batched_ip_owners_disabled_mobility_uses_canonical_ip_and_expiry`；[storage/auth_reads.rs][read-tests]：`batched_session_ip_snapshot_preserves_recent_detail_semantics_and_order`；`batched_ip_candidates_match_legacy_reads_at_multiple_session_counts` | HTTP 文本比较与 stream 地址比较差异保持；JSON detail IP 不以 zset member 替代；发现后的撤销被最终重读拒绝；最新 session 优先，1/100/1000 session 与旧实现逐项一致。 |
| Grant JOIN 等价 | [typed_subdomain_grant/tests.rs][grant-batch-tests]：`batched_active_entries_preserve_legacy_validation_and_orphans`；`batched_active_entries_reject_invalid_scores_only_for_live_grants` | LEFT JOIN 不隐藏 malformed/orphan；失效记录及非法 score 保持原处理顺序。 |

## 后续修复的新增覆盖

- **局部及最终全库已通过**：`32d1aa53` 在 [request_context/tests.rs][context-tests] 新增 `raw_value_wrappers_match_whole_value_parsing_before_and_after_first_match`，并扩充既有标准化差分测试。依赖启用 `serde_json/raw_value` 时，特殊首键可把顶层对象展开成数组，或让表面合法的后缀解析失败。已删除自写 discard；普通数组逐个 `Value` 解析后释放，少见顶层包装走完整 `Value`，严格遵从旧行为。
- **局部及最终全库已通过**：`c65503d6` 的 [storage/aggregates.rs][aggregate-tests] 测试 `subdomain_grant_renewal_rechecks_authority_after_writer_admission`：1 passed，内部对 revoke/replace/expire 三种变更验证续期排队后的事务重验。
- **局部及最终全库已通过**：`fde0e280` 的 mobility 组 28 passed、storage auth_reads 组 5 passed；[auth/mobility/tests.rs][mobility-tests] 的 `batched_ip_candidates_drop_nonmatching_snapshots_before_authority_reads` 补充 1000 个不匹配 session 不跨 await 留存、执行线程内投影、canonical/detail 重复匹配仅一个 owner、bootstrap 0/1/2 owner、同时间稳定排序及投影后撤销。

以上新增项均已在 `c65503d6` 的最终完整库测试中通过，不再沿用旧检查点结果推断新源码状态。跨 cookie/token/IP 来源的 owner 去重，以及单个完整 RPC 中交错权限删除与 preflight/verify，目前仍没有独立组合测试；现有快照、权限、候选重验及注销屏障测试分别覆盖其组成部分。

## 请求内复用的实现边界

- 配置、accounts/TOTP 只在单请求内固定快照，没有跨请求 TTL 缓存。并发凭据/权限变更可能延至下一请求可见，新请求重新读取已提交状态。
- Session 未加入 `request_context`。既有读取点继续调用权威 `get_session`，恢复/写入路径保留存活重验，避免缓存把已注销 session 变成可写授权。
- Grant 只在 bridge/preflight 间复用 inspection 的布尔结果；最终 verify 重新读取，续期使用 expected raw 的事务 CAS，失败后重新检查。整个 host 的 grant 校验和 shadow 修复仍同步进行，本轮用 JOIN 消除 N+1，没有将完整校验移到维护任务。
- IP 批量查询只负责发现候选，不能替代最终 session 权威重读；后续投影修改应继续遵守此边界。

## 为什么没有重复 1/2/4 reader 的 TTL 测试矩阵

目前不认为需要复制同一 TTL 用例到全部 reader 数量。`get_auth_live_strings` 和 `get_auth_live_hash_field` 的时间采样位于提交的 SQLite closure 内（[auth_reads.rs][auth-read-implementation]），`call_auth_read` 的单 reader 与 pool 分支传递同一个 closure（[connection.rs][connection]）。pool 只选择执行槽，不提前计算 TTL 或改写 SQL；2/4 reader 已有饱和、取消、checkpoint 及恢复测试。

因此采用“默认 reader 验证排队后 TTL + 2/4 reader 验证调度/生命周期”的组合覆盖，未声称已运行 TTL 全矩阵。若后续增加按 reader 缓存时间、提前取快照、连接私有缓存或不同 SQL 实现，再增加对应参数化测试。

## 相邻 Go 仓库的相关测试入口

Go `748c97e03f6fb9087ac0b3c90064611dc2c6cf66` 已通过 `go test ./...`、`go test -race ./pkg/proxy` 和 `go vet ./...`，证据位于相邻仓库的实验目录及本目录 results。仓库实际目录为 `Go-Reauth-Proxy`。

- [advanced_auth_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/advanced_auth_test.go)：`TestStripAdvancedAuthGrantOrdinaryCookiesAreUntouched`、`TestStripAdvancedAuthGrantMixedCaseAndMalformedCookies`、`TestAdvancedAuthPolicyVersionPartitionsAuthCache`。
- [auth_bridge_admission_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/rpcbridge/auth_bridge_admission_test.go)：`TestAuthBridgeLimitsSentRequestsAndReusesCompletedSlots`、`TestAuthBridgeAdmissionReleaseResponseCancelRace`。
- [auth_bridge_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/rpcbridge/auth_bridge_test.go)：`TestAuthBridgeRoundTripHonorsContextWhileWriterBlocked`、`TestAuthBridgeBoundedWriterQueue`、`TestAuthBridgeReconnectFailsOnlyOldPendingRequests`。

- [auth_proxy_trailer_wire_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/auth_proxy_trailer_wire_test.go)：真实 HTTP/1 与 HTTP/2 的上下游组合、已预告和未预告 Trailer，以及普通文本、SSE、二进制响应，共 24 组。读取 EOF 后普通 Digest/X-Checksum/X-Late-Checksum 保留，内部 tracing Trailer 被过滤，首个 chunk 仍在 EOF 前可见。此测试发现原有末尾 Trailer 泄漏，独立修复 `748c97e` 在代理返回后再过滤响应头，无 body wrapper 或额外响应缓冲。
- [websocket_target_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/websocket_target_test.go)：`TestPathRuleProxiesWebSocketTargets` 覆盖 ws/wss 握手、路径和消息回显。
- [response_coalescing_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/response_coalescing_test.go)：`TestServeReverseProxyWithResponseCoalescingKeepsStreamingOptOut` 检查 SSE 刷新与 Content-Length；[html_mutation_regression_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/html_mutation_regression_test.go) 的 `TestStreamingToolbarPreservesReadErrorAfterReadySegments` 保留读取错误传播。
- [upstream_failure_diagnostic_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/upstream_failure_diagnostic_test.go)：`TestUpstreamFailureDiagnosticsCoverEveryReverseProxyRoute` 覆盖 path/host/location 代理错误。
- [auth_cache_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/auth_cache_test.go)：缓存键全部维度、exact 优先于 host、并发替换/删除后已发布对象不变；[combined_auth_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/combined_auth_test.go) 覆盖 Set-Cookie 与 none scope 不缓存、host scope 跨路径复用。

[context-tests]: ../../../apps/server-admin-rs/src/auth/request_context/tests.rs
[mobility-tests]: ../../../apps/server-admin-rs/src/auth/mobility/tests.rs
[route-tests]: ../../../apps/server-admin-rs/src/auth/routes/tests.rs
[handlers]: ../../../apps/server-admin-rs/src/auth/routes/handlers.rs
[bridge]: ../../../apps/server-admin-rs/src/auth/routes/bridge.rs
[grant-route]: ../../../apps/server-admin-rs/src/auth/routes/subdomain_grant.rs
[read-tests]: ../../../apps/server-admin-rs/src/storage/redis_store/tests/auth_reads.rs
[aggregate-tests]: ../../../apps/server-admin-rs/src/storage/redis_store/tests/aggregates.rs
[storage-mobility-tests]: ../../../apps/server-admin-rs/src/storage/redis_store/tests/mobility.rs
[reader-pool]: ../../../apps/server-admin-rs/src/storage/redis_compat/auth_read_pool.rs
[migration-tests]: ../../../apps/server-admin-rs/src/storage/redis_compat/tests/migrations.rs
[operation-tests]: ../../../apps/server-admin-rs/src/storage/redis_compat/tests/operations.rs
[grant-batch-tests]: ../../../apps/server-admin-rs/src/storage/typed_subdomain_grant/tests.rs
[auth-read-implementation]: ../../../apps/server-admin-rs/src/storage/redis_compat/auth_reads.rs
[connection]: ../../../apps/server-admin-rs/src/storage/redis_compat/connection.rs
