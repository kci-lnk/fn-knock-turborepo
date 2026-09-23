# audit6 后半段离线分析

TTL1六对12条已完成并通过。用户要求尽快收尾，soak按此请求中止，未作30分钟完成验收；recovery未执行。本包不将整批audit6标为passed。

比较为 Go4d → Go669 的审计修复增量；Rust源码同为5fdd，但各自绑定对应Go身份重建。固定c16/clients2、accounts/sessions1000、ordinary grants100、TOTP、mobility=false、reader1、z/fat/CGU1、Tokio2；正常桥接容量缺省。

停止信息来自root转达的用户收尾决定；本包在该指令后停止远端读取，未采集中止后的outcome，清理及中止证据由root另行保留。TTL1六份原输入及其源SHA保持不变。

## TTL1：六对session_hit

12trial，warm20秒/load60秒，前后正/负TTL均读回1；常规load成功10050318，失败0；warm成功3161841，失败0。六对非回退门槛通过：RPS中位变化≥−5%、P99≤+10%、合计RSS≤+5%，没有要求达到新增收益。

| 指标 | baseline六trial中位 | candidate六trial中位 | 配对变化中位 | 配对bootstrap 95% |
| --- | ---: | ---: | ---: | --- |
| RPS | 13848.497 | 14010.671 | +0.839% | -0.359% 至 +2.986% |
| P99 ms | 4.000 | 4.000 | +0.000% | -10.000% 至 +0.000% |
| Go+Rust各自峰值之和 MiB | 77.391 | 78.557 | +1.429% | 未计算 |

仅作区分：角色中位数之比变化为RPS +1.171%、P99 +0.000%、合计RSS +1.507%，不能代替上表配对中位变化。

| 进程 | CPU ms/成功请求：base→candidate中位 | 配对变化中位 | 平均核数：base→candidate中位 | RSS采样峰MiB：base→candidate中位 |
| --- | ---: | ---: | ---: | ---: |
| go | 0.108265→0.107511 | -0.858% | 1.50114→1.50981 | 42.729→43.113 |
| rust | 0.000373→0.000362 | -4.041% | 0.00517→0.00508 | 34.625→35.352 |
| client | 0.058805→0.058579 | -0.306% | 0.81707→0.82148 | 199.586→199.232 |
| fixture | 0.031705→0.031439 | -0.623% | 0.43733→0.43824 | 84.779→91.350 |

各对变化、每trial原始资源基数与进程身份见TAIL-SUMMARY.json；CPU无新增显著性或收益门槛。Rust每个60秒trial累计CPU仅29–33个10ms tick，相对小变化对计数粒度及后台CPU敏感，不宣称Rust CPU改善。
吞吐配对区间跨零；本次结论是未超过冻结回退预算，不是已证明新增加速。

## 证据与边界

- 本批只评价既有优化后的 Go4d 到修复 Go669 的增量；Rust 源码仍5fdd，仅 expected gateway commit 及构建制品变化；不与以前273个trial合并。
- 原计划本报告覆盖后半段15trial；收尾时实际仅验收TTL1六对12trial。soak按用户请求中止，recovery未执行；前半段30trial由root另行复核，不声称整批45trial完成。
- 稳定session负载没有主动注销；竞态正确性由确定性barrier测试证明，不由这些性能数字证明。
- RPS/P99配对中位变化和固定种子bootstrap95%区间直接复用冻结工具；绝对列为各角色六trial中位数，两个角色中位数之比另列，三者不得互换。P99并非合并请求分位数。
- Go与Rust各自采样RSS峰值之和不一定同时达到；不是VmHWM。CPU ms/成功请求=进程CPU秒*1000/成功数；平均核数=进程CPU秒/实际load秒，含后台与采样收尾，100Hz精度，不是延迟或单请求追踪。CPU配对变化仅描述，不设新增门槛。
- 预定单候选单场景30分钟协议没有资源增长验收门槛；五分钟中位趋势不证明无泄漏。未测停载/GC后回收。Go goroutine来自缓存health，重复观察不独立；Rust async task不可用，native线程不能替代。
- health_ok只说明既有采集成功，不能推断所有缓存组件都healthy；primary队列快照不覆盖auth reader队列，五秒缓存不能定位每个503根因。
- 冻结恢复协议仅包含一轮capacity32/c64两秒burst，再c1连续两秒成功；recovery_elapsed从burst worker清理及收尾后开始，包含完整成功窗，不是首次恢复延迟。单轮差异不做恢复速度收益声明。
- burst的503单独列出，不混入常规warm/load质量计数；存在503仅证明可观察拒绝压力，不能确定Go或Rust哪一层容量首先触发。无503不追加失败门槛，但不能声称已观察饱和。
- 分析完全离线。原始pull按固定文件只读SSH收集、不访问业务端口、不写远端、不启动负载；本地输入核对收集SHA，source index另存源SHA。统一collector的JSON重编码/脱敏可改变收集SHA，两种SHA不混淆。源码到制品关系另依赖冻结build manifest及构建声明，hash不证明编译过程本身可信。

冻结plan SHA256：87e69d8e1dedce80e7ddb6ea574fa1145975123cdca60b3b662f0637a78002ce。本地复跑：`node analyze.mjs --partial`，可显式传入归档路径，详见README；不需要远端或产品二进制。source-files.json或collection.json保留源路径、源SHA、收集SHA及大小。
