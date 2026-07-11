export const dockerAdminPanelResetCommands = {
  ssh: "ssh root@<docker-host>",
  compose:
    "cd /opt/fn-knock-docker && docker compose exec -T fn-knock fn-knock-reset-panel-password",
  dockerExec:
    "docker exec -it \"$(docker ps --filter label=com.docker.compose.service=fn-knock --format '{{.Names}}' | head -n 1)\" fn-knock-reset-panel-password",
} as const;

export const openWrtAdminPanelResetCommands = {
  ssh: "ssh root@<openwrt-host>",
  reset: "fn-knock-reset-panel-password",
} as const;

export const windowsAdminPanelResetCommands = {
  reset:
    '& "$env:ProgramFiles\\FnKnock\\current\\fn-knock-service.exe" reset-panel-password',
} as const;
