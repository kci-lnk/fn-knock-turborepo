# concurrency5 单对探索复跑包

[Corrected analysis](concurrency-analysis.md), [JSON](concurrency-analysis.json), [original failed report](concurrency-original-failed-analysis.md), and [archive manifest](concurrency-replay-archive.json).

Extract the portable bundle to a new directory, then run its wrapper:

```sh
mkdir /tmp/concurrency-replay-UNIQUE
tar -xzf concurrency-replay.tar.gz -C /tmp/concurrency-replay-UNIQUE
python3 -B /tmp/concurrency-replay-UNIQUE/concurrency-replay/replay.py \
  --out /tmp/concurrency-result-UNIQUE
```

The public concurrency-analysis.md adds archive context and links; expected/analysis.md in the bundle preserves the original corrected report byte-for-byte. Both pre-archive and extracted-archive replays matched all results and SHA checks.


仅包含已完成的同规模 c1/c16/c64 补测，不含正在准备的 c64 六对 confirmation。24 条测量通过冻结合同及原有测量门禁；一个 warmup 时长 warning 必须保留。单对 c64 session_hit 合计峰值 RSS 为 88.29→94.10 MiB（+6.58%），需要独立六对复核，不据此修改 reader=1/opt-level=z 默认。

使用 Python 3 标准库，在任意解压位置运行（输出目录必须不存在）：

```sh
python3 -B replay.py --out /tmp/concurrency-replay-UNIQUE
```

该命令不联网、不编译或执行产品、不发起负载，只验证包内文件 SHA，运行原样保存的 corrected analyzer，并核对全部分析内容。移动目录只改变 analysis.json 的 inputs[].path；剔除这一个路径字段后须完全相等，所有数值、身份、输入SHA、字节数、错误和警告均保留。analysis.md 必须逐字节相同。验证结果写入输出目录 REPLAY-VERIFICATION.json。

- `inputs/`：已脱敏收集的12个真实输入（3档 × results/manifest/contract/validity）。没有修改测量内容或重新运行实验。
- `plan/`：原样冻结的4个计划文件，manifest SHA 0616fe8895470b17f51a33594116dc43a0e6ba4f936cdb266cc9176a424bc0e5。
- `tools/`：原样 corrected analyze.py、测试、说明及执行/准备记录。分析器SHA与最终报告相同；不依赖其默认/tmp路径，wrapper显式传--plan。
- `expected/`：原样 final-report-corrected 三文件。
- `audit/original-failed-report/`：原始 final-report 三文件，保留首版分析失败。该失败来自分析器新增了冻结协议没有的 warmup 5.5秒上限硬门槛；不是请求失败或数据被替换。
- `audit/corrected-first-render/`：首次修正输出，保留中间文档渲染记录。

c64 baseline auto_ip_hit 的 warmup=5802.480ms、duration_ok=false、448响应全200且零失败；load=15604.212ms，在15000+750ms原有容忍内，load四质量标志/validation通过。修正分析器恢复原协议：预热响应正确性仍为gate，预热时长偏差另列warning。原始与corrected报告的14个输入记录（12测量/sidecar加2计划JSON）完全一致。

保留的原始失败报告用于审计，不声称当前 corrected analyzer 会复现旧错误门槛；旧版分析器源码未随原始报告保存。没有删除或改写该失败报告，也没有对统计门槛作事后调整。所有变化百分比仅为一次配对观察，不是六对收益、显著性或默认参数建议。
