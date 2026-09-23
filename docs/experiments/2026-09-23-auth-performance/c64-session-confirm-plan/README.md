# c64 session_hit 独立六对确认

状态：**prepared_not_executed**。本目录只准备；没有上传、启动负载、编译或修改产品。root 审阅后决定执行。不得修改或合并已完成的261条结果；这次12条属于独立、事后指定的确认实验，不追溯改变原计划，也不与正式c16六对或短测的一对混算。

触发依据为同规模 c64 短测 session_hit 单对 Go+Rust 峰RSS：基线92,573,696 bytes（88.285MiB），候选98,668,544 bytes（94.098MiB），观测增长6.5837794788%。单对不能确认稳定回归，故冻结新的六对协议。`trigger-evidence.json` 固定原短测 aggregate、manifest、contract、validity 与两个raw trial的 **source/collected SHA256**；原来源仍是 warm5/load15，不能改称正式六对证据。

| 项目 | 冻结值 |
| --- | --- |
| 对比源码 | Rust d4f8805f → 5fddf896；Go 92d4c0c → 4d15fa3 |
| 制品/工具 | 原 base5/candidate5 四个制品，原12个scripts5 SHA + control SHA；不重编译 |
| 场景 | session_hit，c64，clients2 |
| 每 trial | warm20秒 / load60秒；6对共12条，AB/BA交替 |
| 种子 | accounts1000/session1000/ordinary grants100，TOTP；无renewal token |
| 缓存 | 正/负TTL均0，测量前后实际运行值读回 |
| 运行配置 | Rust z；候选reader1，基线原单reader；Tokio2；normal capacity不覆盖 |
| 诊断 | profile0、recovery0；roles=baseline,candidate |
| 参数默认 | 不调整产品默认值 |

`variants5.json` 与原 concurrency5-prep 逐字节一致，SHA为 `95b41efe37dfb7bc13ec380961c768ec832200fe541a37f0413158e1a8e9a28b`。Normal capacity 环境变量从父环境unset；由现有Go1024/Rust Linux64默认处理。每个trial沿用原harness独立产品进程和synthetic数据库。

## 原样门槛

固定运行：`check-auth-performance.mjs RESULTS --require-six-pairs --require-improvement`。

本机 checker 与 lib 的SHA已经与冻结scripts5比对一致；`verify-gates.mjs` 只调用原函数做合成边界测试，不修改checker，也不代替实际最终gate。

- 至少6个有效完整pair；独立contract另外要求**恰好12条、pair0..5、唯一目录与ABBA顺序**，禁止拼入其他试验。
- 配对变化中位：吞吐≥−5%，P99≤+10%。
- 全部pair必须有有效Go/Rust峰RSS；两进程峰值合计的配对变化中位≤+5%。合计使用各进程各自采样峰值之和，不声称是同步总峰值。
- 还必须满足：吞吐中位≥+10%且paired95%下界>0，**或** P99中位≤−15%且paired95%上界<0。
- 原实现边界比较使用1e−12浮点容差；本准备不改数字、算法、门槛或CI定义。没有把更低RSS作为替代收益目标。

合同另核对冻结config、代码/编译/env metadata、四制品/8实际harness文件/control身份、N1000/1000/100、TTL读回、正常capacity unset、preflight真实业务marker、warm/load零错误及load质量。warm必须≥20秒，但不新增warmup.duration_ok上限gate；与原harness保持一致。

## 未来运行（本次未执行）

root 将本目录原样安装到：

`/tmp/fn-knock-auth-performance-20260923/c64-session-confirm-plan`

在目标Linux主机，确认其他实验结束后：

```sh
cd /tmp/fn-knock-auth-performance-20260923/c64-session-confirm-plan
bash run-confirm.sh show
bash run-confirm.sh preflight
AUTH_PERF_NODE=/usr/local/bin/node bash run-confirm.sh run
```

默认action为show。preflight仅哈希和路径核对，不负载。run不可覆盖输出参数，固定新batch：

`/tmp/fn-knock-auth-performance-20260923/c64-session-confirm-final`

run使用原共享 `flock`，并检查不存在其他harness；有冲突立即退出，不终止别的进程，不等待重试。batch已存在则拒绝。SIGINT/TERM仅转发本runner拥有的harness。无自动重跑、参数回退或第二场景。纯请求窗口预计16分钟，启动/采样/清理另计。

## 输出与失败保留

```text
c64-session-confirm-final/
  prepared-manifest.json
  trigger-evidence.json
  started-utc.txt
  session_hit/results.json
  session_hit/manifest.json
  session_hit/<12 trial directories>/result.json
  session_hit.contract.json
  session_hit.gate.json
  session_hit.report.md
  session_hit.*.stderr.log / session_hit.run.log
  outcome.json
  finished-utc.txt
  completed-utc.txt   # 仅全部成功时存在
```

harness退出后先做exact contract，再执行原冻结gate；即使contract或gate失败，只要results.json存在仍尝试生成report。outcome保存各阶段是否执行和原始exit code，并哈希结果、manifest、contract、gate与report。若没有results，gate/report明确标为未执行。report文本不代表gate通过，必须一起读outcome与contract/gate状态。

runner退出：0=全部通过；20=harness失败；21=合同失败；22=严格gate失败/未执行；23=report失败；3=锁或已有harness冲突；2=用法/已有batch等保护；preflight校验失败通常1；INT/TERM为130/143。多阶段失败按harness→contract→gate→report优先决定总exit，所有阶段原始退出仍保留。被中断可能没有最终outcome，已有raw不删除。

归档时白名单应额外保存contract、gate、report、outcome、准备manifest、trigger和完成标记；原suite collector不会自动收齐这些。只归档JSON/审核后的报告和计划，不复制DB、生成凭据config、二进制或原始日志。原始日志仅留隔离/tmp用于失败诊断。

## 本机准备验证

以下只做本机纯函数/语法/合成合同验证：

```sh
python3 -B check-plan.py verify
bash -n run-confirm.sh
node verify-gates.mjs /Users/edgeware/Local/fn-knock-turborepo/scripts
python3 -B test-contract.py
```

不会运行remote preflight或run；实际未来执行仍由root独占协调。结果若失败，保留失败并报告，不通过修改门槛或替换原单对观测来消除反向证据。
