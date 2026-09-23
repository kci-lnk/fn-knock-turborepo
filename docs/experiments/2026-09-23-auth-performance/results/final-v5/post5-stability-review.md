# post5 稳定性与饱和恢复离线审阅

仅分析指定六份已完成JSON。固定source、四制品SHA、冻结config、harness/control身份及每trial有效env/TTL均通过合同核对；未访问业务端口、未启动负载，未读取soak大型raw。

## 30分钟 auto-IP soak

候选单独运行，c16/clients2、accounts/sessions1000、grants100、TOTP、mobility关闭、TTL0、Tokio2、默认reader1/z及正常capacity。实际负载1800.071秒，290000次200，失败0，RPS161.105，P50/P95/P99=100/107/112ms。warmup 3376次200、零失败。

采样8883次、health观察1792次；最大采样间隔212.424ms，客户端event-loop最大延迟27.787ms、启动最大延迟1.289ms。四项quality均true，timeout phase事件0。首尾进程PID/start_ticks一致。

| 进程 | RSS首/末5分钟中位MiB（变化） | RSS采样峰MiB | FD首→末/峰 | native线程首→末/峰 | CPU秒 | CPU ms/成功请求 | 平均核数 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| go | 39.340→40.012 (+1.71%) | 40.512 | 38→38/42 | 10→11/11 | 138.57 | 0.478 | 0.0770 |
| rust | 37.438→37.402 (-0.09%) | 38.004 | 27→27/28 | 8→8/8 | 2033.60 | 7.012 | 1.1297 |

Go goroutine首/末5分钟中位76→75，峰83；Rust async task不可用。Go native线程在前5分钟后中位稳定为11，Rust始终为8。Go RSS缓慢上升约0.67MiB，Rust中位近乎持平；这是观察到的趋势，不是无泄漏证明。

| 5分钟区间 | Go RSS中位MiB | Rust RSS中位MiB | 样本数 |
| --- | ---: | ---: | ---: |
| 0–5分 | 39.340 | 37.438 | 1481 |
| 5–10分 | 39.727 | 37.469 | 1480 |
| 10–15分 | 39.801 | 37.453 | 1480 |
| 15–20分 | 39.699 | 37.414 | 1480 |
| 20–25分 | 39.980 | 37.414 | 1481 |
| 25–30分 | 40.012 | 37.402 | 1481 |

CPU ms/成功请求 = CPU秒×1000/成功数；平均核数 = CPU秒/实际负载秒。负载生成器和fixture分开计，分别为：

- client: 89.39 CPU秒，0.308ms/成功请求，平均0.0497核；采样峰RSS 156.176MiB。
- fixture: 33.50 CPU秒，0.116ms/成功请求，平均0.0186核；采样峰RSS 71.082MiB。

## capacity32 恢复探测

配置的常规warmup/load为c16、1秒/3秒；独立probe先c64 burst发起2秒，再c1寻找持续2秒零失败窗口，恢复计时上限10秒。burst与恢复阶段分别使用最多2个和1个worker。

| 角色 | burst实际秒 | burst 200/503 | 503比例 | c1窗口成功/失败 | c1窗口实际ms | 报告恢复ms | burst末请求→计时起点ms | burst末请求→成功窗末ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 2.319 | 185/18182 | 98.99% | 179/0 | 2010.416 | 2061.020 | 316.836 | 2377.856 |
| candidate | 2.124 | 288/20974 | 98.65% | 256/0 | 2008.202 | 2051.317 | 951.183 | 3002.500 |

两版均第一个c1窗口成功，probe passed=true，随后常规warmup/load均零错误；burst、恢复窗及常规load的四项quality均true。没有自动重试整个trial。报告恢复值包含完整2秒成功窗口，而结束burst请求与开始恢复计时之间包含资源采集/worker清理等收尾。两版约10ms的报告差异不能用作恢复速度优劣结论。

| 角色 | c16 warmup成功/失败 | c16 load成功/失败 | load实际秒 | load RPS | load P99ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline | 102/0 | 279/0 | 3.148 | 88.622 | 211 |
| candidate | 181/0 | 496/0 | 3.081 | 160.973 | 143 |

health端点前后均HTTP200且storage缓存为healthy；但overall/auth_bridge缓存状态仍degraded，burst前后快照甚至复用同一last_checked_at。恢复后bridge reason变为connected。primary队列历史峰值baseline33/268ms、candidate10/5ms，但该字段不统计auth pool，且缓存快照不能定位burst瞬时饱和层。

## 证据边界

- 只读取既有soak汇总，核对其与results字段/桶边界一致；没有读取大型raw，不能独立重算采样中位数或逐请求正确性。
- 单候选30分钟单场景只能说明该窗口内未观察到请求失败；5分钟中位数趋势不能证明无泄漏，未做停载/GC后回收观察。
- Go goroutine来自缓存health快照，重复样本不独立；Rust异步task计数不可用，native线程数不能替代。
- RSS按负载中采样计，Go/Rust峰值不一定同时；不能等同VmHWM。CPU含窗口内后台任务，不是请求延迟。
- 恢复只做一轮capacity32、burst c64，再降c1；不能证明正常capacity、持续过载、多轮或所有路径的恢复行为。
- recovery_elapsed包含2秒连续成功窗口，起点在burst workers退出及phase收尾之后；不等于首次成功时间，也不能据约10ms差异宣称候选恢复更快。
- 503证明burst产生了可观察拒绝压力；没有请求体/日志归因，不能据此确定Go还是Rust哪层容量首先触发。
- health HTTP200仅说明端点可读；缓存overall/auth_bridge仍degraded，不能声明所有组件健康。storage队列仅primary，不能外推auth reader队列。
- burst的低延迟/高总RPS主要包含503；recovery后的c16三秒load和validity比较只有一对，均不用于性能收益验收。

本分析已结束，不监测或干预后续并发补测。
