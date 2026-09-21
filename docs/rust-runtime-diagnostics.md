# Rust 运行诊断：配置读取

在事件中心打开「Rust 运行诊断」，启动 60 秒采样，在采样期间执行待比较的操作，结束后刷新内存详情并导出 JSON。打开面板不会自动开始采样；新增明细只在显式采样期间记录。对比时尽量使用相同配置、操作次数和操作顺序。

## 构建与缓存信息

- `process.source_fingerprint`：构建时对本 crate 的 `src/**/*.rs`、`build.rs`、`Cargo.toml`、`Cargo.lock` 计算的源码指纹，包含未提交修改。不是二进制哈希或签名，也不覆盖外部 path 依赖、编译器及全部构建环境。
- `process.config_cache_limit_bytes`：允许缓存的原始 JSON 字节数门槛，当前为 1 MiB，覆盖实测约 503 KiB 的配置。解析树另有内存开销；超过门槛仍正常读取，只不保留解析结果。

## 操作标签

| 标签                           | 范围                                                                                     |
| ------------------------------ | ---------------------------------------------------------------------------------------- |
| `config.read`                  | 完整配置读取，包括等待、校验、必要的修复和快照发布                                       |
| `config.load_shadow`           | 配置 shadow 读取，从提交前到数据库闭包结束                                               |
| `config.primary_admission`     | 从提交前到数据库闭包开始，包含准入排队和调度                                             |
| `config.async_resume`          | 数据库闭包结束前到异步调用恢复，包含执行器收尾、结果交付和调度                           |
| `config.generation_marker`     | 获取或生成指纹并注入 generation 标记                                                     |
| `config.fingerprint.compute`   | 首次使用此不可变配置时序列化 host_mappings 并计算 SHA-256；属于 generation_marker 子阶段 |
| `config.fingerprint.reuse`     | 复用与完整配置内容绑定的指纹，不跳过数据库及一致性校验                                   |
| `config.fingerprint.fallback`  | typed 与 legacy 原始 JSON 不一致时按返回的 legacy 值重新计算，保留序列化差异的语义       |
| `config.snapshot_compare`      | 判断是否可发布快照；新版本直接通过，否则比较完整快照                                     |
| `config.snapshot_publish`      | 克隆配置并尝试发布快照，仅在需要发布时记录                                               |
| `config.transaction_begin`     | 开始 Immediate 事务                                                                      |
| `config.legacy_read`           | legacy 配置和 generation 的 KV 读取，包含过期清理                                        |
| `config.typed_query`           | typed 配置查询及提取原始字段                                                             |
| `config.json_parse`            | 缓存未命中或超限时解析 typed JSON                                                        |
| `config.transaction_commit`    | 提交 shadow 读取事务                                                                     |
| `config.legacy_json_parse`     | SQLite 闭包结束后解析 legacy 配置及 generation                                           |
| `config.compare`               | 读取已发布版本并判断 typed/legacy 候选是否一致，不包含后续快照比对及修复                 |
| `config.cache.hit`             | 持久化 JSON 与缓存内容完全相同，复用解析结果                                             |
| `config.cache.miss`            | 大小允许缓存，但内容未命中；在解析前记录                                                 |
| `config.cache.bypass_oversize` | JSON 超过门槛，在解析前记录                                                              |

缓存三种结果互斥。查询失败、记录缺失或非法版本信息会在缓存判定之前退出，不计入三种结果。命中率可按 `hit.calls / (hit.calls + miss.calls + bypass_oversize.calls)` 计算；没有记录不等于命中率为零。缓存事件的耗时只是记录事件的开销，判断解析成本应看 `config.json_parse`。

新增字段均为可选字段，旧报告仍可展示：

- `total_bytes` / `max_bytes`：缓存事件对应的原始 JSON 累计／最大字节数，不是堆占用，也不是配置内容。
- `max_wall_at_ms`：该标签最长已完成调用的完成时刻，单位为距采样开始的毫秒，可与资源样本对照；不是该调用的开始时刻，也不是 CPU 最大值的时刻。
- 单次平均耗时：`total_wall_ms / calls`。单次平均 CPU：有 CPU 统计时用 `total_cpu_ms / calls`。

`sqlite_phase` 有同线程 CPU 统计；异步父任务及 `task_phase` 只有墙钟时间。父操作包含子阶段，不能把两者相加；汇总数据库 CPU 时只统计 `sqlite_primary`、`sqlite_auth_read`、`sqlite_health`、`sqlite_analytics`，不要再累加 `sqlite_phase`。

明细仍受 128 个标签上限约束；阶段继承父操作的采样代次，旧操作不会跨入新采样。未完成操作没有完成耗时。诊断本身有额外开销，所以两次对比应使用相同诊断实现。队列历史峰值仍是执行器生命周期累计值，并非本次采样专属。

报告不记录 SQL、JSON 内容、配置键、请求信息或凭据。内存详情仍需单独刷新，其时间戳可能与资源采样不同。
