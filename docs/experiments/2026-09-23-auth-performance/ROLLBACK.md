# 鉴权性能实验的回滚边界

本页只记录可审阅的回滚单元、依赖和验证要求，未执行 revert、切换分支、替换服务或修改运行配置。历史提交并非都能在当前树上直接无冲突 revert；应按功能处理依赖，再以回滚后的完整源码验证。不要把中间实验版本当作已验收的部署目标。

原始基线为主仓 `d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4`、相邻 Go 仓 `92d4c0cb5495d57801d52893a8f0e8496a1c9182`。Rust 候选产品源码为 `5fddf896eaf9b1cf3eb300c08315320498c943b8`，已通过完整库测试及 Clippy；此前优化 v5 的 Go `4d15fa32764e26df58b16930d0e2503a90880002` 已通过完整测试、proxy race、vet、17-case Cookie 三态回归及原始→最终六对本机 benchmark，见 [最终 Go 报告](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-final-20260923/README.md)。追加审计修复后 Go 产品为 `66998225`，G6 必须纳入回滚依赖；其验证另列于下文。匹配 Linux 制品的正式 A/B 与持续负载结果另见 [RESULTS](RESULTS.md)，不由本机 benchmark 推断。此前 Rust `c65503d6`/Go `748c97e0` 组合的正式实验已停止并保留为 [拒绝候选证据](REJECTED_CANDIDATES.md)，不能继续标为最终制品。实际采用哪组制品，以构建 manifest 和二进制哈希为准；工具或文档提交不会改变已构建二进制的源码身份。

## 先恢复 reader 默认值

`FN_KNOCK_SQLITE_AUTH_READERS` 是本轮 reader pool 的启动时实验开关。默认未设置时为 **1**，此时 `auth_read_pool` 为 `None`，走原有单鉴权 reader 路径；显式 `1` 或空字符串也是该路径。`2`、`4` 才启用池，`0` 不是关闭值。实现见 [connection.rs](../../../apps/server-admin-rs/src/storage/redis_compat/connection.rs)。

恢复方式是从实际进程启动配置中移除该变量，再按原有流程重启实验实例；也可显式设为 `1`。只在当前 shell 执行 `unset` 不会改变已经运行的进程，也不会移除服务管理器、容器或 variant 配置中另设的值。下例仅展示新进程应使用的环境，本文未执行：

```sh
env -u FN_KNOCK_SQLITE_AUTH_READERS <原有启动命令及参数>
```

实验 harness 的 `variant.env` 也须删除该项；suite 若指定 reader 矩阵，应重新生成 `--readers 1` 的配置并检查生成文件和结果中的 `effective_runtime_env`。该操作只关闭多 reader 调度，不撤回请求快照、批量读取、grant JOIN 或 CAS，也不修改数据库内容。不要借此调整 bridge 容量、Go 缓存 TTL 或 Tokio workers；它们是不同实验因素，默认与覆盖规则见 [README](README.md)。

## 相邻 Go 仓：阶段和证据

以下路径位于相邻 `Go-Reauth-Proxy` 仓。表中 ns/op、B/op、allocs/op 是同一 **原始 isolated host-hit fixture** 的六样本阶段中位数，包含请求解析开销；箭头比较相邻阶段。它们是累计源码阶段观察，不能把各阶段百分比相加，也不能当作 Linux 服务收益。

| 提交 | 功能 | 相邻阶段 ns/op；B/op；allocs/op | 六样本证据 |
| --- | --- | --- | --- |
| `0cc5ef5d` | 反代回调只捕获所需字段，减少完整规则/配置捕获 | 7879.5→7597.5；13071.5→11997.5；99→96 | [base](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/base-isolated.txt)、[capture](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/capture-isolated.txt) |
| `eb08cbb8` | 惰性 protobuf、完整缓存命中路径与 miss 回调分离 | 7597.5→7311.5；11997.5→11461；96→90 | [capture](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/capture-isolated.txt)、[lazy](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/lazy-isolated.txt) |
| `894d1122` | 已发布缓存条目不可变，命中时复用条目 | 7311.5→7140；11461→11167.5；90→89 | [lazy](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/lazy-isolated.txt)、[immutable](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/immutable-isolated.txt) |
| `2a54d305` | 缓存键用二进制摘要；会话/注销身份字符串仍保留原格式 | 7140→7113.5；11167.5→10973.5；89→86 | [immutable](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/immutable-isolated.txt)、[binary](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/binary-isolated.txt) |
| `91f65ccd` | 普通 cookie 和 ASCII header 快路径 | 7113.5→6688；10973.5→10832.5；86→75 | [binary](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/binary-isolated.txt)、[candidate](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/candidate-isolated.txt)；cookie/header 专项见 [primary raw](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/raw/candidate.txt) |

阶段汇总和完整测试记录见 [原实验说明](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/README.md)、[summary-stages.json](../../../../Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/summary-stages.json)。`4725c86b` 把 combined auth 移入 `http_auth_combined.go` 以保持文件大小预算，没有独立收益主张。缓存写入场景仍须保留已知代价：352→368 B/op（+4.55%），allocs/op 不变；不能只报告命中改善。

原始 `92d4c0c`→最终 `4d15fa3` 的直接六对实验见 [最终 Go 证据](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-final-20260923/README.md)：lean AuthOff/CacheHit 的配对中位延迟变化分别为 -7.190%/-15.198%，B/op 为 -17.920%/-26.896%。这些是包含 trailer 与两项 Cookie 兼容修复的独立直接测量，不由阶段结果拼接；此前 `748c97e0` 的 [trailer 阶段实验](../../../../Go-Reauth-Proxy/docs/experiments/trace-trailer-fix-20260923/README.md) 仅作历史证据。

### Go 功能回滚组

| 回滚组 | 涉及提交/文件 | 依赖与保留要求 |
| --- | --- | --- |
| G1：反代捕获 | `0cc5ef5d`，`pkg/proxy/handler.go` | 可针对捕获方式恢复，但后续鉴权与缓存键也修改同文件，须逐段审阅。保留路由、Host/SNI、上游错误处理和升级语义。 |
| G2：鉴权缓存热路径 | `eb08cbb8`、`894d1122`、`2a54d305`，以及布局变更 `4725c86b` | 这些提交共同修改 lookup 参数、条目类型及 combined auth 调用点。撤回整组时先处理后续依赖，再按 `4725c86b → 2a54d305 → 894d1122 → eb08cbb8` 的逆序组织改动；可保留模块拆分而手动恢复逻辑。单撤某一层须同步适配调用方，不能混用新旧键类型或缓存所有权假设。 |
| G3：cookie/header 快路径 | `91f65ccd`，`advanced_auth.go`、`trace_id.go` | 保留 Cookie 快路径时必须保留 G5 的两项兼容修复。可以恢复原慢路径并保留相同语义测试。普通/保留/畸形 cookie、大小写、Unicode fallback 都须验证。共享 benchmark fixture可保留用于测量，不必随产品逻辑移除。 |
| G4：末尾 trailer 正确性修复 | 测试 `0f16a93b`；修复 `748c97e0`，`response_coalescing.go` | **独立于性能收益，性能回滚时保留。** 缺陷在原始基线已存在；仅恢复到 `92d4c0c` 会失去这项保护。修复在反代返回、coalescer 收尾后过滤下游 header/trailer，不依赖新增 body wrapper。测试和修复应一起保留或移植到回滚结果。 |
| G5：Cookie 快路径兼容修复 | `0978d6b0`、`4d15fa32`，`advanced_auth.go`、`advanced_auth_cookie_compatibility_test.go` | **G3 Cookie 快路径的必需依赖，两项均须保留。** 前者恢复空片段归一化，避免大量分号使上游 Cookie 解析整体为空；后者恢复 Unicode/非 HTTP 边界空白的 `strings.TrimSpace` 语义。若整组恢复原慢路径，也须保留或移植 17-case 回归，不能只撤兼容修复而留下有行为差异的原样透传。 |
| G6：缓存失效代际修复 | `66998225`，`auth_cache.go`、`handler.go`、`http_auth.go`、`http_auth_combined.go`、`toolbar_data.go` | **独立正确性保护，性能回滚时保留或移植。** 覆盖失效后旧 RPC 回填、旧 singleflight 复用和旧配置快照跨代发布。撤回 G2 或恢复旧键类型时，仍须保留请求快照前捕获代际、flight 键隔离和锁内发布检查，不能仅把 40 字节键恢复为未隔离的 32 字节键。80-case barrier 与三个旧快照用例必须继续通过。 |

G4 的增量开销、24 组真实 HTTP/1↔HTTP/2 wire 验证及普通 trailer 保留证据见 [trailer 修复记录](../../../../Go-Reauth-Proxy/docs/experiments/trace-trailer-fix-20260923/README.md)。不得把它计作性能优化收益，也不应为了消除少量测量开销重新引入已知泄漏。

G5 的独立复现、原始通过/中间失败/最终通过三态、最终完整测试/race/vet 与六对 benchmark 证据统一保存在 [最终 Go 报告](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-final-20260923/README.md) 及其 `validation/`、`results/`，本目录不重复复制日志。

G6 使 Go 当前产品身份更新为 `66998225a2d0d78e40390c179681b6a48b5e5635`；`4d15fa3` 是此前 v5 性能证据的身份，不能再称已包含全部审计修复。其 [独立证据](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-cache-invalidation-20260923/README.md) 包含 80/80 原版失败、修复后通过、完整测试/race/vet，以及 `4d→669` 增量和 `92→669` 直接六对本机比较。Rust 源码仍为 `5fddf896`，但配套制品须使用新的 `FN_KNOCK_GATEWAY_COMMIT=66998225…` 重建，不能把旧嵌入身份的 Rust 二进制直接配给新 Go。构建与新一轮 Linux 协议见 [audit6-plan](audit6-plan/README.md)。该修复不保证追溯取消原在途请求，也不是额外性能收益。

## 主仓 Rust：按功能回滚

整轮撤回的产品代码应对照 `5fddf896 → fde0e280 → 32d1aa53 → 60c95286 → 144f5e7f → ed792980 → f792ec97 → 5eccf4f5` 逆向处理依赖。后续格式变化和 `c65503d6`、`1968f4e8` 等测试也须检查，不能假定逐个 revert 无冲突。以下按功能拆分时可以保留独立改进；正确性修复和行为断言应移植到最终回滚结果，而不是为通过编译直接删除。

### R1：请求上下文、批量只读发现及低留存修复

| 提交 | 作用与依赖 |
| --- | --- |
| `5eccf4f5` | 建立请求内配置/凭据快照、只读 auth metadata API、批量 IP 候选发现，以及稳定 session 的只读快路径；包含配套测试。 |
| `f792ec97` 的请求接线部分 | bridge/preflight 接入 `request_context`、凭据和 IP owner 查询改用新 API。该提交也包含 R2，不能仅按提交名整体撤回而假设不影响 R1。 |
| `144f5e7f` | 在请求 scope 边界 box future，控制栈使用；保留 scope 时须保留这项边界。 |
| `60c95286` | 凭据由整张标准化 Vec 留存改为 raw snapshot + 首个选中 ID；第二个不同 ID 才升级完整索引；passkey 状态只取 membership。 |
| `32d1aa53` | 修复选择解析与 `serde_json::Value` 在 `raw_value` 特性下的不一致，覆盖特殊首键、顶层数组包装和损坏后缀。保留选择解析就必须保留该修复。 |
| `fde0e280` | SQLite worker 内完成 IP 候选投影，仅匹配 IDs 跨异步边界；保留排序、规范化与后续权威 session 重读。 |
| `5fddf896` | R1/IP 组的正确性依赖：候选恢复 `raw → Value → LoginSession`，最终确认用权威 `get_session_value → Value → LoginSession`。修复重复字段/RawValue 包装被剔除、把两个 owner 错缩为唯一 owner 的回归；保留 IP 批量投影和最终重读时必须同时保留两处兼容语义及回归断言。 |

主要文件为 `auth/request_context*`、`auth/passkey.rs`、`auth/routes/{handlers,verify,preflight,bridge}.rs`、`auth/mobility/{restore,trusted_sync,active_ips}.rs`、`config/runtime/store.rs`、`storage/redis_compat/auth_reads.rs` 和 `storage/redis_store/auth/reads.rs`。

撤回整个 R1 时，先检查 `5fddf896` 的 IP JSON 兼容修复，再处理 `fde0e280` 的 projection API、`32d1aa53`/`60c95286` 的选择读取 API、`144f5e7f` 的 scope 包装，再恢复 `f792ec97` 中依赖这些 API 的接线，最后处理 `5eccf4f5` 的入口与基础读取层。恢复旧 IP 列表实现时也须移植 `5fddf896` 的 owner 集合/唯一性/撤销断言；不能单独撤销它而保留会缩减兼容 owner 集合的解析。此处给的是依赖逆序，不是一串保证可直接执行的 `git revert` 命令。若保留 R2，应只调整 `f792ec97` 中与上下文有关的部分，保留独立 grant 逻辑。

不要只撤掉 RawValue 修复而留下自写 discard；也不要把恢复整张 Vec 留存或让 1000 个候选跨 await 存活当作无代价开关。早期大规模单对实验出现吞吐/RSS 退化，记录在 [accounts1000 初始诊断](results/accounts1000-c16-initial.md)；它只是定位线索，并非六对收益结论。回滚后的内存/吞吐必须重新测量。

配套验证：`auth::request_context`、`auth::mobility::tests`、`storage::redis_store::tests::auth_reads`，以及 route/bridge 的密码/TOTP、权限和 stream 用例。保留重复 ID、默认时间、完整 JSON 校验、下一请求撤销可见、1000 非匹配项、同 IP 多 owner、IPv6、时间窗和最终 session 重验断言；IP JSON 还须保留 `ip_owner_confirmation_preserves_legacy_json_and_unique_owner_semantics` 对重复字段同值/异值、RawValue 包装、唯一 owner 拒绝及删除后确认的覆盖。若 API 被移除，应改写测试入口以保留行为断言。

### R2：grant JOIN、inspection 复用与 CAS

`f792ec97` 在 `storage/typed_subdomain_grant.rs` 用 JOIN 批量校验、减少逐条读取，在 `auth/routes/{bridge,preflight,subdomain_grant}.rs` 复用准备阶段 inspection，并在 `storage/redis_store/core/value_ops.rs` 与 `redis_compat/eval.rs` 增加 expected-raw 事务 CAS。`c65503d6` 追加 writer 排队后 revoke/replace/expire 的真实重验测试。

本组有三项不同责任：JOIN 减少查询；inspection 只在同请求准备阶段复用；CAS 防止旧续期结果复建已撤销记录或覆盖替换值。**撤回 JOIN 或 inspection 优化时，优先保留 CAS 与最终 authorize 重读。** 不能把全部 `f792ec97` 作为纯性能开关删除：它还接入 R1，且移除 CAS 会撤掉正确性保护。若要更换续期实现，应保持事务内权威重验并让并发撤销测试继续通过。

相关测试位于 `storage/typed_subdomain_grant/tests.rs`、`storage/redis_store/tests/aggregates.rs`、`auth/routes/subdomain_grant.rs` 与 `bridge.rs`。必须检查 malformed/orphan、非法 score、whole-host 校验、shadow 修复等待 writer 后的撤销、policy 更新、preflight/verify 间撤销以及 writer 排队后的 CAS。测试细目及已完成日志见 [CORRECTNESS](CORRECTNESS.md)。

### R3：可选 reader pool 与 admission 指标

`ed792980` 增加 `auth_read_pool.rs`、ConnectionManager 的可选池与 `call_auth_read` 分支，并在单 reader/primary/pool 中分别记录 `sqlite_admission` 与实际执行。`1968f4e8` 更新诊断测试，使取消的排队等待不被误算成 SQL 执行；`61d2a38e` 仅调整相关格式。`scripts/check-rust-task-lifecycle.mjs` 的测试任务预算也随 `ed792980` 更新。

通常先按上文移除 reader env 即可恢复单 reader，无需源码回滚。若删除 pool 实现，应同步移除字段、模块、启动解析和分发分支，并检查 lifecycle checker 对已移除测试的预算；已有 single-reader/checkpoint/cancellation 保护不应撤掉。指标可独立保留；若连 admission 指标也撤回，需同步适配 `1968f4e8` 的期望和 profile 解释，不能把缺失指标报告为零等待或声称旧版采集了新指标。

保留 reader/pool 饱和、取消 waiter 不入队、调用者取消后执行槽/checkpoint guard 仍持有到 closure 完成、旧 generation 不污染新 capture 的测试。TTL 仍在实际 SQLite closure 执行时判断，不能回滚成排队前取时间。R1/R2 使用同一个 `call_auth_read` 接口，关闭 pool 不需要撤回它们。

## 制品与编译参数恢复

编译矩阵的 `z/s/2/3` 是同源码、同锁、同工具链、同 `LTO=fat`/`codegen-units=1` 的制品差异，不是四组 Git 功能提交。恢复仓库 release 默认优化级别时使用 `z`，或清除相应 `CARGO_PROFILE_RELEASE_*` 覆盖后按已记录 profile 重建；不要修改 profile 后仍沿用旧二进制身份/哈希。构建工具及分批方式见 [README](README.md)。

选择旧制品或重建回滚源码时，应同时记录 Rust/Go 源码 SHA、产品版本、工具链、编译参数与哈希，并使 Rust 的 `FN_KNOCK_GATEWAY_COMMIT` 和实际 Go gateway 的版本身份一致。文档 SHA 不是产品 binary SHA。源码回滚不等于数据库回滚；本页不要求删除 SQLite、清空 session/grant 或覆盖用户数据。

实验工具提交与产品回滚分开处理。保留原始结果、无效/失败样本、manifest 和 [CORRECTNESS](CORRECTNESS.md) 的证据；旧测量不能用来证明新回滚组合有效。正常性能实验应继续使用产品 bridge 默认容量，不能撤掉 harness 的 `b3c9c32f` 默认值修复后把显式 32 的饱和结果误当默认行为。

## 回滚后验收

在可审阅的回滚差异完成后，先检查跨组引用和版本身份，再运行对应功能组测试，最后执行完整检查。以下是待执行的验证命令，本页编写时未运行：

```sh
# 主仓
cargo clippy --locked --manifest-path apps/server-admin-rs/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path apps/server-admin-rs/Cargo.toml --lib -- --test-threads=2
node scripts/check-rust-task-lifecycle.mjs

# 在相邻 Go-Reauth-Proxy 仓运行
go test ./...
go test -race ./pkg/proxy
go vet ./...
```

Go 还需确认 optional protobuf presence、exact-before-host、缓存失效/并发持有、cookies、24 组真实 wire trailer、SSE/WebSocket 和上游失败。Rust 的重点为会话注销、凭据权限、grant 撤销/CAS、shadow repair、TTL 与取消，详细测试索引见 [CORRECTNESS](CORRECTNESS.md)。

功能检查通过后，用匹配的 Linux 制品重跑受影响路由、规模和并发的独立语义 smoke，再按 [README](README.md) 的同环境 AB/BA 六对、RSS 和持续负载协议验收。所有指标应绑定回滚后的准确源码与配置；关闭 reader env 的验证也要记录实际进程配置。不要在进行中的串行性能测量旁启动编译或其他负载。
