# 最终 v5 原始指标与冻结工具

本包保存22组、261次trial的完整指标与361个白名单源文件；另含收集索引、原样冻结的12个实验工具、收集复核脚本及本地复核记录。没有数据库、运行时凭据配置、密钥、日志或产品二进制。JSON中的敏感/自由文本按收集规则去敏，source_sha256与collected_sha256分别记录原始和包内字节，不能互换。

本包对应的261次试验批次已在2026-09-23 12:37:16 UTC前结束；后续c64 session六对独立确认另外归档，不混入本包。本包收集后复读原始文件确认SHA未变。REVIEW.json是完整性/合同复核，不是重新生成性能收益门槛。各正式acceptance、单对validity、recovery和soak保持各自边界。c64 baseline auto-IP的warmup耗时5.802秒、嵌套duration_ok=false；正式load15.604秒在原门槛内且零错误。不得因该预热标志而改写既定验收，也不能隐去它。

## 离线复核

Python3与Node.js即可，以下操作不访问远端或启动负载：

```sh
python3 -B tools/collect-final5.py verify --out evidence
node tools/auth-performance-soak.mjs evidence/post5-soak/candidate5-1-candidate-auto_ip_hit/result.json > recomputed-soak.json
node tools/check-auth-performance-plan.mjs --plan evidence/support/EXPERIMENT_PLAN5.json --results evidence/formal-v5-main/results.json --phase main
```

第三条计划路径请以evidence/collection.json中的support映射为准；计划、结果的JSON字节经过收集处理，因此本地重算输入SHA可与远端原验收不同，指标和判断应保持一致。tools/CHECKLIST.md说明收集与复核步骤；完整运行协议、匹配构建和回滚说明位于源码仓docs/experiments/2026-09-23-auth-performance。

INDEX-SHA256.json给出四个本地索引/复核文件SHA；打包前已另逐项核对。完整包的SHA在仓库旁侧manifest中，不放入包内自我引用。每trial的result.json保留samples/runtime_health/profile/recovery；汇总results.json不含全部时间序列。

soak原始8883个资源样本与1792个health观察已用冻结工具重算，和远端summary完全相同；checks/保存该核对以及生产PID/启动时间不变、实验产品进程已退出的收尾记录。没有部署或重启正式服务。
