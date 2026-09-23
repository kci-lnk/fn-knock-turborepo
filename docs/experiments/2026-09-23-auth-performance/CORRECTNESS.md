# 鉴权优化的正确性边界与测试索引

本页记录语义覆盖及已完成的验证检查点。性能收益、RSS 门槛和持续负载结果另见 [README](README.md) 与 `results/`；功能测试通过不能替代性能门槛。

## 验证状态

| 检查点 | 已完成验证 | 证据与限制 |
| --- | --- | --- |
| Rust `60c95286b78d784c288c5060e8a52339e65eabf6` | `cargo clippy --all-targets -- -D warnings` 通过；`cargo test --lib -- --test-threads=2`：2098 passed、0 failed、10 ignored | [Clippy 日志](results/rust-final2-clippy.log)、[完整测试日志](results/rust-final2-tests.log)。这是已完成完整回归的低留存候选检查点，**不是**性能 A/B 的原始基线 `d4f8805f`。 |
| Rust `32d1aa53`：JSON 兼容修复 | `cargo test --lib auth::request_context -- --test-threads=2`：11 passed、0 failed | 本机局部日志 `/tmp/fn-knock-auth-json-value-compat-tests.log`；覆盖 RawValue 包装及损坏后缀，且已由下方 `5fddf896` 的完整检查覆盖。 |
| Rust `fde0e280`：IP 候选投影 | 局部测试：mobility 28 passed；storage auth_reads 5 passed | 本机日志 `/tmp/fn-knock-ip-candidate-mobility-tests-20260923.log`、`/tmp/fn-knock-ip-candidate-auth-reads-tests-20260923.log`；含 1000 个非匹配 session、重复匹配、bootstrap 0/1/2 owner 和投影后撤销。 |
| Rust `c65503d6cdf49336cdad922c1f7cb2046cf318a8`：历史检查点，已拒绝 | 当时 Clippy 通过；完整库测试 2101 passed、0 failed、10 ignored，117.82 秒 | [完整测试日志](results/rust-final3-tests.log)、[Clippy 日志](results/rust-final3-clippy.log)。后补 IP owner JSON 回归在该产品代码上实测失败；这份旧测试结果不能作为最终候选通过依据，见 [拒绝记录](REJECTED_CANDIDATES.md)。 |
| Rust `5fddf896eaf9b1cf3eb300c08315320498c943b8`：IP owner JSON 兼容修复 | `cargo clippy --locked --all-targets -- -D warnings` 通过；`cargo test --locked --lib -- --test-threads=2`：**2102 passed、0 failed、10 ignored**，119.89 秒 | [完整测试日志](results/rust-final4-tests.log)、[Clippy 日志](results/rust-final4-clippy.log)，两个进程均退出 0；修复前新回归 [1 failed](results/ip-json-before.log)，修复后 mobility 组 [29 passed、0 failed](results/ip-json-after.log)。功能验证不代表新制品的性能长测已完成。 |
| Go `4d15fa32764e26df58b16930d0e2503a90880002`：最终 Cookie 兼容修复 | `go test ./...`、`go test -race ./pkg/proxy`、`go vet ./...` 通过；17-case Cookie 回归三态验证完成；原始 `92d4c0c` 到最终源码的六对本机 benchmark 通过检查 | [最终 Go 报告](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-final-20260923/README.md) 及其 `validation/`、`results/` 保留完整证据，不在本目录重复复制。三态为原始通过、中间 `748c97e` 预期失败、最终通过；本机 benchmark 不代表 Linux 服务长测通过。 |

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
| 密码登录期间账户变更 | [routes/handlers.rs][handlers]：`password_login_hash_wait_rejects_changed_password_account_and_mode` | 在密码哈希暂停期间分别交错密码变更、账户删除、改名和登录模式切换；恢复后拒绝登录，无 Set-Cookie、无新 session。该测试已在最终 `5fddf896` 完整日志中通过；它不是 session/grant 完整 RPC 的跨阶段线性一致性测试。 |
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

写锁测试的保证限于稳定 session 子路径及授权元数据读取，不能外推为完整 `AuthorizeHttp` 纯只读或完全不受 writer lock 影响。[完整 handler SQL 诊断](SQL-RPC-STATEMENTS.md) 中，候选 `session_hit` 的 31 条 STMT 仍有 9 条在 primary：成功 verify 调用 `sync_trusted_request` → [`refresh_proxy_session_binding`](../../../apps/server-admin-rs/src/auth/mobility/trusted_sync.rs#L237) → `get_config` → [`load_shadow`](../../../apps/server-admin-rs/src/storage/typed_config.rs#L212)。即使无 binding、mobility 关闭且数据健康，该分支仍先读取实时配置，使用 `BEGIN IMMEDIATE`；两个 legacy 配置键的过期保护及读取、typed 配置读取和事务边界合计 9 条。live getter 承担权威读取、legacy/typed 一致性检查及必要修复职责；本次健康 fixture 未进入修复，但仍排 primary 并获取写锁。未来若增加这一场景的早退快路，必须另验证配置并发更新和修复触发时机，不能仅据当前稳定读取测试删除 live getter。

## 后续修复的新增覆盖

- **局部及最终全库已通过**：`32d1aa53` 在 [request_context/tests.rs][context-tests] 新增 `raw_value_wrappers_match_whole_value_parsing_before_and_after_first_match`，并扩充既有标准化差分测试。依赖启用 `serde_json/raw_value` 时，特殊首键可把顶层对象展开成数组，或让表面合法的后缀解析失败。已删除自写 discard；普通数组逐个 `Value` 解析后释放，少见顶层包装走完整 `Value`，严格遵从旧行为。
- **局部及最终全库已通过**：`c65503d6` 的 [storage/aggregates.rs][aggregate-tests] 测试 `subdomain_grant_renewal_rechecks_authority_after_writer_admission`：1 passed，内部对 revoke/replace/expire 三种变更验证续期排队后的事务重验。
- **局部及最终全库已通过**：`fde0e280` 的 mobility 组 28 passed、storage auth_reads 组 5 passed；[auth/mobility/tests.rs][mobility-tests] 的 `batched_ip_candidates_drop_nonmatching_snapshots_before_authority_reads` 补充 1000 个不匹配 session 不跨 await 留存、执行线程内投影、canonical/detail 重复匹配仅一个 owner、bootstrap 0/1/2 owner、同时间稳定排序及投影后撤销。
- **实测复现并修复，局部及最终全库已通过**：`5fddf896` 在 [auth/mobility/tests.rs][mobility-tests] 新增 `ip_owner_confirmation_preserves_legacy_json_and_unique_owner_semantics`。旧列表经 `raw → Value → LoginSession` 解析，唯一 owner 判断前没有逐条严格 `get_session`；候选投影改用直接 struct 解析后，会剔除重复字段或 RawValue 顶层对象包装，令两个兼容 owner 缩为一个。修复前该回归实际得到 `["a-normal"]`、预期 `["a-normal", "b-compatible"]`，见 [失败日志](results/ip-json-before.log)。候选解析及最终权威确认现已恢复一致的 Value 语义；四种输入覆盖重复同值、最后字段改为匹配/不匹配 IP、RawValue 包装，并检查 HTTP/stream owner 集合、唯一 owner 拒绝和删除后再确认。修复后 [mobility 29 项全通过](results/ip-json-after.log)，亦包含在 `5fddf896` 完整库测试中。该拒绝到选择的回归已通过失败与修复后的结果实测确认。

以上新增项均已在 `5fddf896` 的完整库测试中通过，不再沿用旧检查点结果推断新源码状态。跨 cookie/token/IP 来源的 owner 去重，以及单个完整 RPC 中交错权限删除与 preflight/verify，目前仍没有独立组合测试；现有快照、权限、候选重验及注销屏障测试分别覆盖其组成部分。

## 请求内复用的实现边界

- 配置、accounts/TOTP 没有跨请求 TTL 缓存。配置在创建请求 scope 时取得已发布的快照，accounts 与 TOTP 各自在首次访问时读取并固定原始数据；三者不是同一时刻的数据库快照。并发变更可能延至下一请求可见，新请求会取得当前已发布配置，并在首次凭据读取时看到当时已提交的状态。
- Session 未加入 `request_context`。普通 cookie/session 读取继续使用权威 `get_session`；IP owner 最终确认使用 `get_session_value → Value → LoginSession`，两者均经 `load_and_repair_session_authority` 重读 KV 权威，保留 TTL 与 shadow 修复。恢复/写入路径保留存活重验，避免缓存把已注销 session 变成可写授权。这不保证注销会追溯取消所有已经开始的请求：preflight 已计算的 `normal_access` 仍可由 verify 使用，该跨阶段复用在原始基线 `d4f8805f` 中已存在，区别于本轮新增的配置及凭据请求内快照。注销屏障测试证明的是后续写入不能复建授权状态，不是整个 RPC 与注销之间的线性一致性。
- Grant 只在 bridge/preflight 间复用 inspection 的布尔结果；实际进入 verify 的 grant 路径重新读取，续期使用 expected raw 的事务 CAS，失败后重新检查。`PREFLIGHT_ONLY` 本次没有 verify，不能由相应局部测试推导其在响应前再次读取 grant；网关已有授权缓存的有效窗口也不由这些 Rust 测试消除，这是原有 gateway cache 的边界，本轮没有新增跨请求认证缓存。CAS 检查的是事务执行时 key 仍存活且 raw 相同，不提供跨删除后重建同值的代际标识。整个 host 的 grant 校验和 shadow 修复仍同步进行，本轮用 JOIN 消除 N+1，没有将完整校验移到维护任务。
- IP 批量查询只负责发现候选，不能替代最终 session 权威重读。候选与最终确认均保持旧列表的 `raw → Value → LoginSession` 解码语义，避免仅在其中一处改为严格 struct 解析而改变唯一 owner 判断；后续投影修改应继续遵守此边界。

## 为什么没有重复 1/2/4 reader 的 TTL 测试矩阵

目前不认为需要复制同一 TTL 用例到全部 reader 数量。`get_auth_live_strings` 和 `get_auth_live_hash_field` 的时间采样位于提交的 SQLite closure 内（[auth_reads.rs][auth-read-implementation]），`call_auth_read` 的单 reader 与 pool 分支传递同一个 closure（[connection.rs][connection]）。pool 只选择执行槽，不提前计算 TTL 或改写 SQL；2/4 reader 已有饱和、取消、checkpoint 及恢复测试。

因此采用“默认 reader 验证排队后 TTL + 2/4 reader 验证调度/生命周期”的组合覆盖，未声称已运行 TTL 全矩阵。若后续增加按 reader 缓存时间、提前取快照、连接私有缓存或不同 SQL 实现，再增加对应参数化测试。

## 相邻 Go 仓库的相关测试入口

最终 Go 产品源码为 `4d15fa32764e26df58b16930d0e2503a90880002`，已通过完整测试、proxy race 和 vet；17-case Cookie 兼容回归及原始 `92d4c0c` 到最终源码的六对本机 benchmark 也已完成。命令、原始日志与测量边界见 [最终 Go 报告](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-final-20260923/README.md)。仓库实际目录为 `Go-Reauth-Proxy`；此前 `748c97e0` 的验证与测量仅保留为历史阶段，不代替最终源码证据。

Cookie 快路径的必需兼容依赖是 `0978d6b03767c3e5f3ebf72fa13074c2b72afe7d` 和 `4d15fa32764e26df58b16930d0e2503a90880002`：前者恢复空片段归一化，避免大量分号触发 Go 上游 Cookie 原始片段数量限制；后者恢复 `strings.TrimSpace` 的 Unicode/非 HTTP 边界空白语义。相同的 17-case 生产 helper 测试作为 test-only overlay 验证原始 `92d4c0c` 通过、中间 `748c97e` 失败、最终 `4d15fa3` 通过。保留快路径时两项修复均须保留，回滚分组见 [ROLLBACK 的 G5](ROLLBACK.md#go-功能回滚组)。

- [advanced_auth_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/advanced_auth_test.go)：`TestStripAdvancedAuthGrantOrdinaryCookiesAreUntouched`、`TestStripAdvancedAuthGrantMixedCaseAndMalformedCookies`、`TestAdvancedAuthPolicyVersionPartitionsAuthCache`。
- [advanced_auth_cookie_compatibility_test.go](/Users/edgeware/Local/Go-Reauth-Proxy/pkg/proxy/advanced_auth_cookie_compatibility_test.go)：`TestStripAdvancedAuthGrantCookieNormalizesEmptySegmentsForUpstreamLimit`、`TestStripAdvancedAuthGrantCookieNormalizesEmptySegments`，覆盖空片段、SP/HT、Unicode 边界空白及直接构造 helper 输入的 VT/FF；普通 Cookie 快路径继续验证零分配。
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
