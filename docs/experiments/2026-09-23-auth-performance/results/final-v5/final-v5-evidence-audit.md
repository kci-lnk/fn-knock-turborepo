# v5 最终证据包离线独立复核

审查对象：`/tmp/fn-knock-auth-final5-evidence-collection`。仅查看本地收集索引、复核结果、各组 aggregate/manifest 和 sidecar；没有访问远端、运行负载、编译、修改仓库或重读全部大 raw。本次唯一写入为本报告。

## 结论

未发现阻塞归档交付的缺项、计数冲突或源 SHA/收集 SHA 混用。已收集 **361 个源文件、22 个 job、261 个 trial**；`REVIEW.json.complete_and_consistent=true`、errors为空，与独立汇总一致。需要保留一个已知预热质量例外：不能写“261个trial的所有预热质量均通过”。

## 数量与分组

| 分组 | trial数 | 证据用途 |
| --- | ---: | --- |
| 预备 smoke（4组） | 44 | 单对探索/语义检查，规模不同，不拼并发曲线 |
| 主矩阵 | 108 | 9路由×6对×双角色 |
| 正常缓存TTL1 | 12 | 独立六对回归验收 |
| 有限grant续期 | 12 | 独立六对，每trial64个load token，非持续60秒 |
| grant规模1000/10000 | 4 | 单对探索 |
| reader2/4 | 16 | 单因素、单对探索 |
| compiler s/2/3 | 24 | 单因素、单对探索 |
| profile primary/reader2/reader4 | 14 | 插桩诊断，executor jobs而非SQL statements |
| candidate-only soak | 1 | 1800秒稳定性观察，非A/B |
| 显式capacity32 recovery | 2 | 故障注入，不能混入正常性能 |
| 同规模c1/c16/c64 | 24 | 每组单对并发探索，非六对收益 |
| 合计 | 261 | 只用于证据完整性计数 |

每个job的route/pair/role集合、无重复性和AB/BA顺序独立核对通过。261个aggregate的`raw_result_file`恰好映射到各自job的261个收集raw文件，无漏项或重复映射。raw未重新逐文件读取，其字段一致性依赖本次collector已完成的校验及留存哈希；本审查另检查全部raw路径存在且字节数符合索引。

主矩阵、cache和renewal各有pair 0..5，三份独立acceptance均passed=true、failures=[]。其plan/results/manifest的共9个输入SHA全部匹配collection中**源文件SHA**。本次没有重写统计、重算新的bootstrap、挑选路由或追加gate。

## 请求与质量记录

- 正常measurement合计 **17,825,753** 次，successful相同，failures=0、anomalies=0、latency_overflow=0；200为14,437,310，302为3,388,443。这里包含不同用途/时长实验的工作量，不是总吞吐或成功授权量。
- warmup合计 **5,891,479** 次，successful相同，failures=0、anomalies=0、latency_overflow=0；200为4,764,119，302为1,127,360。
- 261条measurement的四项quality标志与validation.passed均为true。
- **唯一warmup质量例外**：`concurrency5-c64`、baseline、`auto_ip_hit`、pair0，预热请求零失败，但elapsed=5802.480ms，相对请求的5000ms超出802.480ms；原有容忍为max(500ms,5%)=500ms，因此warmup.duration_ok=false。该warmup没有周期资源/health采样（max_sampling_gap_ms=null），不能把sampling_ok=true当作有采样证据。
- 同一trial的load elapsed=15604.212ms，较15000ms超出604.212ms，仍在原有750ms容忍内，四项load quality=true、validation=true。原harness检查预热请求错误、最终validation使用load质量；collector没有新增或放宽该规则。本次保留并披露这条false，不把它伪写为true，也不据此新增回溯门槛。

## 诊断、soak与恢复没有混算

14个profile均手动stopped、dropped_operations=0，归一化分母等于相应成功请求数；这些只用于诊断，不能替代未插桩性能。soak为1条candidate-only，elapsed=1,800,071.056ms、290,000请求、零请求错误，完成标志true；这不含资源增长验收门槛，不证明没有泄漏。本次没有重读/重算大soakraw趋势，采用root已完成的raw重算边界。

recovery两端有效runtime均显式capacity32，64并发burst真实产生 **18,182 / 20,974 次503**（baseline/candidate），另有185/288次200。burst中的这些失败独立保留，**不包含在上述正常measurement零错误中**。两端随后各一次check完成连续2秒零错误窗口：check elapsed=2010.416/2008.202ms，recovery_elapsed=2061.020/2051.317ms，均在10000ms截止内；之后正常load共775请求全部符合预期。

`recovery_elapsed_ms`从burst worker清理结束后起算，包含完整2秒成功观察窗口，不是首次恢复延迟；`observed_until_ms`=3075.776/3058.663还包括管理采样收尾。before与after-burst health的last_checked_at相同，属于既有5秒缓存快照，且没有逐请求503原因/bridge峰值证据；只能说该显式容量压力下观察到拒绝后恢复，不能把每个503定因于Rust排队或Go queue full，亦不能推广至生产默认容量。

## 索引与双SHA

- 收集源路径和目标相对路径各361个唯一值；文件均存在且大小符合索引。构成为261 raw +44 results/manifest +37 case sidecar +19全局支持文件；目录内总366文件，额外5个是收集/复核封装文件，不计为trial。
- 全部100个非raw源文件的本地内容SHA独立重算，均匹配`collected_sha256`。4个INDEX-SHA256封装引用也独立重算全部通过；REVIEW-SPEC、总计划及collector脚本身份吻合。
- 309个文件的source与collected SHA不同，不能将这种差异自动解释为脱敏：JSON格式规范化/末尾换行也会改变SHA。索引记录真正发生字符串脱敏的是6文件、58字符串。
- 10份先前独立下载的main/preliminary原文件另行哈希，均与本包相应`source_sha256`一致；正式gate输入也使用源SHA，不误拿脱敏后的SHA比较。
- `source_consistency_checked=true`且`sources_changed_during_collection=[]`；原始远端字节是否稳定是collector导出时做的检查，本次不再访问远端。所有缺失项仅22个明确可选的`.outcome.json`，没有必需sidecar缺失。
- 本报告不重复扫描约77.49MB的全部raw内容；其哈希值被collection.json及最终tar哈希链绑定，但本次独立重哈希范围是100个非raw文件及4个封装引用。

collector定位为归档完整性/一致性检查；源码→产品制品关系由独立build manifest、正式验收和参数冻结配置核对证明，不能把本次归档复核包装成重新证明所有参数单因素或二进制源码来源。

有一项工具边界应准确描述：当前`review()`仅在INDEX-SHA256.json存在时校验其中项目，不把“缺失INDEX”本身作为错误。**本包INDEX实际存在，预期4项齐全且逐一哈希通过**，因此不影响当前交付；若未来宣称verify能拒绝缺失INDEX，则该宽泛主张不成立。本次不修改collector。

## 输入封装SHA

- `INDEX-SHA256.json`：`63be43f190f6053f9d6d7ebe56e2c5447ea93fae71650c8a196db93267415ccd`
- `collection.json`：`e8df3491622ce6f992ce59dfe6c0dcadd33ca518ff091d7823ec352ae5082aee`
- `REVIEW.json`：`cb8644359078a3a10a2dd8cd0952f34be144f02a8fc6834b93d850aaf9f0c7e4`
- `REVIEW-SPEC.json`：`bbd95b51c118be0e84f6218fe85eeace99921044edde8022a4f91400fe20d5a9`
- `MASTER-COLLECTION-PLAN.json`：`b18e7d405d88d10eed0484f2c69c280d68bcf2e1e7713f7349ffa57d4c99f2c8`
- collector脚本：`ee21d25f87522d3afa289a4857c60c1debfb72e1e01ed610d8aff8b65d0f84f8`
