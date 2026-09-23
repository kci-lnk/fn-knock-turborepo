# 可重复的鉴权性能实验

本工具只运行在**新的 Linux 网络命名空间**中，使用新数据库、固定实验端口、合成凭证和独立 Go/Rust 子进程。拒绝宿主网络命名空间；不替换服务、不复制生产数据库、不修改防火墙或宿主 DNS。结果目录必须不存在，失败数据保留。默认基线主仓 `d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4`，Go `92d4c0c`；每次仍须记录完整源提交、dirty patch、工具链和构建参数。

## 准备

需要 Linux root、Node 22、Python 3（仅初始化 SQLite，**不承担负载**）、`ip`、`unshare`、`getconf`。压测采用多个 Node Worker，每个 Worker 有独立事件循环，使用 keep-alive HTTP/1.1 客户端，来源固定 `198.18.0.1`，不会跟随 302。业务上游是独立 Node 子进程。客户端、上游和两服务进程分别统计 CPU。

提供基线和候选的 Linux Go/Rust 二进制，以及同一份管理/鉴权前端 dist。各组 Go 版本身份必须满足对应 Rust 的 bundle 校验。使用相同工具链、依赖锁和构建设置做代码 A/B；编译参数实验则仅改变指定参数。二进制自动记录 SHA256。

从相邻 Go 仓库编译实验控制工具（仅此独立文件，不修改产品源码）：

```sh
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build \
  -o /tmp/authperf-control \
  /ABS/fn-knock-turborepo/scripts/auth-performance-control.go
```

`variants.json` 示例：

```json
{
  "admin_static": "/tmp/auth-artifacts/ui/www",
  "auth_static": "/tmp/auth-artifacts/server-auth-view/dist",
  "control_binary": "/tmp/authperf-control",
  "baseline": {
    "name": "base",
    "go": "/tmp/auth-artifacts/base/go-reauth-proxy",
    "rust": "/tmp/auth-artifacts/base/server-admin-rs",
    "env": {},
    "metadata": {
      "main_commit": "d4f8805f",
      "go_commit": "92d4c0c",
      "rust_profile": "release"
    }
  },
  "candidates": [
    {
      "name": "candidate",
      "go": "/tmp/auth-artifacts/candidate/go-reauth-proxy",
      "rust": "/tmp/auth-artifacts/candidate/server-admin-rs",
      "env": {},
      "metadata": {
        "main_commit": "RECORD_FULL_COMMIT",
        "go_commit": "RECORD_FULL_COMMIT"
      }
    }
  ]
}
```

额外 compiler-z/s/2/3、reader-1/2/4 变体各自列入 `candidates`，由 `go` / `rust` 指向对应构建制品，`metadata` 标明编译参数及 reader 数。新版 reader pool 支持 `"env":{"FN_KNOCK_SQLITE_AUTH_READERS":"2"}`，仅接受1/2/4，默认1；reader矩阵可使用同一新版二进制和不同env。旧基线不支持该变量，不能声称设置后已改变旧版reader数量。固定端口、数据目录、凭证、运行目标 `linux` 由 harness 覆盖，不能通过 `env` 连接生产实例。默认 Tokio workers=2、bridge max-in-flight=32，可以通过已有对应环境参数做独立变体。

## 小批量运行

先验证所有路径；基线和候选甚至可以指向同一二进制，先验证工具自身噪声：

```sh
bash scripts/run-auth-performance-isolated.sh \
  --config /tmp/variants.json --out /tmp/auth-smoke-UNIQUE \
  --routes bootstrap,challenge,session_api,session_hit,session_miss,grant_hit,grant_miss,grant_renewal,auto_ip_hit,auto_ip_miss \
  --pairs 1 --warmup 1 --seconds 2 --concurrency 2 --clients 1 \
  --renewals 8 --cache-ttl 0
```

确认 smoke 通过后，先做逐路由的单并发、16、64并发，再在拐点附近补测。每个命令只取一个并发数，输出目录不同：

```sh
bash scripts/run-auth-performance-isolated.sh \
  --config /tmp/variants.json --out /tmp/auth-session-c16-UNIQUE \
  --candidates candidate --routes session_hit \
  --pairs 6 --warmup 20 --seconds 60 --concurrency 16 --clients 2 \
  --cache-ttl 0 --sessions 1000
node scripts/check-auth-performance.mjs /tmp/auth-session-c16-UNIQUE/results.json --require-six-pairs
# 对预先选定的目标路由另外检查有统计支持的收益：
node scripts/check-auth-performance.mjs /tmp/auth-session-c16-UNIQUE/results.json --require-improvement
node scripts/report-auth-performance.mjs /tmp/auth-session-c16-UNIQUE/results.json > /tmp/auth-session-report.md
```

入选方案可用 `--pairs 1 --warmup 20 --seconds 1800` 做30分钟持续负载；它是稳定性观察，不能代替六对吞吐比较。不要同时运行其他负载脚本。总运行量随 routes × candidates × pairs 线性增长，优先用 `--routes`、`--candidates` 限定单因素小批。

只观察入选candidate而不再跑baseline时，加 `--roles candidate --pairs 1 --seconds 1800`。单角色只允许至少60秒、单轮且关闭profiling；结果明确为不完整pair，不能进入A/B门槛比较，但仍有独立trial成功条件及完整资源时间序列。用report工具查看，不运行`check --require-six-pairs`。

需要检查短时饱和后的恢复时，单独运行smoke：

```sh
bash scripts/run-auth-performance-isolated.sh \
  --config /tmp/variants.json --out /tmp/auth-recovery-UNIQUE \
  --routes session_hit --pairs 1 --warmup 1 --seconds 3 \
  --concurrency 1 --clients 2 --cache-ttl 0 --recovery-probe 1
```

`--recovery-probe 1`在业务preflight之后额外施加64并发、2秒burst，复用同一路由的语义校验，完整保留状态码、失败次数、资源及health样本；503保留为失败响应，但burst不进入正式性能measurement。burst客户端请求超时固定1秒；结束并清理这些连接后，改为1并发，在10秒截止内寻找完整2秒、逐请求语义正确且零错误的连续窗口。`recovery`记录每次检查、pressure快照和恢复完成时间；完成时间包含2秒成功窗口，管理采样与清理可能稍后结束。无503或其他pressure证据时，只能说burst通过，不能宣称已经验证队列饱和恢复。

探测后继续原有warm/load；若恢复失败，整轮仍为无效，不会由后续成功覆盖。此选项限定pairs=1、warmup/seconds≤10、关闭profiling，且不支持有限token的grant_renewal。比较器按recovery开关分组，并拒绝此类trial的六对/收益门槛，避免预先施压改变缓存和内存状态后混入常规A/B。该探测不修改服务capacity，也不替代进程退出、SQLite写锁等其他故障注入测试。

规模参数独立控制：`--sessions 1..10000`（默认64）、`--accounts 1..1000`（默认1，产生同数量TOTP身份和账户）、`--grants 1..10000`（默认2，普通grant场景实际活跃总条数，包含hit token）。session按账户轮转关联。`--grants 1`的普通grant预检复用hit token，不额外增加grant；renewal场景另外增加1个专用预检token及两份`--renewals`池，`seed.total_grants`记录实际总数。大量grant索引可能使renewal批次无法在截止内完成，先用8/64/256小批定位，不能宣称默认4096必能在60秒内耗尽。

## 单独采集 SQLite executor profile

```sh
bash scripts/run-auth-performance-isolated.sh \
  --config /tmp/variants.json --out /tmp/auth-profile-UNIQUE \
  --routes session_hit --pairs 1 --warmup 20 --seconds 30 \
  --concurrency 16 --cache-ttl 0 --sessions 1000 --accounts 100 \
  --grants 100 --profile 1 --profile-idle 5
```

profile默认关闭，不能用启用profile的吞吐替代无额外观测开销的A/B。工具调用既有 `POST /api/admin/runtime-health/debug/capture` 开始、`DELETE`停止：先采一段idle背景窗口，再预热，最后单独捕获load。API内置60秒截止，因此profile load与idle均限定不超过45秒，结束状态必须为手动stopped。计量请求完成后立即停止recorder；每个capture的原始操作统计保存在trial结果。

每个trial会在独立合成库配置实验管理密码并取得管理session，通过管理界面端口27991携带cookie采集health/profile；这些准备步骤在预热前完成。不可直接请求受保护的backend管理端口27998，否则403仅说明采样未获授权。load期间每秒一次的管理health采样也会产生少量SQLite操作，idle窗口没有周期采样，因此idle只能作背景参照，不可直接相减。

`operation_profile`将`sqlite_admission` scope的calls/total_wall_ms识别为executor入场等待，将`sqlite_primary`及`sqlite_auth_read`的calls/total_wall_ms/total_cpu_ms识别为executor执行统计，同时提供每成功请求归一化和idle executor调用/秒。这是**进程范围executor job计数，不是SQL语句条数，也不是逐请求跟踪**：一个closure可能包含多条SQL，背景操作和采样请求也在窗口内；保留idle参照，不自动扣减背景值。旧baseline缺少新增admission埋点时对应归一化字段为null，不报告为零等待。丢弃的operation label数量、unfinished scope和原始label明细一并保留。

每对实验顺序交替为 AB/BA。每个 route 和样本启动独立的新进程/数据库。先启动初始化 schema，停止实验子进程，再写入合成 typed+legacy 一致种子后重新启动。种子文件要求 harness 所建目录标记；读写的只是该目录下 `state.sqlite3`。模板包含 TOTP 身份、有效 session、匹配当前 advanced-auth policy 的 grant、自动 IP 白名单及 owner session。

## 路由和真实工作检查

| route         | 数据路径                                        | 必须满足的预检及逐请求条件                    |
| ------------- | ----------------------------------------------- | --------------------------------------------- |
| bootstrap     | Go 内置鉴权 HTTP 代理→Rust                      | 200，success=true，client.ip=198.18.0.1       |
| challenge     | 同上                                            | 200，JSON challenge 和 signature 存在         |
| session_api   | 内置 session API＋有效 session cookie           | 200，authenticated=true                       |
| session_hit   | 受保护业务＋有效 session cookie                 | 200，独立业务 fixture 的特征头及正文均匹配    |
| session_miss  | 每请求不同的不存在 session cookie               | 302登录跳转，无 fixture 标记                  |
| grant_hit     | 受保护业务＋有效持久化 grant cookie             | 200，真实业务 fixture 标记                    |
| grant_miss    | 每请求不同的不存在 grant cookie，规则条件不匹配 | 302登录跳转，无 fixture 标记                  |
| grant_renewal | 每请求独立、已到续期时间且仍有效的 grant        | 200业务标记，且必须包含 grant 续期 Set-Cookie |
| auto_ip_hit   | 无cookie，自动IP白名单＋匹配IP的owner session   | 200业务标记                                   |
| auto_ip_miss  | 同样自动IP白名单，但所有owner session IP不匹配  | 302登录跳转，无 fixture 标记                  |

这里 hit/miss 指**凭证或owner查找是否命中**，不等于 Go 的授权缓存 hit/miss。`--cache-ttl 0` 明确关闭正负缓存，用来衡量实际 RPC/SQLite 路径；`--cache-ttl 1` 独立测量正常缓存开启的端到端表现。不能靠 query 变化声称避开 host-scope 缓存。

两项 TTL 均写入合成配置的 `subdomain_mode`，在 Rust 完成启动同步后使用 gRPC `GetAuthConfig → SetAuthConfig → GetAuthConfig` 设定并读回，结束时只读再次确认。关闭缓存必须提供 `control_binary`。只读生成的 config.json 不足以证实 live runtime 状态（零值有省略序列化行为）。

`grant_renewal` 是**有限 token 批次**：`--renewals` 默认为4096，每个load token只用一次，不循环伪装成持续续期。预检使用第三份独立token，warm使用独立token池；warm池用完后用普通grant复用请求继续预热到设定时长。load必须在`--seconds`上限内完成所有token，报告真实elapsed；未完成则试验无效。它的req/s不能与固定时长的grant复用负载混比。普通grant_hit长期运行中自然产生的滑动续期属于该路由真实行为。

## 指标、判读和限制

- `manifest.json`：二进制SHA、路径、variant metadata、环境参数、内核与Node版本。
- `results.json`及每trial `result.json`：真实elapsed、成功/失败请求、状态码、P50/P95/P99、稀疏1ms延迟直方图、Go/Rust/client/fixture CPU秒及每1000次成功请求CPU成本、RSS/FD/thread采样和峰值。
- 每200ms读`/proc`，每秒采样runtime-health中的storage队列、bridge数据；值不存在明确为null，不能用0代替。队列计数是进程累计值，不是每路由的独立事件分布。
- `timeout_phase_events`及保留的`runtime/logs`包含现有phase诊断。**phase只在超时事件中可读，不是所有成功请求的阶段耗时直方图。** 未触发超时不能由空数组推断SQLite等待为0。
- Worker延迟直方图先求和后计算分位数，不平均各worker的P99。计量包含客户端错误，且任何错误响应/语义不符会使样本无效。P99只精确到1ms，60秒以上归入溢出桶并报告。
- 客户端启动延迟>100ms、事件循环最大延迟>100ms、OS采样间隔>1s、时长过冲>5%（最低容忍500ms）均使trial无效并保留证据；不静默重试。失败会停止当前批，避免把错误路由测成高吞吐。
- trial失败时在停止Go/Rust前尝试最后一次有界管理health快照，保存在`failure_health`；管理接口不可用时保留其错误，不覆盖原始业务/预热错误。默认warm阶段仍不做周期health采样；最后快照只能提供失败后的状态和累计计数，不能重建失败瞬间的峰值pressure。客户端phase超时保留已完成worker计数和可取得的部分样本，明确标为partial，不能当作完整性能measurement。
- 比较器按route、并发、candidate、cache TTL、profiling及实际种子规模分组。规模包含sessions、accounts、ordinary grants、total grants及每阶段renewal tokens；不同规模不能凑成六对。缺失的旧版规模字段显示unknown，不应当作已知规模证据。
- 不加门槛参数时只检查有效、完整的smoke pair。`--require-six-pairs`要求每组至少六个有效完整pair，吞吐变化中位数≥−5%、P99变化中位数≤+10%，并要求Go+Rust峰值RSS合计的配对变化中位数≤+5%。任一pair缺失/无效RSS会使此门槛失败，不以零补齐或忽略样本。合计RSS先对每trial的Go、Rust各自峰值求和，再计算每对的candidate/baseline变化，最后取中位数；这是两个进程峰值之和，不表示它们一定同时达到峰值。JSON和Markdown同时列出Go、Rust各自及合计的RSS变化。
- `--require-improvement`隐含上述六对、回归和RSS门槛，且每组必须满足至少一项目标：吞吐变化中位数≥+10%且paired bootstrap 95%区间下界>0，或P99变化中位数≤−15%且区间上界<0。建议只对实验前选定的目标route/规模运行这一更严格的检查；其他保护路由用六对回归门槛。不能先挑出最好的分组再把区间解释成预先指定目标的证据。
- 变化按每对candidate/baseline计算，输出配对变化、中位数、六对以上的确定性paired bootstrap 95%区间。六对时区间仍较粗，P99还受1ms分桶精度限制；置信区间不能消除同机负载、热漂移或设计混杂。

本工具是闭环负载，适合路由拆分、热点归因和A/B；不声称给出固定到达率下的SLO。最终方案还应补固定请求率/突发队列实验和授权撤销、策略变化、跨host拒绝等正确性测试。若客户端CPU或Worker延迟接近瓶颈，应增加独立client worker或分离客户端CPU，不能把客户端饱和归咎Rust。`linux`运行目标有意避开fnOS宿主操作，因此不能代替完整版后台任务/分配器的单独验证。

工具单元验证（无需设备、不运行负载）：

```sh
node --test scripts/tests/auth-performance.test.mjs
node --check scripts/auth-performance.mjs
```

该目录只存工具说明；测量结果必须在实际运行后另行记录，不能把旧实验数字当作此次改动收益。
