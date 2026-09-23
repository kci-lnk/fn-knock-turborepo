# 最终 v5 预备 smoke 离线审查

四组共 **44 个 trial**，证据校验结果：**通过**。每组每路由仅1对；这是探索和正确性 smoke，不是六对正式收益验收，不计算显著性或宣称性能改进。

| 批次 | c / worker | 账户/session/普通grant | 凭证 | warm/load 发起窗 | trial | 测量请求 | 预热请求 |
| --- | --- | --- | --- | --- | ---: | ---: | ---: |
| accounts1000-c16-v5 | 16/2 | 1000/1000/100 | totp | 3/8秒 | 6 | 11,675 | 4,375 |
| smoke-v5-c1 | 1/1 | 1/1/1 | totp | 1/3秒 | 20 | 30,403 | 10,634 |
| smoke-v5-c64 | 64/2 | 100/100/100 | totp | 3/8秒 | 10 | 97,281 | 36,676 |
| password-v5-c16 | 16/2 | 1000/1000/100 | password | 3/8秒 | 8 | 12,047 | 4,569 |

测量共 151,406 次、预热 56,254 次符合预期的响应；各 trial 记录的失败/异常/直方图溢出均为0，预检和四项质量标志全部通过。测量200/302合计分别为137,126/14,280；302是期望拒绝，不等于授权成功。c64 的10个 trial 均为200、没有503；这只证明本次 N100 短窗完成，不能证明默认容量在任意规模均不会饱和。

最大测量期客户端事件循环延迟 27.36ms、启动延迟 1.295ms、采样间隔 218.60ms。timeout phase 事件计数均为0；不能据此推断 SQLite 或 bridge 等待为0。warmup 没有周期健康采样。

c1 的 grant_renewal 每端仅8个独立 load token，全部消费；总 grant 数为18（1普通+1预检+8 warm+8 load），完成时间34.839/23.902ms，不是持续3秒的续期吞吐。原始结果含单对延迟/吞吐，但本报告不将其升级为正式收益。

## 身份、隔离与语义范围

- 基线 Rust d4f8805f / Go 92d4c0c；最终候选 Rust 5fddf896 / Go 4d15fa3。四个制品哈希与 v5 固定值完全一致，完整 SHA 见 JSON。各批次 harness/control 指纹一致；本地保存的 seeder、请求规格/验证器和 worker 源文件 SHA 均匹配运行 manifest。
- 两端 z/fat/CGU1；candidate reader=1，baseline无reader覆盖；Linux、Tokio2、bridge容量无覆盖，正负缓存TTL在每次预检与结束均由runtime控制工具读回0。源码与binary关联仍以构建/运行manifest证据为准，本审查没有反编译或重新获取产品binary。
- 44个独立Go、44个独立Rust PID/start-tick身份，各自从warmup前至load后保持一致，44个trial目录唯一；这些记录不能独立证明数据库内容全新。
- 逐请求语义由同指纹worker实时验证：业务路由需origin特征头及正文；session API需authenticated=true；bootstrap需success及client IP；challenge需challenge/signature；miss需302、Location及无origin标记；renewal还需非删除grant Set-Cookie。结果摘要未保留完整响应body/header，本离线步骤核对验证器源码、状态和零失败记录，不能重新执行每次响应的语义判断。
- password组只覆盖 session_api/session_hit/auto_ip_hit/auto_ip_miss，共8个trial。种子预先创建1000个AuthAccount及TOTP身份，session.method=PASSWORD、credentialId关联account、totpId关联sourceTotpId，权限mode=all；它覆盖已认证session的账户权限读取和auto-IP owner授权。没有执行密码登录、密码hash校验、错误密码、账户变更竞态或密码grant发放，不能称为密码登录性能测试。
- 固定seeder明确设置mobility=false；seed摘要本身没有独立的mobility读回字段，故这是绑定seeder实现的配置证据，并非本次额外读取SQLite配置。

## 比较边界与离线复跑

**不能跨这些批次绘制并发曲线**：c1=N1，c64=N100，c16=N1000，凭证类型和route集合也不完全相同。同规模c1/c16/c64补测应另行分析。这44个trial不能与正式主矩阵凑成更多独立配对。

仅从远端下载四组已完成的 results.json/manifest.json；没有访问业务端口、运行新负载或编译。download-manifest.json 保存来源和输入校验和；JSON另保留逐trial摘要及检查错误。脚本只读原始输入并覆盖自身生成的两个输出，可把整个目录复制到任意位置离线运行：

```sh
python3 summarize-preliminary.py --directory .
```

复跑使用Python标准库，无仓库路径或/tmp外部依赖，不重新实现bootstrap统计，也不调用远端。输出 PRELIMINARY-REVIEW.md 与 PRELIMINARY-SUMMARY.json；失败时返回非零。
