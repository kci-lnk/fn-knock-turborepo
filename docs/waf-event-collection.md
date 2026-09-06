# WAF 事件收集

WAF 请求检查仍在 Go 网关内实时执行。这里的长轮询只影响检查结果的收集和落库。

## 调度

- 启动立即尝试领取；WAF 停用时不访问网关。
- 空闲调用 `WaitWafEvents`，最多等待 60 秒。收到可领取事件通知后，从首次通知开始按 `drain_interval_seconds` 聚合，默认 2 秒。
- 每批最多领取 500 条，持久化后确认租约。确认返回仍有积压时，释放处理锁、让出执行权并立即处理下一批。
- 领取、落库和确认共用事件处理锁；长轮询、聚合和退避均不持有此锁，手动拉取可以并行唤醒和领取。
- 只有启用状态和聚合间隔影响调度。间隔变化根据原聚合起点重新计算到期时间；其他配置发布保留当前等待和到期时间。保留天数在实际领取锁内读取最新快照。
- 停用取消等待并休眠至再次启用；关机取消等待和退避。已开始的批处理不因配置发布而取消。

## 通知与租约

`WaitWafEvents` 只查看可领取事件，不领取、不删除。Go 在同一队列锁内检查事件并注册共享通知通道；新增事件、释放租约和租约到期广播唤醒后，等待者重新检查。等待时释放锁，客户端取消会退出服务端等待。Go 网关优雅停止前也会先取消等待 RPC，避免空闲长轮询拖延重启；普通配置和持久化操作保持原退出流程。

通知合并到共享通道，不为每个事件分配 goroutine、定时器或配置副本。队列仍使用原有容量限制（默认 1,000 条、估算载荷 16 MiB）、10 分钟事件 TTL 和 30 秒租约。没有增加磁盘事件队列；进程重启仍可能丢失尚未持久化的内存事件。

持久化失败主动释放租约，领取错误与确认错误重试。重复投递由 `trace_id` 幂等落库。单批确认返回的剩余数优先于领取时的剩余数，兼容已有的无租约立即领取响应。

## 超时与兼容

请求 `timeout_ms=0` 使用 60,000，超过 60,000 返回 `INVALID_ARGUMENT`；正常空等待返回 `available=false`。缺少 runtime 返回 `FAILED_PRECONDITION`。

Rust 复用独立的等待客户端，传输及请求超时均为 65 秒，连接超时沿用普通客户端设置。其他接口及租约操作的超时不变。

只有 `UNIMPLEMENTED` 启用旧周期轮询，每 60 秒重新探测，支持先升级任一端。网络和服务错误按 1、2、4、8、16、30 秒退避，成功后重置。正常等待超时不是失败，停用及关机造成的取消单独计数。

诊断中 `wait / waf.wait_events` 表示长等待；`task / waf.drain_events` 表示实际处理，两者分别统计，不能把等待的墙钟耗时视为处理 CPU 耗时。

## 可复现验收

Go 仓库：

```sh
go test ./...
go test -race ./cmd/server ./pkg/waf ./pkg/admin ./pkg/proxy
go build -o /tmp/wafwaitfixture ./internal/wafwaitfixture
```

主仓库：

```sh
cargo test --locked --manifest-path apps/server-admin-rs/Cargo.toml --profile runtime-test --no-run
```

使用构建输出中的 **库测试可执行文件**（`src/lib.rs` 对应的 `server_admin_rs-<hash>`，其 `--list` 包含 `waf_long_polling_ab`）运行：

```sh
python3 scripts/bench-waf-long-poll.py \
  --rust-test-bin /absolute/path/to/library-test-binary \
  --go-fixture /tmp/wafwaitfixture \
  --output-dir /tmp/waf-long-poll-ab
```

默认逐组运行，预热 3 秒、空闲各 600 秒，随后测试 3 次零散事件和 1,000 条突发事件。`--parallel` 可同时运行两组，需在报告中注明并行干扰。`--idle-seconds` 仅用于缩短本地冒烟检查，不能替代 10 分钟验收。

每组使用独立 Rust 进程、SQLite 临时库和 Go 队列，通过真实 gRPC 通信。旧组运行原周期调度，新组运行正式长轮询调度。工具输出 RPC 计数、Go goroutine/堆分配、Rust 存活任务数、诊断操作、落库确认延迟，以及每 5 秒一次的 CPU/RSS 进程采样；负载边界额外记录累计 CPU 时间和 RSS。

这是事件收集路径的对照实验，没有运行完整网关请求检查、其他后台服务或用户流量。进程 CPU/RSS 结果不可直接外推到用户设备；长等待需要一个持续 RPC 和连接相关任务，目标是减少空拉取及唤醒，不保证整进程 RSS 降低。

实测结果见 [2026-09-06 A/B 验收报告](waf-event-collection-ab-2026-09-06.md)。
