# fnOS 证书同步结构兼容

此次修复支持两种明确的 PostgreSQL 表结构：fnOS 1.2.0604 对照环境的 37 列结构，以及用户报告的 39 列结构。后者在 `cert` 表中增加可空、无默认值的 `platform` 和 `cert_url`（均为 `character varying`），并改变 `renewal` 的列位置。用户报告结构的系统版本尚未确认，不按系统版本号猜测兼容性。

结构检查按表名、列名比较，忽略物理列顺序。缺列、未知列、扩展字段只出现一个、类型/可空性/默认值变化，以及 identity/generated 属性或触发器不符合要求，均阻止同步。错误只包含表、列、属性名称，不包含实际默认值或证书/续期凭据。实际 `varchar(n)` 长度限制也被采集并纳入计划版本；数据写入前按字符数验证长度，同时验证 NULL、整数范围和日志行 ID。不接受带额外约束的 PostgreSQL domain 类型。

新增证书会构造与已验证结构一致的完整行；扩展字段显式为 NULL。已有行的扩展值会保留。结构参与同步计划版本，但不改变托管摘要版本或历史日志格式。恢复前核对日志行的字段集合，不将缺少字段自动视为 NULL，不对跨结构的历史日志进行自动改写。数据库事务取得三张表的锁后，会再次比较完整列元数据并检查触发器，然后才执行任何删除或插入，避免预检查与写入之间的结构变化绕过检查。

## 验证方式

在仓库根目录执行常规回归：

```sh
cargo test --locked --manifest-path apps/server-admin-rs/Cargo.toml --lib fnos_certificate_sync -- --skip live
npm exec --workspace=server-admin-view -- vitest run --config vitest.config.ts component-tests/fnos-certificate-sync.test.ts
npm exec --workspace=server-admin-view -- tsx --test tests/fnos-certificate-sync-contract.test.ts
```

显式执行 PostgreSQL 15 集成测试（需要 Docker 和 `postgres:15` 镜像）：

```sh
docker pull postgres:15
cargo test --locked --manifest-path apps/server-admin-rs/Cargo.toml --lib postgres15_ -- --ignored --nocapture
```

该测试创建无外部网络、无宿主目录挂载、无端口暴露的临时容器，结束时删除。仅操作容器中的合成证书数据，复用生产行构造和事务 SQL，验证两种结构的新增、更新、纳管、删除、续期记录恢复、失败回滚、引用保护、外部改动拦截和摘要一致性。夹具包含从对照机器只读查询确认的真实字段长度；额外测试在预检查后、执行事务前改变触发器、已支持的列集合或字段长度，验证正向写入和恢复均在数据库变更前被拦截。

集成测试不启动 fnOS 网络服务，也不替代事发机器上发布后的实际验证。不要对真实 NAS 运行已有的 live 测试来代替此隔离测试。本修复不需要修改用户数据库结构或删除证书。
