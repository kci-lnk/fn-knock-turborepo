# 未入选的候选与无效实验

这些结果保留用于复核，不能合并进最终六对验收。

| 检查点 / 实验 | 排除原因 | 后续处理 |
| --- | --- | --- |
| Rust `144f5e7f` 初始请求上下文 | 大账户规模下跨 await 留存整张标准化列表，session 场景合计峰值 RSS 约增加 17.8%，未过 5% 门槛 | 改为保留原始快照、按需提取首个 ID，第二个不同 ID 才建立索引；重新验证 |
| Rust `60c95286` 选择性凭据读取 | 自写跳过 JSON visitor 没有完全保留 `serde_json/raw_value` 特殊键行为；auto-IP 仍跨 await 留存大量不匹配 session | `32d1aa53` 恢复完整 Value 解析语义；`fde0e280` 在 SQLite worker 内投影成匹配 ID，释放不匹配对象 |
| 初始 c64 短测 | 实验工具隐式设置 bridge capacity=32，造成预期外 503 | 删除正常实验中的该覆盖；base/base c64 control 通过。显式 capacity=32 仅作为独立饱和/恢复用例 |
| `accounts1000-c16-v3` | Go 构建遗漏运行时 `version.Commit` 注入，Rust bundle 身份校验拒绝启动 | 保留失败制品及记录；正确注入后使用新目录 `accounts1000-c16-v3b`。失败启动不计为性能退化 |
| Rust `c65503d6` 与 `formal-v3-main` | IP owner 候选直接 raw→LoginSession 解析，与旧列表 raw→Value→LoginSession 不同。重复字段或 RawValue 包装可能少算 owner，使原本两个 owner 的拒绝变成唯一 owner 选择 | 在 2026-09-23 07:25 UTC 左右终止实验，保留已完成的 12 个 trial（6 个单对目标场景）；三个保护场景尚未完成。恢复列表解析语义并保留最终权威重读后，冻结新源码重新跑正式矩阵 |
| Go `748c97e` / `0978d6b` 普通 Cookie 快速路径 | 大量空 Cookie 分段会触发 Go 上游的 cookie 数量上限；非空分段边缘的 Unicode 空白也可能令原本可识别的普通 cookie 被丢弃。这些输入在旧 helper 中先经过清理 | `0978d6b` 恢复空分段清理，`4d15fa3` 恢复非 HTTP OWS 边缘空白清理；常规 Cookie 保持快路径，最终基准改为原始 `92d4c0c` 直接对比 `4d15fa3` |
| `formal-v4-main` 与 v4 短测 | Go 基线使用 `-s -w` 与显式 Version/Commit 链接参数，candidate4 只注入 Commit，构建控制变量不一致 | 08:09 UTC 左右停止精确实验 PID，保留 56 个完成 trial（正式 12 个），不用于源码收益验收。按相同 release flags 从两份精确源码归档重建 Go，基线逐字节相同；candidate5 使用新制品 SHA。Rust/Go 源码与验收门槛均不变，完整重跑 v5 |

`c65503d6` 的 12 个已完成正式 trial 均通过响应和采样质量检查，但这不覆盖后来发现的特殊 JSON 兼容缺陷，因此它们仍然被排除。已编译完成的该检查点 `z/s/2/3` 制品亦不用于最终编译参数结论。

证据：

- [初始候选原始 JSON 归档](results/initial-evidence.tar.gz)：包含早期有效、失败与探索试次。
- [c655 检查点原始 JSON 归档](results/rejected-c655-evidence.tar.gz)：76 个 JSON 文件，包括基线 capacity control、bundle 启动失败、c655 短测和中止的正式批次。
- [v4 构建参数差异归档](results/rejected-v4-link-flags-evidence.tar.gz)：67 个 JSON 文件；56 条完成试次全部通过响应检查，但不用于匹配构建参数后的验收。[停止与哈希记录](results/rejected-v4-link-flags-manifest.json)。

归档保留每 trial 的有效性、资源、请求统计及原始路径引用；路径仍指向隔离实验目录。原始目录中的数据库、运行日志和二进制没有打入 Git 归档。归档中的中止批次只能用于追溯，不得补齐或重编号成新源码的独立样本。
