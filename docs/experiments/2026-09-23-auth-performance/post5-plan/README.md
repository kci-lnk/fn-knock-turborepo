# Final-source post-main experiment bundle

本目录是本次实验专用草稿；准备不构建、不上传、不启动服务或负载。最初任务目录名保留 `post4-prep`，但最终制品已改为 **candidate5**：Rust `5fddf896eaf9b1cf3eb300c08315320498c943b8`、Go `4d15fa32764e26df58b16930d0e2503a90880002`，Go 须使用与基线一致的 `-s -w -X ...Version=2.4.15 -X ...Commit=...`。v4 的 Go link flags 不匹配，不能作为 v5 验收输入。远端工具引用 root 冻结的 `scripts5`；不修改 `scripts4` 或复制第二套运行工具。

## 阶段与参数

除特别注明，全部 TOTP、accounts/sessions=1000、concurrency=16、clients=2、Tokio workers=2、cache TTL=0、ordinary grants=100；正常组不设置 bridge capacity。每个 trial 的进程和数据库由现有 harness 新建。

| group | 对照及路由 | 对数与时长 | 使用范围 |
| --- | --- | --- | --- |
| cache | 原始 base→candidate5；session_hit；TTL=1 | 6 对，warm20s/load60s | plan cache 六对回归与 RSS 验收 |
| renewal | 原始 base→candidate5；grant_renewal；ordinary grants=1、每 phase 64 个独立 token | 6 对，warm20s/request-start deadline60s | plan renewal 六对回归与 RSS；报告有限批次实际 elapsed，非持续60秒吞吐 |
| grants | 原始 base→candidate5；grant_hit；grants=1000/10000；**c1** | 每规模1对，warm5s/load15s | 短测归因；CLI clients=2，但c1时现有harness只启动1个有效worker |
| params | candidate5-z-reader1→reader2/4，或→compiler s/2/3且reader1；bootstrap/session_hit/grant_hit/auto_ip_hit | 每种参数/路由1对，warm5s/load15s | 单因子探索，不交叉 reader×compiler，不改默认、不作收益验收 |
| profile | 原始 base→candidate5 的 grant_hit/session_hit/auto_ip_hit；reader1→2/4 的 grant_hit/auto_ip_hit | 1对，warm20s/load30s、idle5s；合计14trial | 单独采集，不能合并进性能样本；无额外compiler profile |
| soak | candidate5-z-reader1 单角色；auto_ip_hit | warm20s/load1800s | 首尾5分钟/5分钟分桶 RSS、FD、线程和可用goroutine，非无泄漏证明 |
| recovery | 原始 base→candidate5；auto_ip_hit；双方显式capacity32 | 1对warm1s/load3s，加既有c64 burst2s、c1恢复窗口10s | 恢复正确性，非生产默认容量或性能比较 |

主矩阵仍使用独立 `formal-v5-main/results.json`，以 plan 的6目标/3保护路由完整验收。cache/renewal 调用原有统计库上的 `check-auth-performance-plan.mjs`；短测/profile/recovery 仅调用不带六对门槛的有效性检查。不会把探索的1对解释成改善成立。所有 group 都串行执行，首个 harness/语义/验收失败立即停止并保留原始证据；排查后显式选择另一个新 batch，不自动重试或删除失败样本。

## 本机离线生成

待 root 最后冻结 baseline5/candidate5 及 scripts5 后运行。示例中的本地制品/快照位置须替换为 root 确认的实际路径；生成器只读输入和本地制品，只写全新的输出目录：

```sh
python3 /tmp/fn-knock-auth-post4-prep/prepare-post.py \
  --base-config /tmp/fn-knock-auth-variants5-20260923.json \
  --candidate-name candidate5 \
  --candidate-go-manifest /tmp/fn-knock-auth-go-matched5-20260923/candidate/build.json \
  --candidate-go-sha256 603188b385a10abb7bf1d6a6f8798c2234e0beafcc692e5abda0e7112a48eab6 \
  --baseline-rust-artifact /tmp/fn-knock-auth-baseline5-20260923/compiler-z/server-admin-rs \
  --baseline-go-artifact /tmp/fn-knock-auth-go-matched5-20260923/baseline/go-reauth-proxy \
  --tools-dir /tmp/fn-knock-auth-tools5-20260923 \
  --remote-tools /tmp/fn-knock-auth-performance-20260923/scripts5 \
  --remote-bundle /tmp/fn-knock-auth-performance-20260923/post5-plan \
  --remote-compiler-prefix /tmp/fn-knock-auth-performance-20260923/compiler5 \
  --main-results /tmp/fn-knock-auth-performance-20260923/formal-v5-main/results.json \
  --output /tmp/fn-knock-auth-post4-prep/bundle-post5-ready
```

生成器检查固定源码、z/fat/CGU1、Rust各档相同锁与工具链、成功构建记录、所有制品真实SHA256及 root 确认的匹配link-flags Go哈希。正式双方四个二进制必须同时匹配 `EXPERIMENT_PLAN.json` 的 `artifact_sha256` pins，scripts5文件也须匹配root的快照manifest；baseline重建后需先更新冻结计划，不能先运行再发现身份不符。参数档共用新的 Go gateway；Rust四档仍嵌入同一个4d Go源码身份，无需因同源码Go链接参数变化而重编Rust。

输出 `configs/`、`jobs.json`、`jobs.sh` 和 `bundle-manifest.json`。引用的scripts5工具文件、配置、计划、runner全部有哈希；scripts5须包含 auth-reader 专用profile统计与离线plan checker，不能原地修改已冻结bundle或工具快照。manifest保留本地来源与远端目标路径、源码、编译记录、工具版本/文件哈希。验证的是这些记录与制品内容一致，不声称从二进制SHA自行证明源码身份。

## 待人工选择时执行的上传与运行命令

**以下命令没有执行。** 在 main 停止且没有其他远端负载时，root 可先审阅 bundle。仅上传新计划目录，禁止覆盖 `scripts4/scripts5`。所有制品必须已完成且生成器检查通过：

```sh
ssh root@192.168.31.98 'test ! -e /tmp/fn-knock-auth-performance-20260923/post5-plan && mkdir /tmp/fn-knock-auth-performance-20260923/post5-plan'
scp -r /tmp/fn-knock-auth-post4-prep/bundle-post5-ready/. \
  root@192.168.31.98:/tmp/fn-knock-auth-performance-20260923/post5-plan/
```

baseline5（或 root 冻结配置指定的 baseline）、candidate5、compiler5-s/2/3、assets/control 和 scripts5 应已由主实验上传；runner preflight 会重新校验产品二进制和工具内容，不替换它们。

```sh
# 在目标Linux执行；没有action时只展示计划。
post=/tmp/fn-knock-auth-performance-20260923/post5-plan/run-post.sh
bash "$post" show
bash "$post" preflight
bash "$post" check-main > /tmp/formal-v5-main.acceptance.json

# 一个新结果目录内顺序运行全部组；main验收失败也会停止。
bash "$post" run all /tmp/fn-knock-auth-performance-20260923/post5-all-UNIQUE

# 或逐组显式执行，每次仍使用新的结果batch。
bash "$post" run cache /tmp/fn-knock-auth-performance-20260923/post5-cache-UNIQUE
bash "$post" run renewal /tmp/fn-knock-auth-performance-20260923/post5-renewal-UNIQUE
# 后续groups: grants params profile soak recovery
```

启动前检查全局互斥锁和现存harness进程；有其他实验则拒绝开始，绝不终止它。runner收到TERM/INT时只转发到自己当前的harness并等其清理合成进程；不使用广泛kill。shell不自行ssh。进程环境清除继承的bridge/reader覆盖及allocator/Go调优变量，具体实验值由variant设置；main仍须按原manifest单独校验。不同工具/制品/结果批次不混合。

## 结果解释与收集

每case保留 `.run.log`、原始 `results.json`/`manifest.json`/各trial `result.json`，并产生 `.report.md`。cache/renewal另有 `.acceptance.json`；其余双角色用 `.validity.json`，没有通过性能门槛的含义。soak输出 `.soak-summary.json` 并检查真实30分钟及现有请求/采样质量；资源变化仍需解释。`completed-cases.txt`只在单case全部步骤成功后追加。不要上传原始数据库、密钥或配置日志；沿用原collector的JSON白名单及脱敏策略。

profile 的 auth-reader execution 为 `kind=sqlite_auth_read`，admission为 `kind=sqlite_admission,label=sqlite_auth_read`；两者不含checkpoint gate等待。runtime-health storage队列仍主要反映primary，并非2/4个reader的汇总。参数短测若有收益线索，必须另行预声明六对及所有回归/RSS门槛后才可改变默认；本脚本不自动挑选“最佳值”。
