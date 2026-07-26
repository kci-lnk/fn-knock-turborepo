# fn-knock Docker 部署与维护

本目录包含 fn-knock 的镜像定义、本地 Compose 环境和维护者发布脚本。

- 普通用户：使用已发布镜像，推荐在 Linux 主机上采用 HOST 网络部署。
- 项目维护者：使用仓库内的 Compose 和 `scripts/fn-knock-docker.sh` 构建、测试及发布镜像。

面向最终用户的最新部署说明以
[Docker Compose 部署文档](https://docs.fnknock.cn/quick-start/docker-deployment)
为准。

## 使用已发布镜像部署

### 环境要求

需要 Linux 主机或基于 Linux 的 NAS，并已安装 Docker Engine 和 Docker
Compose 插件：

```bash
docker version
docker compose version
```

下文的推荐配置使用 HOST 网络，让容器直接访问宿主机网络。

### 选择镜像源

| 镜像源     | 镜像地址                                | 适用网络                                  |
| ---------- | --------------------------------------- | ----------------------------------------- |
| 官方镜像源 | `hub.fnknock.cn/kcilnk/fn-knock:latest` | 中国大陆网络；`latest` 每 30 分钟同步一次 |
| Docker Hub | `kcilnk/fn-knock:latest`                | 可稳定访问 Docker Hub 的网络              |

下文默认使用官方镜像源。切换镜像源时，只需同时修改拉取命令和
`.env` 中的 `FN_KNOCK_IMAGE`。如需锁定版本，可将 `latest` 改为已发布的固定
标签。

### 选择网络模式

| 网络模式  | 推荐程度   | 说明                                                                  |
| --------- | ---------- | --------------------------------------------------------------------- |
| HOST 网络 | 推荐、默认 | 容器直接使用宿主机网络，可识别真实网卡与 IPv6                         |
| 桥接网络  | 可选       | 使用隔离的双栈 bridge 并映射端口，但 DDNS 可能找不到宿主机网卡或 IPv6 |

需要使用 DDNS“从网卡获取”或依赖宿主机 IPv6 时，请使用 HOST 网络。桥接网络
适合更看重网络隔离、并且不依赖宿主机网卡识别的部署。

### 一键安装

在目标 Linux 主机的 root 终端中粘贴下面整段脚本。脚本会检查 Docker
环境，在 `/opt/fn-knock-docker` 写入 HOST 网络配置并启动 fn-knock。

如果目标目录中已经存在 `.env` 或 `docker-compose.yml`，脚本会停止，不会
覆盖原配置。

```bash
sh <<'FN_KNOCK_INSTALL'
set -eu

if [ "$(id -u)" -ne 0 ]; then
  echo "Please run this installer in a root terminal." >&2
  exit 1
fi

command -v docker >/dev/null 2>&1 || { echo "Docker is not installed." >&2; exit 1; }
docker compose version >/dev/null 2>&1 || { echo "Docker Compose is not available." >&2; exit 1; }

install_dir=/opt/fn-knock-docker
mkdir -p "$install_dir"
cd "$install_dir"

if [ -e .env ] || [ -e docker-compose.yml ]; then
  echo "Existing .env or docker-compose.yml found; installation stopped." >&2
  exit 1
fi

cat > .env <<'FN_KNOCK_ENV'
FN_KNOCK_IMAGE=hub.fnknock.cn/kcilnk/fn-knock:latest
TZ=Asia/Shanghai
ADMIN_VIEW_PORT=7991
BACKEND_PORT=7998
AUTH_PORT=7997
GO_BACKEND_PORT=7996
GO_REPROXY_PORT=7999
DOCKER_ADMIN_TRUSTED_PROXY_CIDRS=
DOCKER_DISCOVER_LAN_IP=
FN_KNOCK_ENV

cat > docker-compose.yml <<'FN_KNOCK_COMPOSE'
services:
  fn-knock:
    image: ${FN_KNOCK_IMAGE}
    restart: unless-stopped
    network_mode: host
    environment:
      TZ: ${TZ:-Asia/Shanghai}
      FN_KNOCK_RUNTIME_TARGET: docker
      FN_KNOCK_DATA_DIR: /var/lib/fn-knock
      FN_KNOCK_GATEWAY_CONFIG_DIR: /usr/local/etc/fn-knock
      ADMIN_VIEW_PORT: ${ADMIN_VIEW_PORT:-7991}
      BACKEND_PORT: ${BACKEND_PORT:-7998}
      AUTH_PORT: ${AUTH_PORT:-7997}
      GO_BACKEND_PORT: ${GO_BACKEND_PORT:-7996}
      GO_REPROXY_PORT: ${GO_REPROXY_PORT:-7999}
      DOCKER_ADMIN_TRUSTED_PROXY_CIDRS: ${DOCKER_ADMIN_TRUSTED_PROXY_CIDRS:-}
      DOCKER_DISCOVER_LAN_IP: ${DOCKER_DISCOVER_LAN_IP:-}
      ADMIN_VIEW_HOST: 0.0.0.0
      BACKEND_HOST: 127.0.0.1
    volumes:
      - fn_knock_data:/var/lib/fn-knock
      - fn_knock_gateway:/usr/local/etc/fn-knock
    healthcheck:
      test:
        [
          "CMD-SHELL",
          "curl -fsS http://127.0.0.1:${ADMIN_VIEW_PORT:-7991}/api/admin/healthz || exit 1",
        ]
      interval: 10s
      timeout: 5s
      retries: 12
      start_period: 20s

volumes:
  fn_knock_data:
  fn_knock_gateway:
FN_KNOCK_COMPOSE

docker compose pull
docker compose up -d
docker compose ps
FN_KNOCK_INSTALL
```

### 手动安装

#### 1. 准备目录并拉取镜像

```bash
mkdir -p /opt/fn-knock-docker
cd /opt/fn-knock-docker
docker pull hub.fnknock.cn/kcilnk/fn-knock:latest
```

#### 2. 创建 `.env`

将以下内容保存为 `/opt/fn-knock-docker/.env`：

```dotenv
FN_KNOCK_IMAGE=hub.fnknock.cn/kcilnk/fn-knock:latest
TZ=Asia/Shanghai
ADMIN_VIEW_PORT=7991
BACKEND_PORT=7998
AUTH_PORT=7997
GO_BACKEND_PORT=7996
GO_REPROXY_PORT=7999
DOCKER_ADMIN_TRUSTED_PROXY_CIDRS=
DOCKER_DISCOVER_LAN_IP=
```

关键配置：

| 配置项                             | 默认值                                  | 说明                                                      |
| ---------------------------------- | --------------------------------------- | --------------------------------------------------------- |
| `FN_KNOCK_IMAGE`                   | `hub.fnknock.cn/kcilnk/fn-knock:latest` | 可改为 Docker Hub 地址或固定版本标签                      |
| `ADMIN_VIEW_PORT`                  | `7991`                                  | 管理面板的宿主机端口                                      |
| `GO_REPROXY_PORT`                  | `7999`                                  | 网关入口的宿主机端口                                      |
| `DOCKER_ADMIN_TRUSTED_PROXY_CIDRS` | 留空                                    | 仅当 `7991` 位于可信反向代理后时，填写代理出口 IP 或 CIDR |
| `DOCKER_DISCOVER_LAN_IP`           | 留空                                    | 仅在第三方反代无法自动识别宿主机局域网地址时填写          |

`BACKEND_PORT`、`AUTH_PORT` 和 `GO_BACKEND_PORT` 是内部组件端口，通常保持默认值。

#### 3. 创建 `docker-compose.yml`

推荐的 HOST 网络配置只需要一个 `fn-knock` 容器：

```yaml
services:
  fn-knock:
    image: ${FN_KNOCK_IMAGE}
    restart: unless-stopped
    network_mode: host
    environment:
      TZ: ${TZ:-Asia/Shanghai}
      FN_KNOCK_RUNTIME_TARGET: docker
      FN_KNOCK_DATA_DIR: /var/lib/fn-knock
      FN_KNOCK_GATEWAY_CONFIG_DIR: /usr/local/etc/fn-knock
      ADMIN_VIEW_PORT: ${ADMIN_VIEW_PORT:-7991}
      BACKEND_PORT: ${BACKEND_PORT:-7998}
      AUTH_PORT: ${AUTH_PORT:-7997}
      GO_BACKEND_PORT: ${GO_BACKEND_PORT:-7996}
      GO_REPROXY_PORT: ${GO_REPROXY_PORT:-7999}
      DOCKER_ADMIN_TRUSTED_PROXY_CIDRS: ${DOCKER_ADMIN_TRUSTED_PROXY_CIDRS:-}
      DOCKER_DISCOVER_LAN_IP: ${DOCKER_DISCOVER_LAN_IP:-}
      ADMIN_VIEW_HOST: 0.0.0.0
      BACKEND_HOST: 127.0.0.1
    volumes:
      - fn_knock_data:/var/lib/fn-knock
      - fn_knock_gateway:/usr/local/etc/fn-knock
    healthcheck:
      test:
        [
          "CMD-SHELL",
          "curl -fsS http://127.0.0.1:${ADMIN_VIEW_PORT:-7991}/api/admin/healthz || exit 1",
        ]
      interval: 10s
      timeout: 5s
      retries: 12
      start_period: 20s

volumes:
  fn_knock_data:
  fn_knock_gateway:
```

HOST 网络下不需要声明 `ports` 或自定义 bridge，容器直接监听宿主机端口。

#### 4. 启动并检查

```bash
docker compose up -d
docker compose ps
docker compose logs -f fn-knock
```

最后一条命令会持续显示日志，可按 `Ctrl+C` 退出。

### 可选：使用桥接网络

桥接网络可能导致 DDNS 找不到宿主机网卡或 IPv6。确认业务不依赖“从网卡
获取”后，在 `.env` 中增加：

```dotenv
FN_KNOCK_DOCKER_IPV4_SUBNET=172.30.0.0/16
FN_KNOCK_DOCKER_IPV6_SUBNET=fd42:fb33:7f7a:100::/64
```

并将 `docker-compose.yml` 改为：

```yaml
services:
  fn-knock:
    image: ${FN_KNOCK_IMAGE}
    restart: unless-stopped
    environment:
      TZ: ${TZ:-Asia/Shanghai}
      FN_KNOCK_RUNTIME_TARGET: docker
      FN_KNOCK_DATA_DIR: /var/lib/fn-knock
      FN_KNOCK_GATEWAY_CONFIG_DIR: /usr/local/etc/fn-knock
      ADMIN_VIEW_PORT: ${ADMIN_VIEW_PORT:-7991}
      BACKEND_PORT: ${BACKEND_PORT:-7998}
      AUTH_PORT: ${AUTH_PORT:-7997}
      GO_BACKEND_PORT: ${GO_BACKEND_PORT:-7996}
      GO_REPROXY_PORT: ${GO_REPROXY_PORT:-7999}
      DOCKER_ADMIN_TRUSTED_PROXY_CIDRS: ${DOCKER_ADMIN_TRUSTED_PROXY_CIDRS:-}
      DOCKER_DISCOVER_LAN_IP: ${DOCKER_DISCOVER_LAN_IP:-}
      ADMIN_VIEW_HOST: 0.0.0.0
      BACKEND_HOST: 127.0.0.1
    ports:
      - "${ADMIN_VIEW_PORT:-7991}:${ADMIN_VIEW_PORT:-7991}"
      - "${GO_REPROXY_PORT:-7999}:${GO_REPROXY_PORT:-7999}"
    networks:
      - fn_knock_net
    volumes:
      - fn_knock_data:/var/lib/fn-knock
      - fn_knock_gateway:/usr/local/etc/fn-knock
    healthcheck:
      test:
        [
          "CMD-SHELL",
          "curl -fsS http://127.0.0.1:${ADMIN_VIEW_PORT:-7991}/api/admin/healthz || exit 1",
        ]
      interval: 10s
      timeout: 5s
      retries: 12
      start_period: 20s

volumes:
  fn_knock_data:
  fn_knock_gateway:

networks:
  fn_knock_net:
    enable_ipv6: true
    ipam:
      config:
        - subnet: ${FN_KNOCK_DOCKER_IPV4_SUBNET:-172.30.0.0/16}
        - subnet: ${FN_KNOCK_DOCKER_IPV6_SUBNET:-fd42:fb33:7f7a:100::/64}
```

如果子网与同机现有网络冲突，请将 `FN_KNOCK_DOCKER_IPV4_SUBNET` 换成其他
私网 CIDR；IPv6 子网应保持为 ULA `/64`。

### 首次访问与端口

默认端口如下：

| 端口   | 服务            | 暴露范围                | 用途                               |
| ------ | --------------- | ----------------------- | ---------------------------------- |
| `7991` | 管理后台入口    | HOST 网络或 bridge 映射 | 首次访问时设置 Docker 管理面板密码 |
| `7999` | 网关 / 代理入口 | HOST 网络或 bridge 映射 | 外部用户访问代理服务时使用         |
| `7998` | Rust 后端       | 宿主机回环或容器内部    | 通常保持默认值                     |
| `7997` | 认证前端        | 宿主机回环或容器内部    | 通常保持默认值                     |
| `7996` | Go 网关管理     | 宿主机回环或容器内部    | 通常保持默认值                     |

当前镜像使用内置 SQLite 存储，只需运行一个 `fn-knock` 容器，不需要 Redis。

1. 打开 `http://<宿主机IP>:7991`，设置管理面板密码并登录。
2. 在管理后台完成反向代理、子域名、证书和鉴权配置。
3. 让外部业务流量访问 `7999` 对应的网关入口。
4. 如果 `7991` 位于可信反向代理后，在 `.env` 设置
   `DOCKER_ADMIN_TRUSTED_PROXY_CIDRS`。
5. 仅当第三方反代无法自动识别宿主机局域网地址时，才设置
   `DOCKER_DISCOVER_LAN_IP`。

### 数据卷

| 数据卷             | 容器路径                  | 内容                                       |
| ------------------ | ------------------------- | ------------------------------------------ |
| `fn_knock_data`    | `/var/lib/fn-knock`       | 密钥、备份以及 FRP、Cloudflared 等运行数据 |
| `fn_knock_gateway` | `/usr/local/etc/fn-knock` | 网关配置和默认 SQLite 数据库               |

执行 `docker compose pull` 或重新创建容器不会删除具名卷。删除卷前应先备份，
具体操作请参考
[备份、恢复与数据清理](https://docs.fnknock.cn/guide/backup-and-restore)。

### 更新镜像

保持 `.env` 中使用 `latest`，然后重新拉取并创建容器：

```bash
cd /opt/fn-knock-docker
docker compose pull
docker compose up -d
docker compose ps
```

持久化卷不会被删除。固定版本标签只会重新拉取同一标签；如需升级，请先修改
`FN_KNOCK_IMAGE`。

### 使用 Watchtower 自动更新

当 `.env` 使用 `latest` 时，可以在同一台 Docker 主机上运行 Watchtower：

```bash
docker run -d \
  --name watchtower \
  --restart unless-stopped \
  -v /var/run/docker.sock:/var/run/docker.sock \
  nickfedor/watchtower --cleanup
```

默认情况下，Watchtower 每 24 小时检查所有运行中的容器。发现同一镜像标签的
摘要变化后，它会拉取镜像并按原配置重新创建容器，更新期间会有短暂重启。
固定版本标签不会自动跨标签升级。

`--cleanup` 只清理已被替换的旧镜像，不会删除 fn-knock 的具名数据卷。

```bash
docker ps --filter name=watchtower
docker logs watchtower
```

> 此基础配置会管理主机上的所有运行中容器，并通过 Docker Socket 获得管理
> Docker 的高权限。仅在可信主机上使用；启用前先备份 fn-knock 数据。如果
> 其他容器不应自动更新，请按
> [Watchtower 官方文档](https://watchtower.nickfedor.com/)
> 使用容器名称、标签或 Scope 限制更新范围。

### 重设管理面板密码

忘记密码时，在运行 Docker 的主机上执行：

```bash
cd /opt/fn-knock-docker
docker compose exec -T fn-knock fn-knock-reset-panel-password
```

再次访问 `7991` 时会重新进入首次设置密码流程。该命令只清除管理面板密码、
登录会话和密码输错后的退避状态，不会删除业务配置、反代规则、证书、白名单、
日志或数据卷。

## 仓库内开发与发布

以下内容面向 fn-knock 项目维护者，不是最终用户安装已发布镜像所必需的步骤。

### 目录说明

| 文件                      | 用途                                                          |
| ------------------------- | ------------------------------------------------------------- |
| `Dockerfile`              | 组装预构建的前端、Go 网关和 Rust 后端，生成 Alpine 运行时镜像 |
| `compose.yaml`            | 本地构建与测试的 Compose 主文件                               |
| `compose.override.yaml`   | 本地调试覆盖项，启用运行时 HMAC secret 输出                   |
| `compose.remote.yaml`     | SSH 测试部署使用的 Compose 模板，只运行已加载镜像             |
| `.env.example`            | 本地和 SSH 测试部署的默认环境变量                             |
| `entrypoint.sh`           | 在单容器内启动 Go 网关和 Rust 后端                            |
| `reset-panel-password.sh` | 镜像内的管理面板密码重置入口                                  |
| `rust-backends/`          | 各目标架构的 Rust 后端二进制                                  |

仓库内 `compose.yaml` 和 `compose.remote.yaml` 使用 bridge 网络，便于开发和
测试；普通用户部署仍应优先使用上文的 HOST 网络配置。

### 本地构建与测试

在仓库根目录执行：

```bash
cp deploy/docker/.env.example deploy/docker/.env
```

常用命令：

```bash
# 构建当前主机架构的本地镜像
npm run fn-knock:docker:build

# 构建并以前台方式启动本地 Compose 环境
npm run fn-knock:docker:up

# 查看容器日志
npm run fn-knock:docker:logs

# 重设本地管理面板密码
npm run fn-knock:docker:reset-panel-password

# 停止本地环境
npm run fn-knock:docker:down
```

默认访问地址：

- 管理后台：`http://127.0.0.1:7991`
- 网关入口：`http://127.0.0.1:7999`

脚本会优先读取 `deploy/docker/.env`，不存在时回退到
`deploy/docker/.env.example`。本地环境会自动加载
`compose.override.yaml`，并把 buildx 缓存写入
`~/.cache/fn-knock-buildx/<arch>`。

默认托管 builder 名称为 `fn-knock-buildx`。如果它不存在，脚本会以
`docker-container` driver 创建；如构建代理配置发生变化，脚本会重新创建
托管 builder 以应用新配置。

### SSH 测试部署

快速部署只构建远端主机实际使用的架构：

```bash
FN_KNOCK_DOCKER_REMOTE_HOST=root@<docker-host> \
npm run fn-knock:docker:local-deploy-fast
```

完整部署会构建并传输 `amd64`、`arm64` 和 `arm32` 三套镜像：

```bash
FN_KNOCK_DOCKER_REMOTE_HOST=root@<docker-host> \
npm run fn-knock:docker:local-deploy
```

两种流程都会：

1. 通过 SSH 检测远端架构。
2. 使用 buildx 构建镜像。
3. 通过 `docker save | ssh ... docker load` 传输镜像。
4. 上传 `compose.remote.yaml` 和环境文件。
5. 重新创建远端 Compose 服务并等待健康检查通过。

默认远端目录为 `/opt/fn-knock-docker`。未指定
`FN_KNOCK_DOCKER_IMAGE_TAG` 时，远端测试镜像的基础标签为：

```text
<version.json 中的 version>-<YYYYMMDDHHMMSS>
```

随后追加 `-amd64`、`-arm64` 或 `-arm32`。可以显式覆盖标签：

```bash
FN_KNOCK_DOCKER_REMOTE_HOST=root@<docker-host> \
FN_KNOCK_DOCKER_IMAGE_TAG=<version-or-tag> \
npm run fn-knock:docker:local-deploy-fast
```

远端排查和密码重置：

```bash
npm run fn-knock:docker:remote-ps
npm run fn-knock:docker:remote-logs
npm run fn-knock:docker:remote-reset-panel-password
```

这些命令同样读取 `FN_KNOCK_DOCKER_REMOTE_HOST` 和
`FN_KNOCK_DOCKER_REMOTE_DIR`。

### 发布到 Docker Hub

先登录 Docker Hub：

```bash
docker login
```

然后从仓库根目录发布：

```bash
FN_KNOCK_DOCKER_IMAGE_REPO=kcilnk/fn-knock \
npm run fn-knock:docker:hub-publish
```

默认版本标签取自 `version.json`。发布流程会：

1. 构建并推送 `linux/amd64`、`linux/arm64` 和 `linux/arm/v7` 镜像。
2. 分别创建 `-amd64`、`-arm64` 和 `-arm32` 架构标签。
3. 创建版本号对应的多架构 manifest。
4. 将 `latest` 指向同一 manifest。
5. 校验 manifest 包含全部三个目标平台。

如需覆盖版本标签：

```bash
FN_KNOCK_DOCKER_IMAGE_REPO=kcilnk/fn-knock \
FN_KNOCK_DOCKER_IMAGE_TAG=<version-or-tag> \
npm run fn-knock:docker:hub-publish
```

### 构建与发布环境变量

| 环境变量                               | 默认值或用途                                                             |
| -------------------------------------- | ------------------------------------------------------------------------ |
| `FN_KNOCK_DOCKER_ENV_FILE`             | 指定环境文件；默认 `deploy/docker/.env`，不存在时使用 `.env.example`     |
| `FN_KNOCK_DOCKER_IMAGE`                | 覆盖本地构建镜像名                                                       |
| `FN_KNOCK_DOCKER_IMAGE_REPO`           | SSH 部署默认 `fn-knock`；发布 Docker Hub 时必须显式提供 `namespace/repo` |
| `FN_KNOCK_DOCKER_IMAGE_TAG`            | 覆盖发布基础标签                                                         |
| `FN_KNOCK_DOCKER_LOCAL_ARCH`           | 覆盖本地构建架构；默认使用当前主机架构                                   |
| `FN_KNOCK_DOCKER_CACHE_DIR`            | buildx 缓存根目录，默认 `~/.cache/fn-knock-buildx`                       |
| `FN_KNOCK_DOCKER_BUILDER`              | 使用指定的现有 buildx builder                                            |
| `FN_KNOCK_DOCKER_MANAGED_BUILDER`      | 托管 builder 名称，默认 `fn-knock-buildx`                                |
| `FN_KNOCK_DOCKER_BUILD_RUST_BACKENDS`  | `auto`、`0` 或 `1`，默认 `auto`                                          |
| `FN_KNOCK_DOCKER_RUST_BACKEND_BIN_DIR` | 指定预构建 Rust 后端目录                                                 |
| `FN_KNOCK_USE_PREPARED_ARTIFACTS`      | 默认 `1`，使用共享的预构建产物                                           |
| `FN_KNOCK_ARTIFACTS_DIR`               | 共享产物目录，默认 `dist/fn-knock-artifacts`                             |
| `FN_KNOCK_DOCKER_REMOTE_HOST`          | 远端 SSH 地址                                                            |
| `FN_KNOCK_DOCKER_REMOTE_DIR`           | 远端 Compose 目录，默认 `/opt/fn-knock-docker`                           |
| `FN_KNOCK_DOCKER_SERVICE_NAME`         | Compose 服务名，默认 `fn-knock`                                          |
| `FN_KNOCK_DOCKER_WAIT_TIMEOUT`         | 远端健康检查超时秒数，默认 `180`                                         |

构建代理变量：

- `FN_KNOCK_DOCKER_HTTP_PROXY`
- `FN_KNOCK_DOCKER_HTTPS_PROXY`
- `FN_KNOCK_DOCKER_ALL_PROXY`
- `FN_KNOCK_DOCKER_NO_PROXY`
- `FN_KNOCK_DOCKER_PROXY_HOST_ALIAS`

未设置专用代理变量时，脚本会回退到标准的 `HTTP_PROXY`、`HTTPS_PROXY`、
`ALL_PROXY` 和 `NO_PROXY`。本机代理地址中的 `127.0.0.1`、`localhost` 或
`::1` 会被改写为容器可访问的 `host.docker.internal`；可以通过
`FN_KNOCK_DOCKER_PROXY_HOST_ALIAS` 覆盖该别名。

### Docker 运行时限制

容器会设置 `FN_KNOCK_RUNTIME_TARGET=docker`，并收敛以下能力：

- 禁用 `run_type=0`。
- 禁用宿主机防火墙管理。
- 禁用 Smart Connect 和 dnsmasq 相关能力。
- 禁用应用内 FPK 更新。

管理入口监听 `ADMIN_VIEW_PORT`，并在完成管理面板密码验证后，把请求代理到
仅监听 `127.0.0.1:${BACKEND_PORT}` 的内部 Rust 后端。
