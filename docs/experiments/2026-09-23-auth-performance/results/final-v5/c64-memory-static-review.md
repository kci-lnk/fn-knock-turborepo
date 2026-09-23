# c64 session_hit RSS：有界静态留存审阅

结论：`5fddf896` 的请求内 TOTP 原始快照确实比 `d4f8805f` 留存更久，且可能随并发放大；它是与现有数据相容的候选原因，尚未得到堆分配/存活对象测量或六对复测确认。配置 `Arc<Value>` 本身不是每请求复制整棵树。此次不编译、不运行测试或负载、不访问远端、不改仓库，只读当前产品源码、两版差异及既有本地结果；当前相关产品文件与 `5fddf896` 一致。

## 已有证据与边界

数据来自 `/tmp/fn-knock-auth-final5-evidence-collection/concurrency5-c{1,16,64}/results.json`。三档同为 accounts/sessions1000、grants100、TOTP、mobility=false、TTL0、reader1/z、Tokio2、正常 capacity 无覆盖，clients=2（c1 实际只有一个活跃worker），每组仅一对 warm5/load15。下表只取 `session_hit`，两角色均有效、零错误。

| c | RPS base→candidate | P99 ms | Go峰RSS MiB | Rust峰RSS MiB | 两进程峰RSS之和 MiB | Rust CPU ms/成功请求 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 154.18→232.11 | 11→8 | 38.97→38.98 | 37.36→34.62 | 76.33→73.61 | 4.760→2.951 |
| 16 | 186.25→486.46 | 114→44 | 41.02→42.20 | 42.07→37.95 | 83.09→80.14 | 8.381→2.894 |
| 64 | 174.01→480.77 | 419→152 | 43.69→45.81 | 44.59→48.29 | 88.29→94.10 | 8.917→2.928 |

c64 的合计增幅为约6.58%，其中 Go 增加2.12MiB、Rust增加3.70MiB。请求上下文在Rust内，不能直接解释Go的增长。各进程峰值不保证同一时刻发生；这些是采样RSS，不是活跃堆字节或分配归因，也不能以单对推断泄漏。candidate从c16到c64吞吐近乎持平、P99上升，符合更多请求等待的可能性，但没有每时刻存活context数。

独立c16 `profile-primary/session_hit`（`/tmp/fn-knock-auth-post5-parameter-review/profile-primary/results.json`）显示 auth-reader jobs/成功请求3→7，primary约7.026→1.010；candidate auth admission累计26.273ms/成功请求。说明读工作转移到auth reader，存在后续await等待，并非SQL条数变成7或能据此计算活跃raw份数。profile没有分配/RSS对象归因，旧admission不可观测，不能从这些数值证明raw是根因。

## 有效session路径的生命周期

1. **handler scope。** [bridge.rs:434](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/bridge.rs:434) 在取得handler capacity后创建 `request_context::scope`，包住整个 `handle_bridge_message`。context保留到handler结束/取消，随后才进入response channel发送；它不会因此继续覆盖发送队列等待。新 [request_context.rs:124](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/request_context.rs:124) 包含一个 `Arc<RequestContext>` 和一个 boxed handler future；嵌套scope复用同一context。新增context及Box分配，但boxing也会缩小外层future内联部分；本次没有sizeof或allocator测量，不能推定净frame字节增量。

2. **配置共享，不是N倍大配置。** context内config是 `Arc<Value>`，[request_context.rs:23](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/request_context.rs:23) 和 [lifecycle.rs:172](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/storage/redis_store/lifecycle.rs:172) 都只克隆已发布Arc。d4 `bridge.rs:590` 已经为整个handler持有同类config快照；旧restore/sync又调用 `get_config().await`，候选的只读快捷路径改为复用Arc。因此“每个并发请求新增一整份config”不符合代码。请求期间发布新config时，旧Arc可被存活请求保留，这是快照语义；本fixture没有配置更新。

3. **TOTP只查一次，但全部原始字节跨后续await保留。** [preflight.rs:361](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/preflight.rs:361) 先权威读取session，再在`:402`解析credential权限。[preflight.rs:1004](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/preflight.rs:1004) 取credential、计算scope/header后返回，此函数在取得credential后没有其他await。fixture是单cookie、无app token、mobility关闭、无binding的普通TOTP session；`totp_id`为首个 `authperf-totp`，仅这一处credential查询，accounts的OnceCell不初始化。

4. **旧/新保留对象有实质差别。** d4 `preflight.rs:1024–1030` 为 `get_totps().await?.into_iter().find(...)`；[accounts.rs:339](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/storage/redis_store/auth/accounts.rs:339) 两版本未改：从SQL读取String、解析完整Value、规范化完整Vec。上述同步返回/选择结束后，raw、Value、未选中Vec项都会drop，留下的单credential也在scope判断函数返回时释放；allocator是否把释放页归还OS另说。没有证据表明这里已有解析结果缓存。

   候选 [request_context.rs:31](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/request_context.rs:31) 的 `Selective { raw: Option<String>, first: Option<(String, Option<T>)> }` 则保留整份原始JSON、一个ID/选中credential及固定normalize时间，直到context结束；第一次查询只缓存单项，并不删除raw。第二个不同ID才提升为全索引并释放raw（`:84–104`）；本路径没有第二ID，故不会晋升。已有测试 [tests.rs:435](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/request_context/tests.rs:435) 明确断言单ID后仍有包括第999项的raw，这是当前有意的快照设计，不是忘记drop的孤立变量。

5. **raw存活期间还有读等待。** credential判断后，`try_restore_access`调用 [restore.rs:237](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/mobility/restore.rs:237) 再读session和binding；普通fixture无binding返回false。[trusted_sync.rs:15](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/mobility/trusted_sync.rs:15) 同IP快捷判断还可能await IP location读取（fixture没有ipLocation），mobility关闭时active-IP判断立即返回true。normal_access返回后，verify在 [verify.rs:325](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/verify.rs:325) 调用trusted同步，继续 [trusted_sync.rs:237](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/mobility/trusted_sync.rs:237) 的session/binding及无binding时当前config读取。这些await期间raw仍在CURRENT中。

6. **没有发现长期后台抓住raw。** 正常成功路径的 [common_locations.rs:135](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/common_locations.rs:135) 只捕获state/IP等数据，不捕获RequestContext；不把这一点包装成普遍无泄漏证明。session本身仍按权威API重读，写路径仍重新读session/config并执行现有撤销保护；context没有保存session/grant权威对象。

## 静态量级估算

依据冻结seed.py第24–25、53行的紧凑JSON格式，使用已记录20字符createdAt，仅作字符串尺寸计算（未执行seed、SQL或测试）：1000条TOTP原始JSON为 **189,779字节，即0.181MiB**。accounts另有数组，但本TOTP session路径不加载，不能把两份相加。

若恰好C个handler都已经读完TOTP且尚未结束，raw内容本身约为：c1=0.181MiB、c16=2.896MiB、c64=11.583MiB；c16→64差额上界8.687MiB。不包括String容量、分配器、选中对象或future；也不包含尚未加载raw的请求，所以不是实测RSS预测。

实际Rust采样峰c16→64：candidate +10.344MiB，baseline +2.523MiB，两者差分+7.820MiB。量级相容只说明该线索值得核查：每个请求进入阶段时序、reader队列、分配器保留/碎片、future尺寸、SQLite缓存及不同吞吐均会影响RSS。baseline也可能在存储响应队列/尚未poll的future中持有已读取raw，并非全程raw总量为零；此处确认的是消费后继续跨await存活的差别。不能用相近数字反推64份raw确实同时存活。Go增长也需要单独解释。

## 两个不增加跨请求认证缓存的可行方向

**方案A：在可证明结束credential消费的终态分支提前释放raw。** 在桥接请求的 `resolve_preflight_normal_access` 返回已授权 `browser_session` 后，credential权限和响应header已固定；当前preflight只接受该normal_access，verify仅做现有trusted同步，不再次读取credential。可将这一分支分成显式“credential读取阶段”和“后续同步阶段”，用内部类型/接口封闭后续credential访问，再释放原始快照和不用的单项对象；config Arc、session权威重读及写入撤销检查继续保留。其他可能查询不同owner、账号或展示passkey的路径仍保留原快照/自适应索引。

这只能缩短最后阶段留存，不能消除normal_access内部所有await的raw存活，故收益可能有限。**不能在第一次命中后全局drop raw并允许后续get再读DB**，也不能把已初始化的snapshot伪装成空数组；[未查询ID及缺失项快照测试:194](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/request_context/tests.rs:194) 要求后续不同ID仍来自原输入。不能跳过trusted同步/权威重读来换内存。先证明终态分支的消费集合，再加边界断言和回归，JSON读取器与normalize行为不变。

**方案B（本轮已排除，不实现）：仍保持请求私有快照，但把大raw改为无损压缩字节。** 已有直接依赖 [Cargo.toml:31](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/Cargo.toml:31) `flate2` 可用于有限尺寸阈值以上的raw；第一次完整验证/选出credential后，缓存选中项和固定normalized_at，将原始UTF-8逐字节压缩并drop原String。重复同ID直接命中；第二个不同ID或membership查询先恢复完全相同的字节，再使用现有Value扫描与索引晋升，晋升后释放压缩输入。任何新请求仍单独读SQL并执行TTL过滤，不共享授权结果，也不引入进程级credential缓存。

必须压缩**原字节**而不是解析后重新序列化：重复字段、RawValue私有包装、损坏suffix、未知字段/错误类型和normalize默认时间语义都沿用现有读取器；整个数组仍验证，TOTP损坏仍空、accounts损坏仍报错、重复ID仍首个有效，legacy迁移仍在现有存储API完成。压缩失败回退保存原raw，解压后不重新读数据库。压缩会增加CPU和短时双缓冲；高熵数据可能不节省内存，不能预先承诺通过5%门槛，也不应未经测量移入当前已繁忙的单reader闭包。它比终态释放覆盖面广，但风险/成本更高，优先级在A之后。

本轮只保留方案A作为六对确认后的后续线索，方案B不推进。如果六对不能确认c64 RSS回退，不需要为单对噪声增加产品复杂度。若后续实施A，原有完整JSON差分、legacy迁移、未查询/缺失ID同请求快照、下一请求看到撤销、session/写路径重读语义均应保留；不能通过跨请求认证缓存或延长过期权限达到指标。

## 追加：方案A能否提前到restore await之前

实施取舍已明确：**本轮不实现、不推荐压缩方案**，其CPU和复杂度成本过高；产品继续冻结。以下仅是方案A的进一步静态边界，没有修改产品或运行验证。

**结论：不能依据“首次session权限检查成功 + 普通cookie + mobility=false”在 `try_restore_access(...).await` 之前安全释放快照。** 当前代码没有足够的terminal判据保证后续不再查询其他credential。若只追求这一更早释放点，不建议实施；保持bridge级完整normal_access返回后的保守边界更容易审计。

当前完整调用关系如下（`credential*`可读取同snapshot的另一ID）：

```text
每个AuthBridgeEnvelope独立request_context::scope
  handle_bridge_message / AuthorizeHttpRequest
    InspectSubdomainGrant → config + grant检查 → return（不加载credential）
    其他HTTP模式
      resolve_preflight_normal_access
        get_session(呈现的session cookie，权威读)
        local/manual提前放行（通常尚未加载credential）
        browser_session存在 → resolve_session_subdomain_access → credential
        app信号存在 → resolve_mobility_subdomain_access → owners → credential*
        try_restore_access.await
          fnos-token / trim-token / app恢复 / proxy-session恢复
        restored.success
          → resolve_mobility_subdomain_access → 权威owners重读 → credential*
          → scope拒绝 / LoginFirst成功时返回
        browser_session存在
          → sync_browser_session_ip_with_session.await
          → LoginFirst无条件返回browser_session（sync错误仅记录）
        StrictWhitelist或无browser_session继续
          → auto whitelist / IP owner列表 → credential*
          → app fallback owner列表 → credential*
      apply_preflight_behavior_with_grant_inspection(既定normal_access)
      resolve_auth_access_with_normal_access_and_rule_match
        browser_session成功 → sync_trusted_request
          → fresh get_session / binding / 当前config（不调用credential helper）
        → 构建verify响应（toolbar抑制字段也是同步派生）
```

具体阻止更早释放的边界：

- **restore成功后再查owner是现有行为。** [preflight.rs:431](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/preflight.rs:431) 的restore之后，`:442–445`会再次解析mobility owner权限。即使没有app信号，`restore_proxy_session`仍先读当前session和binding；mobility=false时，已有带whitelistRecordId的binding可返回true（[restore.rs:260](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/mobility/restore.rs:260)）。fixture无binding是seed条件，不能当作一般请求判据。
- **同session ID不保证未来credential ID不变。** [preflight.rs:852](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/preflight.rs:852) 会重新权威读取session，随后取其当前totpId/credentialId。存储 [mobility.rs:107](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/storage/redis_store/auth/mobility.rs:107) 的CAS update接受通用字段Map，没有把这些字段编码为不可变。不能用首次session副本替换这个权威重读，也不能假设重新读到的ID必然相同。若新ID出现，现有契约要求它从首次credential raw快照解析，而非再读最新列表。
- **多种cookie/来源仍可产生不同owner。** [preflight.rs:725](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/preflight.rs:725) 对同名cookie取最后一个非空值，因此并非遍历所有重复session cookie；但session cookie与fnos-token可以共存，trim app还可使用授权头。owner收集`:859–895`合并session、token绑定、IP来源并按session ID去重，之后`:827–829`逐owner查询credential。必须依据现有最终HeaderMap解析结果，不能用“请求只有一个Cookie header”判定单owner；不要改变重复cookie的选择语义。
- **StrictWhitelist不是终态。** 即使已有有效browser_session，`:482`只有LoginFirst提前返回；StrictWhitelist会继续`:493`的auto-IP及`:523`的app fallback。前一来源不适用/无owner并不说明后面没有credential查询。restore错误本身用`?`直接退出，后续没有fallback，但不能提前知道将走错误分支。
- **共享函数还有auth shell/bootstrap调用者。** [verify.rs:79](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/verify.rs:79) 先调用resolve_auth_access（其中调用normal_access），随后`:101`调用public_passkey_status；[passkey.rs:1146](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/passkey.rs:1146) 又查询matching_totp_ids。二者位于同一request scope。因此即使normal_access已经返回browser_session，也不能在共享函数内部对所有调用者无条件clear/seal，否则破坏后续passkey的同请求快照。WOL与stream也有credential helper使用，不能由通用get的“第一个命中”触发释放。
- **Inspect与Authorize不共享跨envelope快照。** [bridge.rs:543](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/bridge.rs:543) 每个envelope只分派一个handler；Inspect模式`:593–604`立即返回，仅使用config/grant。后续Authorize是另一个scope，会重新读其输入。无需为了Inspect保存credential，也不能把Inspect缓存搬到Authorize。PreflightAndVerify在同一个Authorize内才复用normal_access；这与旧版已有derived decision复用一致。

**能够证明的稍早位置：** 仅对明确的HTTP Authorize桥接调用链，在 [preflight.rs:469](/Users/edgeware/Local/fn-knock-turborepo/apps/server-admin-rs/src/auth/routes/preflight.rs:469) 已进入 `browser_session=Some` 且 `access_mode=LoginFirst` 的分支后、调用 `sync_browser_session_ip_with_session.await` 前，此时restore已完成；若restore成功且LoginFirst，前面的分支早已返回，所以能走到这里意味着restore未成功。这个分支的sync成功或错误均在`:482–490`返回browser_session，后续不会进入auto-IP/app fallback；本HTTP Authorize的剩余preflight/verify又没有credential消费。因此可以在这里结束**这个桥接调用专用**的credential阶段，保留所有authority读取/写入保护。

这需要显式的调用者能力或阶段参数，不能直接往通用 `resolve_preflight_normal_access` 塞一个clear。建议内部API把“仍可查询credential”与“已封闭，仅做后续同步”区分，只有桥接调用者可选择提前结束；auth shell/stream/WOL和未知后续消费者继续原有context。结束后不能通过把OnceCell置空而触发新SQL，也不能把缺少raw解释为空集合。若不愿承担这种接口分支与封闭证明，保留原方案A的bridge返回点即可。

**最终建议：不再向restore之前推进。** 最早可证明的位置仍在restore之后，只比bridge返回点提前最后一次preflight sync await；它是否值得额外接口复杂度，需要六对RSS确认及后续受控存活字节观测。当前保持冻结，不为尚未确认的单对增幅改变授权调用顺序。
