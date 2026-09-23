# post5 规模、参数与 profile 离线审阅

仅离线读取各case的results.json/manifest.json，并验证完整trial集合；本次收集仅复制已完成并写入completed-cases的阶段。未访问业务端口、未启动负载。所有规模/参数组仅1对，不能作六对收益验收或据此改变默认。profile 与性能短测分开解读。

规模组是旧base5→最终candidate5，c1；参数组是最终candidate5-z-reader1→单因素变体，c16。均TOTP、accounts/sessions1000、mobility关闭、正负TTL0、Tokio2、正常capacity无覆盖。非profile组每trial warm5/load15；1pair固定AB顺序，各组也有时间顺序，不能估计顺序偏差。grants10000的15秒窗口只有基线43/候选132次成功请求，P99尾部样本很少。

每组manifest.config与冻结对应config逐字段相同，完整四条variant/component/path/SHA身份与固定制品匹配；每trial角色名、metadata、env/有效env、并发和TTL均已核对。reader组只改变reader数量；compiler组只改变Rust路径及opt元数据，均以最终z/reader1为基线。未使用primary队列health字段推断auth reader队列。

CPU ms/成功请求 = cpu_ms_per_1000_success / 1000；平均核数 = 进程CPU秒 / 实际请求窗口秒。包含同窗口后台CPU，不等于请求延迟。服务RSS是Go/Rust各自采样峰值之和，两峰不保证同时发生；不是VmHWM、空闲回收值或无泄漏证明。

| 阶段/路由 | RPS 基线→候选 (变化) | P99 ms 基线→候选 (变化) | 服务RSS MiB 基线→候选 (变化) | Rust CPU ms/成功请求 | Go CPU ms/成功请求 |
| --- | ---: | ---: | ---: | ---: | ---: |
| grants-1000/grant_hit | 34.53→132.87 (+284.8%) | 35.0→11.0 (-68.6%) | 74.79→75.74 (+1.3%) | 26.969→6.189 | 1.178→0.963 |
| grants-10000/grant_hit | 2.81→8.75 (+210.9%) | 372.0→131.0 (-64.8%) | 74.64→74.69 (+0.1%) | 352.791→112.197 | 1.628→1.364 |
| explore-reader-2/bootstrap | 397.32→393.89 (-0.9%) | 53.0→55.0 (+3.8%) | 76.41→76.92 (+0.7%) | 3.151→3.172 | 0.414→0.390 |
| explore-reader-2/session_hit | 484.98→610.63 (+25.9%) | 45.0→38.0 (-15.6%) | 78.77→79.52 (+0.9%) | 2.881→2.897 | 0.458→0.460 |
| explore-reader-2/grant_hit | 1036.44→1518.50 (+46.5%) | 25.0→17.0 (-32.0%) | 76.67→77.39 (+0.9%) | 1.008→1.105 | 0.531→0.422 |
| explore-reader-2/auto_ip_hit | 165.32→178.96 (+8.3%) | 108.0→111.0 (+2.8%) | 77.20→80.94 (+4.8%) | 6.787→9.413 | 0.485→0.468 |
| explore-reader-4/bootstrap | 394.81→389.82 (-1.3%) | 56.0→54.0 (-3.6%) | 77.46→77.46 (+0.0%) | 3.177→3.176 | 0.411→0.374 |
| explore-reader-4/session_hit | 488.46→660.82 (+35.3%) | 44.0→36.0 (-18.2%) | 80.61→79.65 (-1.2%) | 2.931→2.910 | 0.468→0.456 |
| explore-reader-4/grant_hit | 1039.97→1643.74 (+58.1%) | 23.0→18.0 (-21.7%) | 76.20→79.84 (+4.8%) | 1.014→1.276 | 0.539→0.366 |
| explore-reader-4/auto_ip_hit | 166.36→153.24 (-7.9%) | 111.0→148.0 (+33.3%) | 76.69→86.02 (+12.2%) | 6.659→14.557 | 0.487→0.502 |
| explore-compiler-s/bootstrap | 385.34→398.47 (+3.4%) | 64.0→53.0 (-17.2%) | 75.52→80.34 (+6.4%) | 3.242→3.129 | 0.427→0.448 |
| explore-compiler-s/session_hit | 479.08→543.93 (+13.5%) | 45.0→40.0 (-11.1%) | 78.54→79.44 (+1.1%) | 2.932→2.393 | 0.483→0.464 |
| explore-compiler-s/grant_hit | 996.63→1082.63 (+8.6%) | 28.0→24.0 (-14.3%) | 74.83→77.70 (+3.8%) | 1.041→0.943 | 0.555→0.543 |
| explore-compiler-s/auto_ip_hit | 166.69→207.98 (+24.8%) | 107.0→89.0 (-16.8%) | 77.14→78.98 (+2.4%) | 6.648→5.203 | 0.482→0.459 |
| explore-compiler-2/bootstrap | 401.69→422.70 (+5.2%) | 54.0→53.0 (-1.9%) | 74.86→85.76 (+14.6%) | 3.152→2.943 | 0.414→0.413 |
| explore-compiler-2/session_hit | 486.46→587.40 (+20.7%) | 47.0→38.0 (-19.1%) | 77.83→89.70 (+15.3%) | 2.866→2.175 | 0.469→0.437 |
| explore-compiler-2/grant_hit | 1049.93→1211.78 (+15.4%) | 23.0→20.0 (-13.0%) | 75.58→86.38 (+14.3%) | 1.007→0.842 | 0.540→0.515 |
| explore-compiler-2/auto_ip_hit | 167.31→236.21 (+41.2%) | 109.0→80.0 (-26.6%) | 76.68→88.76 (+15.8%) | 6.599→4.541 | 0.485→0.453 |
| explore-compiler-3/bootstrap | 386.77→411.20 (+6.3%) | 55.0→61.0 (+10.9%) | 75.80→86.90 (+14.6%) | 3.188→2.968 | 0.416→0.418 |
| explore-compiler-3/session_hit | 489.19→600.87 (+22.8%) | 44.0→37.0 (-15.9%) | 77.67→90.49 (+16.5%) | 2.887→2.161 | 0.473→0.445 |
| explore-compiler-3/grant_hit | 1048.71→1209.35 (+15.3%) | 23.0→20.0 (-13.0%) | 75.16→87.43 (+16.3%) | 1.007→0.842 | 0.540→0.513 |
| explore-compiler-3/auto_ip_hit | 166.09→230.92 (+39.0%) | 111.0→79.0 (-28.8%) | 77.63→89.80 (+15.7%) | 6.615→4.686 | 0.477→0.446 |

## 单对探索观察

- reader4的auto-IP出现明显单对回退：RPS-7.9%，P99+33.3%，服务RSS+12.2%，Rust CPU/成功请求+118.6%。grant/session收益不能抵消此路径风险，也不能证明其他并发/规模的结论。
- compiler s的四路RPS变化范围+3.4%至+24.8%，服务RSS变化+1.1%至+6.4%。这是速度与内存取舍，单对不能决定替换默认z。
- compiler 2的四路RPS变化范围+5.2%至+41.2%，服务RSS变化+14.3%至+15.8%。这是速度与内存取舍，单对不能决定替换默认z。
- compiler 3的四路RPS变化范围+6.3%至+39.0%，服务RSS变化+14.6%至+16.5%。这是速度与内存取舍，单对不能决定替换默认z。
- 所有探索只有1对；不据此调整reader1/z默认。profile只用于定位下一步调查方向，不能与无profile短测混合统计或声称正式收益验收。

## Profile（独立30秒capture、idle5秒）

以下是进程级共享recorder的executor jobs与admission scopes，不是SQL statement数，也不保证都来自前景请求。所有auth reader slot共用recorder；kind=sqlite_auth_read 是执行，kind=sqlite_admission且label=sqlite_auth_read 是入场等待。checkpoint gate等待及提交到worker开始执行间的调度延迟不包含在这两类wall time内。primary数据按kind=sqlite_primary及admission同名label单独求和；不能从overall admission减auth推算primary，因为overall还包含其他reader。primary queue的health字段只属于primary，不代表auth pool；health ping来自另一reader。旧版没有admission instrumentation，null表示不可观测，不能读为零等待。累计wall/request可叠加多个job与并发等待，不是请求延迟分解百分比。idle5秒没有load期的周期health采样，不直接从load扣除idle值。

执行wall/CPU包含SQLite闭包内查询、解析和投影等工作，并非只计SQL引擎耗时。profile中的吞吐带有instrumentation成本，不替代无profile短测。旧auto-IP路径没有观察到auth-reader执行，相关null不能推断为存在一条零成本auth查询。执行成本随reader增加的原因仍需进一步归因，本表不能单独确定是分配器、锁或CPU竞争。

| 阶段/路由/角色 | auth jobs/成功请求 | auth执行wall ms/成功请求 | auth执行CPU ms/成功请求 | auth admission ms/成功请求 | primary jobs/成功请求 | primary admission ms/成功请求 | auth未结束scope | 总dropped |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| profile-primary/grant_hit/baseline | 6.000 | 2.764 | 2.734 | null | 0.017 | null | 0 | 0 |
| profile-primary/grant_hit/candidate | 2.000 | 0.633 | 0.611 | 13.366 | 0.004 | 0.000 | 0 | 0 |
| profile-primary/session_hit/baseline | 3.000 | 0.411 | 0.241 | null | 7.026 | null | 0 | 0 |
| profile-primary/session_hit/candidate | 7.000 | 0.683 | 0.587 | 26.273 | 1.010 | 0.452 | 0 | 0 |
| profile-primary/auto_ip_hit/baseline | null | null | null | null | 3.052 | null | 0 | 0 |
| profile-primary/auto_ip_hit/candidate | 3.000 | 4.611 | 4.529 | 79.653 | 0.029 | 0.000 | 0 | 0 |
| profile-reader-2/grant_hit/baseline | 2.000 | 0.630 | 0.609 | 13.358 | 0.004 | 0.000 | 0 | 0 |
| profile-reader-2/grant_hit/candidate | 2.000 | 0.983 | 0.794 | 7.930 | 0.003 | 0.000 | 0 | 0 |
| profile-reader-2/auto_ip_hit/baseline | 3.000 | 4.525 | 4.450 | 78.043 | 0.028 | 0.000 | 0 | 0 |
| profile-reader-2/auto_ip_hit/candidate | 3.000 | 10.923 | 9.087 | 81.540 | 0.032 | 0.000 | 0 | 0 |
| profile-reader-4/grant_hit/baseline | 2.000 | 0.635 | 0.615 | 13.421 | 0.004 | 0.000 | 0 | 0 |
| profile-reader-4/grant_hit/candidate | 2.000 | 1.899 | 1.002 | 5.527 | 0.003 | 0.000 | 0 | 0 |
| profile-reader-4/auto_ip_hit/baseline | 3.000 | 4.606 | 4.530 | 79.211 | 0.028 | 0.000 | 0 | 0 |
| profile-reader-4/auto_ip_hit/candidate | 3.000 | 21.636 | 12.189 | 64.922 | 0.032 | 0.000 | 0 | 0 |

已完成14个profile capture；dropped总计0，全部kind未结束scope总计0。

完成阶段：grants-1000, grants-10000, explore-reader-2, explore-reader-4, explore-compiler-s, explore-compiler-2, explore-compiler-3, profile-primary, profile-reader-2, profile-reader-4

状态：profile-reader-4已完成，本审阅在此停止，不监测soak。
