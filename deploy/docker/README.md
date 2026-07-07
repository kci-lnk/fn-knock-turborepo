# fn-knock Docker Deployment

`deploy/docker` 用来承载 `fn-knock` 的 Docker 本地测试和远端发布。

当前构建链路已经按“多阶段 + 持久化缓存”设计：

- `deps` 阶段只根据 workspace manifests 安装依赖
- `admin-view-builder`、`auth-view-builder`、`server-builder` 分别独立构建各自产物
- `runtime` 阶段只带最终运行文件
- 本地和远端发布统一通过 `docker buildx` 构建
- 脚本默认会自动创建并复用 `fn-knock-buildx` 这个 `docker-container` builder
- 默认持久化缓存目录为 `~/.cache/fn-knock-buildx`

## 目录说明

- `Dockerfile`：多阶段构建镜像，构建共享运行时产物并打包到最终镜像
- `compose.yaml`：本地 compose 主文件，包含 `build` 配置
- `compose.override.yaml`：本地调试覆盖项
- `compose.remote.yaml`：远端发布专用 compose 文件，只使用已加载镜像，不在远端构建
- `entrypoint.sh`：在单容器内启动 Go 网关和 Node 后端
- `.env.example`：本地与远端通用的环境变量模板

## 从 Docker Hub 直接安装并运行

如果你是镜像使用者，而不是要在这个仓库里二次开发，推荐直接拉取 Docker Hub 镜像运行，不需要本地构建。

### 1. 准备运行目录

```bash
mkdir -p /opt/fn-knock-docker
cd /opt/fn-knock-docker
```

### 2. 准备 `.env`

下面这份配置适合绝大多数场景，默认直接使用正式发布镜像：

```dotenv
FN_KNOCK_IMAGE=kcilnk/fn-knock:latest
TZ=Asia/Shanghai
ADMIN_VIEW_PORT=7991
BACKEND_PORT=7998
AUTH_PORT=7997
GO_BACKEND_PORT=7996
GO_REPROXY_PORT=7999
FN_KNOCK_DOCKER_IPV4_SUBNET=172.30.0.0/16
FN_KNOCK_DOCKER_IPV6_SUBNET=fd42:fb33:7f7a:100::/64
DOCKER_ADMIN_TRUSTED_PROXY_CIDRS=
DOCKER_DISCOVER_LAN_IP=
```

配置建议：

- `FN_KNOCK_IMAGE`：推荐默认使用 `:latest`，这样用户可以始终更新到最新版本；如果需要锁版本，也可以改成 `:1.4.3` 这样的固定 tag
- `ADMIN_VIEW_PORT`：管理后台入口端口，默认 `7991`
- `GO_REPROXY_PORT`：网关对外服务端口，默认 `7999`
- `FN_KNOCK_DOCKER_IPV4_SUBNET`：Docker bridge 的容器 IPv4 子网；如果同机网络冲突，可换成其它私网 CIDR
- `FN_KNOCK_DOCKER_IPV6_SUBNET`：Docker bridge 的容器 IPv6 子网，默认启用 ULA `/64`，用于保留外部 IPv6 来源地址
- `BACKEND_PORT` / `AUTH_PORT` / `GO_BACKEND_PORT`：容器内部组件端口，通常保持默认即可
- `DOCKER_ADMIN_TRUSTED_PROXY_CIDRS`：如果 `7991` 需要挂在可信反代后面，填反代出口 IP 或 CIDR
- `DOCKER_DISCOVER_LAN_IP`：仅第三方反代无法自动识别宿主机局域网地址时作为兜底

### 3. 准备 `docker-compose.yml`

```yaml
services:
  fn-knock:
    image: ${FN_KNOCK_IMAGE}
    restart: unless-stopped
    environment:
      TZ: ${TZ:-Asia/Shanghai}
      FN_KNOCK_RUNTIME_TARGET: docker
      REDIS_HOST: redis
      REDIS_PORT: 6379
      FN_KNOCK_DATA_DIR: /var/lib/fn-knock
      FN_KNOCK_GATEWAY_CONFIG_DIR: /usr/local/etc/fn-knock
      ADMIN_VIEW_PORT: ${ADMIN_VIEW_PORT:-7991}
      BACKEND_PORT: ${BACKEND_PORT:-7998}
      AUTH_PORT: ${AUTH_PORT:-7997}
      GO_BACKEND_PORT: ${GO_BACKEND_PORT:-7996}
      GO_REPROXY_PORT: ${GO_REPROXY_PORT:-7999}
      DOCKER_ADMIN_TRUSTED_PROXY_CIDRS: ${DOCKER_ADMIN_TRUSTED_PROXY_CIDRS:-}
      DOCKER_DISCOVER_LAN_IP: ${DOCKER_DISCOVER_LAN_IP:-}
      DDNS_HOST_IF_INET6_PATH: /host/proc/net/if_inet6
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
      - /proc/1/net:/host/proc/net:ro
    depends_on:
      redis:
        condition: service_healthy
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

  redis:
    image: redis:7-bookworm
    restart: unless-stopped
    environment:
      TZ: ${TZ:-Asia/Shanghai}
    command: ["redis-server", "--appendonly", "yes"]
    networks:
      - fn_knock_net
    volumes:
      - fn_knock_redis:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 20

volumes:
  fn_knock_data:
  fn_knock_gateway:
  fn_knock_redis:

networks:
  fn_knock_net:
    enable_ipv6: true
    ipam:
      config:
        - subnet: ${FN_KNOCK_DOCKER_IPV4_SUBNET:-172.30.0.0/16}
        - subnet: ${FN_KNOCK_DOCKER_IPV6_SUBNET:-fd42:fb33:7f7a:100::/64}
```

这份 compose 配置的要点：

- `fn-knock` 和 `redis` 都使用 `unless-stopped`，重启后会自动拉起
- 只对宿主机开放 `ADMIN_VIEW_PORT` 和 `GO_REPROXY_PORT`
- 默认启用容器 IPv6 网络，避免宿主机 IPv6 入口被 Docker 转接到容器 IPv4 后丢失真实来访 IP
- 只读挂载宿主机 `/proc/1/net`，让 Docker 内的 DDNS 可以直接选择宿主机公网 IPv6；如果设备没有 IPv6 地址，DDNS 会自动退回原有公网探测逻辑
- `fn_knock_data` 保存业务数据，`fn_knock_gateway` 保存网关配置，`fn_knock_redis` 保存 Redis 持久化数据
- `depends_on + healthcheck` 会让 `fn-knock` 等 Redis 就绪后再启动

### 4. 拉取并启动

```bash
docker compose pull
docker compose up -d
docker compose ps
docker compose logs -f fn-knock
```

### 5. 端口说明

- `7991`：管理后台入口。首次访问会进入 Docker 管理面板密码设置流程
- `7999`：网关 / 代理入口。最终用户访问代理服务时通常使用这个端口
- `7998`：Node 后端内部端口，不对宿主机暴露
- `7997`：认证前端内部端口，不对宿主机暴露
- `7996`：Go 后端内部端口，不对宿主机暴露
- `6379`：Redis 仅在 compose 内部使用，不对宿主机暴露

### 6. 请求来源 IP

Docker bridge 发布端口时，本机回环访问会经过 Docker 的本地转发层，因此 `curl http://127.0.0.1:7999` 这类探测日志可能显示 `172.30.0.1` 或其它 Docker 网关地址。局域网或公网 IPv4 从宿主机网卡进入时，通常会保留真实 TCP 来源；如果经过上游反代，真实访问者需要由可信反代写入 `X-Forwarded-For`、`X-Real-IP` 或云厂商真实 IP 头。

请求日志中的“客户端 IP”会优先展示由上游真实 IP 头推断出的访问者；“连接来源 IP”保留网关实际看到的 TCP 对端，便于排查 Docker 端口发布、本机探测和反代链路。

### 7. 首次配置与后续更新

首次运行后：

1. 打开 `http://<宿主机IP>:7991`
2. 设置并登录 Docker 管理面板密码
3. 在管理后台中继续配置代理规则、鉴权、证书、白名单等业务项
4. 对外提供服务时，让流量进入 `7999`

后续升级：

```bash
docker compose pull
docker compose up -d
```

如果你使用的是 `latest`，这个流程会直接更新到最新镜像；如果你要固定版本，修改 `.env` 里的 `FN_KNOCK_IMAGE=kcilnk/fn-knock:<version>` 后再执行同样的命令。

## 本地测试

### 准备环境

```bash
cp deploy/docker/.env.example deploy/docker/.env
```

默认变量：

```dotenv
FN_KNOCK_IMAGE=fn-knock:local
TZ=Asia/Shanghai
ADMIN_VIEW_PORT=7991
BACKEND_PORT=7998
AUTH_PORT=7997
GO_BACKEND_PORT=7996
GO_REPROXY_PORT=7999
FN_KNOCK_DOCKER_IPV4_SUBNET=172.30.0.0/16
FN_KNOCK_DOCKER_IPV6_SUBNET=fd42:fb33:7f7a:100::/64
DOCKER_ADMIN_TRUSTED_PROXY_CIDRS=
DOCKER_DISCOVER_LAN_IP=
```

### 常用命令

```bash
# 构建镜像
npm run fn-knock:docker:build

# 启动本地环境
npm run fn-knock:docker:up

# 查看日志
npm run fn-knock:docker:logs

# 忘记管理面板密码时重置
npm run fn-knock:docker:reset-panel-password

# 单台远端主机快速部署（只构建远端实际架构）
npm run fn-knock:docker:local-deploy-fast

# 发布到 Docker Hub（推 amd64 / arm64 / arm32 + manifest）
npm run fn-knock:docker:hub-publish

# 停止环境
npm run fn-knock:docker:down
```

启动后默认访问地址：

- 管理后台入口：`http://127.0.0.1:7991`
- 网关代理入口：`http://127.0.0.1:7999`

本地测试特点：

- 自动读取 `deploy/docker/.env`，不存在时回退到 `deploy/docker/.env.example`
- 容器默认时区为 `Asia/Shanghai`，可通过 `TZ` 环境变量覆盖
- 自动加载 `compose.override.yaml`
- 默认附加 `EXPOSE_RUNTIME_HMAC_SECRET=1`，便于本地调试
- 只对外开放 `ADMIN_VIEW_PORT` 和 `GO_REPROXY_PORT`，`BACKEND_PORT` 仅保留在容器内部
- 首次访问 `ADMIN_VIEW_PORT` 需要设置管理面板密码，后续访问需要先输入该密码
- 成功登录后，可在“系统设置 -> 面板”里直接修改管理面板密码
- `ADMIN_VIEW_PORT` 默认只允许宿主机本地、局域网或 VPN 等内网来源访问，公网直连会直接收到拒绝页面
- 如果需要让外部用户经反向代理访问 `ADMIN_VIEW_PORT`，请把反代节点的出口 IP / CIDR 写入 `DOCKER_ADMIN_TRUSTED_PROXY_CIDRS`；这样仍会拒绝公网直连，但会放行来自可信反代的请求
- 可信反代需要继续透传 `X-Forwarded-For` 或 `X-Real-IP`；`go-reauth-proxy` 当前转发逻辑已经会附带这些头
- 如果你是通过 `go-reauth-proxy` 反代 `ADMIN_VIEW_PORT`，`一键发现本地服务` 会自动识别 Docker 宿主机对应的局域网 IPv4
- `DOCKER_DISCOVER_LAN_IP` 仅作为第三方反代场景下的兜底覆盖项；正常走 `go-reauth-proxy` 时不需要手工填写
- `ADMIN_VIEW_PORT` 会把通过认证的请求内部代理到 `BACKEND_PORT`
- 构建通过 `docker buildx` 执行，并将缓存写入 `~/.cache/fn-knock-buildx/<arch>`
- 如果当前活动 builder 是 `docker` driver，脚本会自动切到托管的 `fn-knock-buildx`

### 忘记管理面板密码

如果你是在开发仓库里操作本地 compose 环境，可以直接执行：

```bash
npm run fn-knock:docker:reset-panel-password
```

如果是最终客户机上的 Docker 主机，先登录到主机：

```bash
ssh root@<docker-host>
```

然后执行推荐命令：

```bash
cd /opt/fn-knock-docker && docker compose exec -T fn-knock fn-knock-reset-panel-password
```

如果只知道容器已经在跑 Docker，但不确定 compose 目录，也可以直接执行：

```bash
docker exec -it "$(docker ps --filter label=com.docker.compose.service=fn-knock --format '{{.Names}}' | head -n 1)" fn-knock-reset-panel-password
```

当前 `root@192.168.31.135` 上我已实际确认可用的命令就是：

```bash
cd /opt/fn-knock-docker && docker compose exec -T fn-knock fn-knock-reset-panel-password
```

这个命令只会清除：

- Docker 管理面板密码
- 管理面板登录会话
- 密码输错后的退避状态

不会删除业务配置、反代规则、证书、白名单、日志目录或数据卷内容。执行完成后，下次访问 `ADMIN_VIEW_PORT` 会重新进入“首次设置密码”流程。

### 本地前端调试 Docker 模式

如果当前本地运行的是普通开发环境，但需要调试 Docker 模式下的管理面板密码页、Docker 专属文案或能力限制提示，可以直接在浏览器 DevTools 中设置前端调试标记：

```js
localStorage.setItem("fn_knock:debug:docker-mode", "1");
localStorage.setItem("fn_knock:debug:docker-admin-stage", "setup"); // setup | login | authenticated
location.reload();
```

调试标记说明：

- `fn_knock:debug:docker-mode=1`
  开启前端 Docker 模式调试覆盖
- `fn_knock:debug:docker-admin-stage=setup`
  模拟首次进入，需要先设置管理面板密码
- `fn_knock:debug:docker-admin-stage=login`
  模拟密码已设置，但当前还未登录
- `fn_knock:debug:docker-admin-stage=authenticated`
  模拟已经通过管理面板密码验证

行为说明：

- 该调试方式只覆盖 `apps/server-admin-view` 的前端判断，不会修改 Node 后端的真实 `deployment_target`
- 开启后，前端会按 Docker 模式展示对应的能力限制，例如禁用直连模式、宿主机防火墙能力、Smart Connect 等
- 当阶段为 `setup` 时，前端提交成功的密码会自动保存到 `fn_knock:debug:docker-admin-password`
- 当阶段为 `login` 时，前端会使用 `fn_knock:debug:docker-admin-password` 作为本地校验密码
- 如果没有保存过调试密码，`login` 会自动回退到 `setup`

清理调试标记：

```js
localStorage.removeItem("fn_knock:debug:docker-mode");
localStorage.removeItem("fn_knock:debug:docker-admin-stage");
localStorage.removeItem("fn_knock:debug:docker-admin-password");
location.reload();
```

## 发布新版本到远端 Docker

### 默认目标

发布命令默认将镜像发布到：

- SSH 主机：`root@192.168.31.135`
- 远端部署目录：`/opt/fn-knock-docker`

### 发布命令

单台远端主机推荐使用快速部署命令：

```bash
npm run fn-knock:docker:local-deploy-fast
```

快速部署脚本会自动完成以下事情：

1. SSH 检测远端主机架构
2. 只使用 `docker buildx build` 构建远端实际运行的架构镜像
3. 生成对应架构的镜像 tag
4. 用 `docker save | ssh ... docker load` 把这一套镜像传到远端
5. 上传 `compose.remote.yaml` 和远端 `.env`
6. 远端执行 `docker compose up -d --remove-orphans --force-recreate`
7. 等待健康检查通过

如果需要一次性把三套架构镜像都同步到远端 Docker，仍然可以使用完整部署命令：

```bash
npm run fn-knock:docker:local-deploy
```

完整部署脚本会自动完成以下事情：

1. SSH 检测远端主机架构
2. 同时使用 `docker buildx build` 本地构建 `linux/amd64`、`linux/arm64` 和 `linux/arm/v7`
3. 生成一组镜像 tag
4. 用 `docker save | ssh ... docker load` 把三套镜像都传到远端
5. 上传 `compose.remote.yaml` 和远端 `.env`
6. 远端根据主机架构自动选择对应 tag 启动
7. 远端执行 `docker compose up -d --remove-orphans --force-recreate`
8. 等待健康检查通过

构建缓存说明：

- `deps` 阶段只要 `package.json` / workspace `package.json` 不变，就能直接复用依赖层
- 三个构建阶段相互独立，修改后端代码时不会强制重建两个前端阶段
- Docker 内不再依赖 `assemble-runtime.sh` 的聚合构建路径，而是直接构建各自工作区产物
- Node 构建阶段会在构建机原生平台执行，避免前端和服务端 JS 产物在非本机架构仿真环境里重复编译
- `local-deploy-fast` 只会使用远端实际架构对应的 `~/.cache/fn-knock-buildx/<arch>` 缓存
- `local-deploy` 仍会分别复用 `~/.cache/fn-knock-buildx/amd64`、`arm64` 和 `arm32` 下的 buildx 缓存
- 如果依赖声明变化、Dockerfile 变化、或源码真实影响产物，仍然会触发对应层重建，这是正常行为

### 镜像 tag 规则

如果没有手工指定 `FN_KNOCK_DOCKER_IMAGE_TAG`，脚本会从根目录 `version.json` 读取 `version`，先生成基础 tag：

```text
<version>-<YYYYMMDDHHMMSS>
```

然后自动产出三套镜像：

```text
fn-knock:<base-tag>-amd64
fn-knock:<base-tag>-arm64
fn-knock:<base-tag>-arm32
```

例如：

```text
fn-knock:1.4.1-20260409094530-amd64
fn-knock:1.4.1-20260409094530-arm64
fn-knock:1.4.1-20260409094530-arm32
```

推荐发布新版本时先更新根目录 `version.json`，再执行部署命令。快速部署只会产出远端架构对应的 tag；完整部署会产出三套架构 tag。

如果希望手工指定基础 tag：

```bash
FN_KNOCK_DOCKER_IMAGE_TAG=1.4.2 npm run fn-knock:docker:local-deploy-fast
```

如果远端是 `amd64`，快速部署实际发布镜像会变成：

```text
fn-knock:1.4.2-amd64
```

完整部署实际发布镜像会变成：

```text
fn-knock:1.4.2-amd64
fn-knock:1.4.2-arm64
fn-knock:1.4.2-arm32
```

当前默认远端 `root@192.168.31.135` 已确认是 `x86_64`，因此实际运行的是 `-amd64` 镜像，但 `-arm64` 和 `-arm32` 也会同步上传到远端 Docker。

### 发布后排查

```bash
# 查看远端容器状态
npm run fn-knock:docker:remote-ps

# 查看远端日志
npm run fn-knock:docker:remote-logs
```

## 发布到 Docker Hub

### 登录与仓库名

发布前先确保本机已经登录 Docker Hub：

```bash
docker login
```

`fn-knock:docker:hub-publish` 会直接推送目标镜像仓库，所以需要显式指定：

```bash
FN_KNOCK_DOCKER_IMAGE_REPO=kcilnk/fn-knock
```

### 默认 tag 规则

如果没有手工指定 `FN_KNOCK_DOCKER_IMAGE_TAG`，Docker Hub 发布会直接沿用项目当前版本号：

```text
version.json -> version
```

例如当前版本为 `1.4.3` 时，发布命令会推送：

```text
kcilnk/fn-knock:1.4.3-amd64
kcilnk/fn-knock:1.4.3-arm64
kcilnk/fn-knock:1.4.3-arm32
kcilnk/fn-knock:1.4.3
kcilnk/fn-knock:latest
```

其中：

- `:1.4.3-amd64`、`:1.4.3-arm64` 和 `:1.4.3-arm32` 是显式架构 tag
- `:1.4.3` 是多架构 manifest tag，`docker pull` 时会自动选择合适的平台
- `:latest` 也会同步更新为同一组多架构镜像，方便用户始终拉取最新版

### 发布命令

最常见的发布方式：

```bash
FN_KNOCK_DOCKER_IMAGE_REPO=kcilnk/fn-knock \
npm run fn-knock:docker:hub-publish
```

如果要手工覆盖版本号：

```bash
FN_KNOCK_DOCKER_IMAGE_REPO=kcilnk/fn-knock \
FN_KNOCK_DOCKER_IMAGE_TAG=1.4.4 \
npm run fn-knock:docker:hub-publish
```

### 发布流程

`fn-knock:docker:hub-publish` 会自动完成：

1. 使用现有 `docker buildx` builder 分别构建 `linux/amd64`、`linux/arm64` 和 `linux/arm/v7`
2. 将三个架构镜像直接推送到 Docker Hub
3. 创建版本号对应的多架构 manifest tag
4. 同步更新 `latest` 多架构 manifest tag
5. 校验 manifest 内同时包含 `linux/amd64`、`linux/arm64` 和 `linux/arm/v7`

## 可配置环境变量

### 本地构建/测试

- `FN_KNOCK_DOCKER_ENV_FILE`：指定 env 文件路径
- `FN_KNOCK_DOCKER_IMAGE`：覆盖本地构建镜像名
- `TZ`：容器时区，默认 `Asia/Shanghai`
- `FN_KNOCK_DOCKER_LOCAL_ARCH`：覆盖本地构建架构
- `FN_KNOCK_DOCKER_CACHE_DIR`：指定 buildx 本地缓存目录
- `FN_KNOCK_DOCKER_BUILDER`：指定 buildx builder 名称
- `FN_KNOCK_DOCKER_MANAGED_BUILDER`：托管 builder 名称，默认 `fn-knock-buildx`
- `FN_KNOCK_DOCKER_HTTP_PROXY` / `FN_KNOCK_DOCKER_HTTPS_PROXY` / `FN_KNOCK_DOCKER_ALL_PROXY`：覆盖 Docker 构建代理；未设置时会回退到标准 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`
- `FN_KNOCK_DOCKER_NO_PROXY`：覆盖 Docker 构建时的 `NO_PROXY`
- `FN_KNOCK_DOCKER_PROXY_HOST_ALIAS`：容器访问宿主机代理的地址别名，默认 `host.docker.internal`

### 发布/远端部署

- `FN_KNOCK_DOCKER_IMAGE_REPO`：镜像仓库名，默认 `fn-knock`
- `FN_KNOCK_DOCKER_IMAGE_TAG`：手工指定发布基础 tag；快速远端部署时会扩展为远端实际架构，完整远端部署时会扩展为 `-amd64`、`-arm64` 和 `-arm32`
- `FN_KNOCK_DOCKER_REMOTE_HOST`：远端 SSH 地址，默认 `root@192.168.31.135`
- `FN_KNOCK_DOCKER_REMOTE_DIR`：远端 compose 落地目录，默认 `/opt/fn-knock-docker`
- `FN_KNOCK_DOCKER_WAIT_TIMEOUT`：远端健康检查等待秒数，默认 `180`

示例：

```bash
FN_KNOCK_DOCKER_REMOTE_HOST=root@192.168.31.136 \
FN_KNOCK_DOCKER_REMOTE_DIR=/srv/fn-knock \
FN_KNOCK_DOCKER_IMAGE_TAG=1.4.2 \
npm run fn-knock:docker:local-deploy-fast
```

如果你的本机代理监听在 `127.0.0.1:7890`，可以直接这样执行：

```bash
HTTP_PROXY=http://127.0.0.1:7890 \
HTTPS_PROXY=http://127.0.0.1:7890 \
npm run fn-knock:docker:local-deploy
```

脚本会在 `docker buildx` builder 和实际构建阶段自动注入代理，并把 `127.0.0.1` / `localhost` 改写成容器内可访问的 `host.docker.internal`。如果你手工指定了 `FN_KNOCK_DOCKER_BUILDER`，则需要确保那个 builder 自己已经带上相同的代理环境。

## 运行时限制

Docker 模式下后端会自动识别 `FN_KNOCK_RUNTIME_TARGET=docker`，并收敛能力边界：

- 禁用 `run_type=0`
- 禁用宿主机防火墙管理
- 禁用 Smart Connect / dnsmasq 相关能力
- 禁用应用内 FPK 更新

管理端在 Docker 中会保留 `127.0.0.1:${BACKEND_PORT}` 作为容器内部后端接口，并在 `0.0.0.0:${ADMIN_VIEW_PORT}` 提供一个只允许内网访问的管理入口。用户浏览器访问 `ADMIN_VIEW_PORT` 后，请求会先完成管理面板密码验证，再由该入口内部代理到 `BACKEND_PORT`。
