# c64 session独立六对确认归档

12次独立trial，2026-09-23 13:06:59 UTC完成；与原261次分开。冻结Rust5fdd/Go4d、c64/1000账户/1000session/100grant、TTL0、reader1/z、Tokio2，20秒预热/60秒测量，AB/BA交替。合同、冻结收益与合计RSS门槛通过。吞吐配对中位+169.09%、P99−61.99%、合计采样峰RSS+3.99%；Rust单进程+8.02%、Go+0.24%，不能称逐进程内存均无回退。

26个白名单源文件和12份完整raw均保存，另含源/归档SHA映射、原冻结工具/计划、只读分析器。JSON敏感/自由文本经规则处理；原source SHA和包内collected SHA不可互换。无DB、生成凭据配置、密钥、产品二进制或日志。collector完整性通过不替代性能gate，明确结果见evidence/support/outcome.json。

离线复跑（解压后，在本目录）：

```sh
python3 -B collector/collect-final5.py verify --spec collector/review-spec.json --out evidence
python3 -B plan/check-plan.py contract evidence/c64-session-confirm
node tools/check-auth-performance.mjs evidence/c64-session-confirm/results.json --require-six-pairs --require-improvement
python3 -B analysis/analyze.py --input analysis/inputs --out replay-analysis
```

前三项已由root本地执行，严格gate JSON与远端逐值相等。分析只使用小汇总，raw时间曲线由root独立收集，不将RSS当活跃堆字节或泄漏证明。原collector的selftest固定261，不能用于此12条spec；verify与collect按spec工作，静态复核见collector/review.md。checks/记录此批收尾时生产PID/启动时间不变、无残留实验产品进程。

历史4d制品的结论不会自动适用于后续审计修复；后续Go撤销竞态修复独立记录源码和测量。
