# 鉴权优化实现与验收结果

**状态：优化v5实现及273次远端试验已完成，主矩阵、缓存/续期及额外c64 session六对均通过冻结门槛；用户追加审计发现Go基线已有的缓存撤销竞态，正在独立修复和复测。** 本页只采用匹配构建参数后的 v5 制品；此前检查点及中止批次见 [排除记录](REJECTED_CANDIDATES.md)。

## 实现与边界

| 功能组 | 已实现的改动 | 语义边界 |
| --- | --- | --- |
| Rust 请求内复用 | 配置快照、按需读取凭据原始快照、首次 ID 查询及第二个 ID 后的索引；bridge/preflight 复用 grant inspection | 无跨请求 Rust 认证缓存；session 和最终 grant 授权仍权威重读。配置和两类凭据不是同一时刻的数据库快照 |
| Rust 读取减量 | 同 IP 提前返回；binding/活跃 IP 专用鉴权 reader；session/IP 候选批量读取并在 worker 内投影；grant 索引使用 LEFT JOIN | TTL 在实际执行时检查；非法成员、孤立记录、兼容 JSON、修复和唯一 owner 判断保留；行扫描及 JSON 处理仍为 O(N) |
| Rust 写入与调度 | 续期 expected-raw 事务 CAS；可选 reader 1/2/4；独立 admission 与 execution 诊断 | 默认 reader=1；单写执行器、checkpoint 和取消生命周期保留；成功 verify 后的 binding 同步仍可能通过实时配置读取等待 writer |
| Go 热路径 | 缩小反代闭包捕获；实际 RPC 时才构造 protobuf；缓存全命中独立路径；不可变缓存条目；二进制 SHA-256 键；Cookie/header 快路径 | 完整缓存维度、exact→host 优先级、保留 Cookie 过滤和 optional protobuf presence 保留；缓存写入的局部代价单独记录 |
| 兼容修复 | IP owner 保持旧 Value JSON 解码；Cookie 空分段及 Unicode 边缘空白保持旧归一化；末尾 trace trailer 过滤 | IP JSON 有修复前失败、修复后通过及 legacy Value 对照；Cookie 有原始通过、中间失败、最终通过的三态验证；trailer 为原始基线已有缺陷，性能回滚应保留该修复 |

实现提交、相邻阶段 Go A/B 和可回滚功能组见 [ROLLBACK](ROLLBACK.md)，逐项正确性证据见 [CORRECTNESS](CORRECTNESS.md)。功能组存在依赖，不能把中间检查点作为可部署回滚制品。

## 固定来源与测量口径

- 原始源码：Rust `d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4`，Go `92d4c0cb5495d57801d52893a8f0e8496a1c9182`。
- 候选源码：Rust `5fddf896eaf9b1cf3eb300c08315320498c943b8`，Go `4d15fa32764e26df58b16930d0e2503a90880002`。之后的工具、测试 overlay 和文档提交不改变产品身份。
- Linux 主矩阵：同 Rust 1.96.0、Go 1.26.7、LTO=fat、CGU=1、opt-level=z；Go 两边均 `-trimpath -s -w` 并注入对应 Version/Commit。完整命令与 SHA 见 [构建记录](results/builds/final)、[冻结计划](EXPERIMENT_PLAN.json)。
- 远端为 4 CPU Linux 主机上的隔离网络命名空间、合成数据库和独立进程；主矩阵 c16、2 个客户端 worker、1000 账户/1000 session/100 grant、TOTP、mobility 关闭、正负缓存 TTL=0。每 trial 预热20秒、测量60秒，六对交替 AB/BA。
- 正常 bridge 容量不覆盖：Go 1024，当前 Linux/Rust 64；Tokio 固定2。capacity32 仅用于独立恢复试验。未替换或重启生产服务。

验收口径在测量前冻结：目标吞吐配对中位提升至少10%且95%区间下界大于0，或P99至少下降15%且区间上界小于0；其他保护项为吞吐中位不低于−5%、P99不高于+10%、Go+Rust合计采样峰值RSS中位不高于+5%。逐进程RSS另列，合计门槛不表示每个进程都低于5%。

延迟是闭环负载的毫秒直方图，不是固定到达率 SLO。合计峰值 RSS 为两进程各自采样峰值之和，不保证两峰同时出现。比较先算每对变化，再取中位数；六对配对 bootstrap 区间和既定门槛由工具计算。客户端错误、预期响应不符或采样质量失败的试次不能进入收益结论。

## Go 独立六对基准（已完成）

同一 lean handler fixture、Apple M5、Go 1.26.7、cpu=2，原始 `92d4c0c` 直接对比最终 `4d15fa3`。六对交替执行，测量时没有并行编译。

| 场景 | ns/op 中位数：基线→候选 | 配对时间变化（95%区间） | B/op 中位数：基线→候选 | allocs/op |
| --- | ---: | ---: | ---: | ---: |
| 关闭鉴权 | 4708→4367 | −7.19%（−7.54%～−6.56%） | 6517→5350 | 66→55 |
| 完整鉴权缓存命中 | 6999→5946.5 | −15.20%（−15.53%～−14.24%） | 8336→6096 | 90→66 |

缓存命中配对 B/op 下降26.896%，通过10%的分配门槛，延迟没有退化。该 fixture 把 HTTP 解析移出计时循环，并使用模拟上游；不能称为生产 HTTP 延迟。历史高基数缓存写入局部基准为352→368 B/op（+4.55%），分配次数不变，时间改善。各项原始数据和复跑命令见 [最终 Go 报告](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-final-20260923/README.md) 及 [阶段报告](/Users/edgeware/Local/Go-Reauth-Proxy/docs/experiments/auth-hot-path-20260923/README.md)。

## 查询减量的直接证据（已完成）

下面是一千账户/session/grant、mobility关闭时，**一次预热后的进程内完整 Rust AuthorizeHttp handler** 的 SQLite 语句启动数，包含事务边界；不包括 Go、网络、独立 Inspect RPC 或冷请求/周期后台任务。18 个 handler、18 次16语句校准及36次零 SQL idle 控制均通过。

| 路径 | 总语句：基线→候选 | 只读非事务语句：基线→候选 |
| --- | ---: | ---: |
| 有效 session | 52→31 | 25→15 |
| 有效 grant | 3024→12 | 3018→8 |
| auto-IP 命中 | 15→11 | 7→5 |

完整说明及模板直方图见 [SQL-RPC-STATEMENTS](SQL-RPC-STATEMENTS.md)。其中 session 仍有9条 primary 语句，来自成功 verify 后 `refresh_proxy_session_binding` 的实时配置读取；健康状态也申请 `BEGIN IMMEDIATE`。不能声称完整 RPC 已经纯只读或不会等写锁。

[存储方法诊断](SQL-STATEMENTS.md) 另外测得：N1000 健康 grant getter 1008→6，普通 session normal-access 40→13，**开启 mobility** 的 owner 查询8019→9。这些方法边界和配置不同，不能拼接成完整 HTTP 的查询次数；SQL 数量降为常数也不代表数据扫描量为常数。

## Linux 最终端到端 A/B

正式测试前的44次预备 smoke 全部有效，151,406次测量响应符合预期。它们分别覆盖 N1/N100/N1000 和已有 PASSWORD session 的账户权限路径；不包含密码登录/hash 性能，也不能跨不同规模比较并发扩展性。见 [预备验证报告](results/final-v5/preliminary-review.md) 与已离线重放核对的 [分析包](results/final-v5/preliminary-replay.tar.gz)。

`formal-v5-main` 的108个 trial 全部有效，六个目标场景均通过收益门槛，三个保护场景通过回归门槛；远端验收和本地重算结果一致。测量期共6,832,047次符合预期的请求，预热及测量语义错误均为0。这里的成功包含应当拒绝的302，不能把请求总数称为成功授权次数。

| 场景 | 吞吐 req/s 基线→候选 | 配对吞吐变化 | P99 ms 基线→候选 | 配对P99变化 | 配对合计峰值RSS变化 |
| --- | ---: | ---: | ---: | ---: | ---: |
| bootstrap | 146.8→398.5 | +171.2% | 129.5→52 | −59.5% | −4.01% |
| session API | 87.1→307.7 | +254.3% | 216.5→71 | −67.2% | −2.33% |
| 有效 session | 188.2→493.3 | +163.9% | 112→42 | −62.4% | −4.14% |
| 有效高级授权 | 275.7→1037.6 | +276.8% | 68.5→23 | −66.2% | −1.10% |
| auto-IP 命中 | 81.8→164.0 | +100.1% | 233.5→111 | −52.2% | −5.57% |
| auto-IP 未命中 | 127.4→216.9 | +71.6% | 186→85.5 | −53.5% | −5.32% |
| challenge（保护） | 3225.0→3212.3 | −0.6% | 10→10 | 0% | −0.54% |
| 无效 session（保护） | 2550.4→2594.0 | −0.1% | 13→13 | 0% | −0.58% |
| 无效 grant（保护） | 1174.6→2677.8 | +127.0% | 23→12 | −45.7% | +0.03% |

绝对值是两端各六个 trial 的中位数；变化是六个配对变化的中位数，两者不能互相换算。六目标吞吐95%区间下界均高于0，P99区间上界均低于0；challenge和无效session的吞吐区间跨零，仅能说回归检查通过。完整区间、逐进程RSS及CPU表见 [主矩阵分析](results/final-v5/main-analysis.md)；[离线复跑包](results/final-v5/main-analysis-replay.tar.gz)已核对数值一致。原验收见 [main-acceptance.json](results/final-v5/main-acceptance.json)。

客户端最高0.432核，最大事件循环延迟34.14ms、采样间隔222.38ms，未见客户端饱和证据。目标路由的Rust每请求CPU配对中位下降59.1%–71.5%；Go并非各场景都下降：auto-IP未命中从0.445→0.649 CPU ms/请求，配对中位约+45.2%，同时Rust从12.805→5.236 ms/请求。它是进程总量观察，不是该接口内部具体函数的CPU归因；[六对核对与归因限制](results/final-v5/auto-ip-miss-cpu.md)单独保留。CPU成本与每秒占用核数分别记录，不把吞吐提高后的总CPU上升误作单位成本增加。

缓存开启及有限续期各自完成12个有效 trial，两个独立验收均通过，且没有与主矩阵混合统计。v4构建参数不匹配的结果始终排除。

| 独立实验 | 两端吞吐中位数 | 配对吞吐变化（95%区间） | 两端P99中位数 | 配对合计RSS变化 |
| --- | ---: | --- | ---: | ---: |
| 缓存开启，TTL=1 | 13252.98→13956.20 req/s | +4.86%（+3.46%～+7.03%） | 5→4ms | −5.51% |
| 有限续期，TTL=0 | 121.37→304.22 批次req/s | +153.24%（+132.97%～+171.47%） | 170→82ms | −0.67% |

缓存组共9,849,692次测量请求，零语义错误；P99配对变化−20%（区间−20%～−10%），需结合5→4ms和毫秒分桶精度理解。客户端最高0.838核、上游fixture最高0.441核，未见客户端/上游饱和证据。它是TTL1的正常缓存开启负载；现有health没有累计RPC或cache hit/miss计数，不能声称100%缓存命中或计算精确命中率。

续期每 trial 64个独立token，共768次测量请求全部消费并通过业务marker、200状态及非删除续期Set-Cookie验证器；没有额外保存独立Cookie计数或完整响应头。批次真实耗时中位数527.405→210.380ms，基线范围487.550–564.959ms，候选196.305–223.228ms。P99配对变化−51.51%（−60.18%～−48.75%）。吞吐分母是这些真实完成时间，不是持续60秒；短窗CPU/RSS也不能当作稳态容量或泄漏证明。

## 同规模并发与高并发内存确认

额外24次短测固定N1000账户/session、100 grant、TTL0、reader1/z、Tokio2，仅改变c1/c16/c64。每格一对、预热5秒/测量15秒，均通过原冻结合同和正式测量质量检查；不与六对主矩阵合并。候选的吞吐在c16附近进入平台，而c64尾延迟继续增加：

| 候选路径 | c1 / c16 / c64 吞吐 req/s | c1 / c16 / c64 P99 ms |
| --- | ---: | ---: |
| bootstrap | 236.60 / 390.56 / 393.46 | 9 / 56 / 192 |
| session | 232.11 / 486.46 / 480.77 | 8 / 44 / 152 |
| grant | 461.95 / 1032.02 / 1032.93 | 5 / 23 / 86 |
| auto-IP | 134.91 / 178.63 / 167.83 | 11 / 103 / 406 |

这是固定顺序、单对探索，不能定位具体锁竞争或推出并发默认值。客户端最高0.215核、上游最高0.106核，未见压测端饱和证据。c64 baseline auto-IP预热耗时5.802秒，嵌套`warmup.duration_ok=false`；正式15.604秒测量仍满足原门槛，响应零错误。分析工具曾额外加入非协议内的预热时长硬门槛，已纠正为诊断警告，原报告和输入保持留档，见 [并发分析](results/final-v5/concurrency-analysis.md) 与 [可复跑包](results/final-v5/concurrency-replay.tar.gz)。

c64 session的单对合计峰值RSS为88.29→94.10MiB（+6.58%），超过5%参考线，已触发 [独立六对确认](c64-session-confirm-plan/README.md)：同冻结制品、20秒预热/60秒测量、六对交替AB/BA，保持原RSS和收益门槛。追加12次已全部完成，237,882次测量请求、78,857次预热请求零错误，身份、合同及原冻结收益/RSS门槛均通过。吞吐配对中位+169.09%（95%区间+163.36%～+173.84%），P99−61.99%（−62.90%～−60.22%），合计峰值RSS+3.99%，六对范围+3.08%～+4.96%。Go单进程RSS配对中位+0.24%，Rust+8.02%；合计门槛通过不能解释为Rust没有内存代价。原单对+6.58%保留，完整12条与旧261条不混算，见 [确认报告](results/final-v5/c64-session-confirm-review.md) 和 [原始数据及复跑包](results/final-v5/c64-session-confirm-replay.tar.gz)。解压后合同、严格gate和分析均离线复跑一致。请求内原始凭证快照跨await留存是一个量级相容的线索；[静态生命周期审阅](results/final-v5/c64-memory-static-review.md)解释了为什么不能通用提前释放，以及可考虑的限定分支。它没有堆归因证据，不把线索当作根因，本轮不引入压缩或跨请求认证缓存。

## 规模、reader 与编译参数

按冻结的 [post5-plan](post5-plan/README.md) 串行执行。规模组是原始源码对比最终源码，参数组是最终 z/reader1 对比一个参数变体。均只跑一对，预热5秒、测量15秒，不能替代六对收益验收；profiling 另外采集，executor job 不等于 SQL 语句。

| grant 规模（单并发） | 吞吐 req/s 基线→候选 | P99 ms | 合计峰值RSS变化 | Rust CPU ms/请求 |
| --- | ---: | ---: | ---: | ---: |
| 1000 | 34.53→132.87 | 35→11 | +1.26% | 26.97→6.19 |
| 10000 | 2.81→8.75 | 372→131 | +0.07% | 352.79→112.20 |

两个规模均通过请求语义与质量检查。10000条组只有43/132次请求，P99不具备稳定尾部估计所需的样本量；它证明当前实现仍有明显规模成本，不能把 JOIN 后的查询条数降低解释成 O(1) 授权。此次测试上限为10000条，没有新增业务硬上限。

reader=2 对 session/grant 吞吐的单对变化为+25.9%/+46.5%，auto-IP为+8.3%，但 auto-IP 的 Rust 每请求CPU增加38.7%、RSS增加4.8%。reader=4 的 session/grant 吞吐提高35.3%/58.1%，auto-IP却下降7.9%、P99增加33.3%、RSS增加12.2%。这些权衡不支持直接增加默认并行度，reader仍为1。

| 编译级别，相对 z | bootstrap / session / grant / auto-IP 吞吐变化 | 合计峰值RSS变化范围 | 判断 |
| --- | --- | ---: | --- |
| s | +3.4% / +13.5% / +8.6% / +24.8% | +1.1%～+6.4% | 有进一步复测价值；bootstrap内存代价尚未过门槛 |
| 2 | +5.2% / +20.7% / +15.4% / +41.2% | +14.3%～+15.8% | 速度收益伴随明显内存代价，不进入默认 |
| 3 | +6.3% / +22.8% / +15.3% / +39.0% | +14.6%～+16.5% | 同样增加内存；bootstrap P99单对还增加10.9%，不进入默认 |

这不是参数的六对验收，也没有把单对结果凑成统计区间。全部44次规模/参数探索及14次独立 profile 均有效；配置、角色及四条制品身份均与对应冻结组完全匹配，profile 没有丢失记录或未结束 scope。

profile显示：grant鉴权reader任务从6降为2个/请求；session的primary任务从约7.03降为1.01个/请求，同时部分读取移入鉴权reader。reader=2/4使grant入场等待分别从约13.4降至7.9/5.5ms/请求，但auto-IP的执行成本显著增加：reader=4独立profile中，执行wall从4.61增至21.64ms/请求、CPU从4.53增至12.19ms/请求。累计scope时间不等于请求延迟；执行scope还包含JSON解析和投影，checkpoint gate及worker调度间隙不包含在内。旧版本没有admission埋点，缺失值是null。profile本身有观测成本，不替代无profile的性能比较，也不足以确定锁、分配器或CPU竞争中的具体原因。

Rust 参数制品已全部构建：z=38,536,480 B，s=42,253,936 B，2=55,183,424 B，3=56,838,720 B。源码、锁、工具链、LTO与CGU固定。构建耗时受缓存影响，不作为运行性能证据。默认仍为 reader=1、opt-level=z；单对探索不足以修改默认值。

全部规模、参数、profile数值及逐进程CPU见 [详细审阅](results/final-v5/post5-parameter-review.md)；[离线分析包](results/final-v5/post5-parameter-replay.tar.gz)解出重放后，Markdown逐字节一致，JSON仅生成时间与本地路径不同。

## 持续负载与恢复

最终候选独立完成 auto-IP 持续负载1800.071秒，290,000次测量请求全部符合预期，四项质量检查均通过，最大采样间隔212.42ms。共保存8883个进程资源样本和1792个可用Go runtime观察。

| 指标，首尾5分钟中位数 | Go | Rust |
| --- | ---: | ---: |
| RSS | 39.34→40.01MiB（+1.71%） | 37.44→37.40MiB（−0.09%） |
| FD | 38→38 | 27→27 |
| 原生线程 | 10→11；第二段5分钟起保持11 | 8→8 |

Go goroutine中位数76→75、峰值83；其health数据有缓存，重复观察并非独立样本。Rust async task数没有独立暴露，不能用原生线程数代替。该窗口没有出现明显的资源持续累积证据，但单次30分钟运行、负载中RSS及已有指标不能证明无泄漏。

独立capacity32恢复试验确实触发拒绝压力：基线/候选的c64 burst分别出现18,182/20,974次503。降为c1后，两端第一个连续两秒窗口均全部成功（179/256次），后续正常c16预热和三秒测量也零错误。报告的恢复时间约2061/2051ms包含完整成功窗口，且从burst清理后开始计时；不能用约10ms差异宣称候选恢复更快，也不能由503确定首先触发容量限制的是Go还是Rust。

health端点均可读，但缓存的overall/auth_bridge状态仍为degraded，不能称所有组件已健康。该试验仅证明一次指定容量和路径下的请求恢复，不外推持续过载或多轮恢复。完整资源分段、CPU、压力计数及时间边界见 [稳定性与恢复报告](results/final-v5/post5-stability-review.md) 和 [可离线重放包](results/final-v5/post5-stability-replay.tar.gz)。完整raw已用冻结工具重算，8883个资源样本和1792个health观察得到的summary与远端完全一致，见 [重算核对](results/final-v5/soak-recompute-check.json)。

## 功能验证与交付索引

最终产品源码已完成 Rust 库测试（2102通过、0失败、10忽略）、all-targets Clippy `-D warnings`；Go 全量测试、proxy race、vet；24组真实HTTP/1/HTTP/2 trailer组合、流式首块、WebSocket及错误处理覆盖；具体断言、日志与局限见 [CORRECTNESS](CORRECTNESS.md)。API、task lifecycle 和源码规模检查通过。

实验入口与复跑说明见 [README](README.md)，正式参数和制品哈希见 [EXPERIMENT_PLAN](EXPERIMENT_PLAN.json)，后续串行任务见 [post5-plan](post5-plan/README.md)，历史失败/中止样本见 [REJECTED_CANDIDATES](REJECTED_CANDIDATES.md)。22组261次试验的完整raw、profile/recovery、冻结工具与收集索引见 [原始证据包](results/final-v5/final-v5-evidence.tar.gz)，[包说明](results/final-v5/final-v5-evidence-README.md)给出纯离线复核命令，[manifest](results/final-v5/final-v5-evidence-archive.json)记录SHA。解压后的完整性、主矩阵门槛、soak重算已实际离线复跑，见 [复跑核对](results/final-v5/final-v5-evidence-replay-check.json)。361个白名单源文件收集后复读SHA未变；敏感及自由文本按规则去敏，原始SHA与归档SHA分别记录。额外c64 session六对确认与此包独立，不混合样本。两包共273次远端trial，18,063,635次测量请求与5,970,336次预热请求符合预期；容量恢复burst的503单独统计。此结论只绑定Rust5fdd/Go4d，后续审计修复使用独立源码与复测记录。
