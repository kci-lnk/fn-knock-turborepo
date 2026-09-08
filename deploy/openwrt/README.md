# OpenWrt 手动网关放行

LuCI 的敲门 Knock 页面提供“防火墙来源区域”和“放行防火墙”按钮。
先保存并应用网关端口，再选择来源区域并点击按钮。默认使用已有规则的区域，
否则选择 `wan`；没有 `wan` 时必须手动选择。仅放行网关 TCP 端口（默认 7999），
覆盖 IPv4/IPv6 到路由器自身的入站流量，不开放管理后台或内部 API 端口。

规则保存到 `firewall.fn_knock_gateway_ingress`，由 OpenWrt 在重启时正常加载。
更改网关端口或来源区域后，需要再次点击；重复点击更新同一规则并重新尝试重载。
页面状态说明的是已保存配置，不能证明外网连通性。
撤销放行请在 LuCI 原生防火墙页面删除 `Allow-FnKnock-Gateway` 规则。
服务安装、升级、启动、停止、重启、配置保存和卸载均不会自动增删该规则。

## 实现约定

- `/usr/libexec/fn-knock-firewall status` 只读取已提交配置，返回 JSON，包括
  `state`（`absent`、`configured`、`conflict`）、`port`、`source_zone`、
  `configured_port` 和 `zones`。
- `allow <zone> <expected-port>` 是唯一写入入口。端口从已提交的 `fn-knock`
  配置读取，并与页面传入端口核对，防止使用过期页面放行错误端口。
- 使用 BusyBox ash、ubus/rpcd、jshn 和系统防火墙 init 脚本；不增加 fw3/fw4、
  iptables 或 nftables 软件包依赖，不直接运行包过滤命令。
- helper 的工具和库路径固定，不接受环境变量替换执行路径；故障注入仅在临时测试副本中进行。
- 每次操作使用独立 rpcd UCI 会话，不提交或丢弃其他 CLI/LuCI 会话的暂存修改。
  发现默认 CLI 暂存的防火墙修改时拒绝执行，防止系统重载顺带激活这些修改。
  LuCI ACL 只开放状态查询和指定 helper 的 `allow` 命令，没有通用 firewall 写权限。
- 不覆盖同名非本应用规则，也不覆盖包含额外字段、列表或限制的已有规则。
- 写入或提交失败时不重载；`reload_failed` 表示规则已保存，但重载失败，可以重试。
- 使用原子目录锁，最多等待约 4 秒。进程正常退出会释放锁。若被 SIGKILL 后持续报告
  `busy`，管理员确认没有正在执行的 helper 后，可删除空目录
  `/var/run/fn-knock-firewall.lock.d`；不要删除正在使用的锁。

## 验证

```sh
bash scripts/tests/test-openwrt-firewall.sh
FN_KNOCK_TEST_OPENWRT_IMAGE=openwrt/rootfs:x86-64-21.02.7 bash scripts/tests/test-openwrt-firewall.sh
bash scripts/tests/test-openwrt-runtime-contract.sh
bash scripts/tests/test-openwrt-tar-compat.sh
```

防火墙测试需要 Node.js 和 Docker，默认使用 OpenWrt 23.05.5 x86-64 容器，
仅挂载只读源码，不操作宿主机防火墙。测试使用真实 BusyBox、jshn、UCI、ubus、rpcd，
覆盖 ACL、隔离事务、重复和并发点击、区域切换、规则冲突、非法输入与失败重试。
重载通过测试替身检查调用与故障处理，fw4 另执行规则生成验证。

OpenWrt 21.02.7 的同一套 helper 测试可验证 fw3 年代的运行环境。
容器没有内核 iptables filter 表时，明确跳过 fw3 规则生成。
实际 fw3/fw4 防火墙重载、IPv4/IPv6 外部连通性和整机重启后的持久性仍需在路由器上验收。
