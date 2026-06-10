export const dockerAdminPanelResetCommands = {
  ssh: "ssh root@<docker-host>",
  compose:
    "cd /opt/fn-knock-docker && docker compose exec -T fn-knock fn-knock-reset-panel-password",
  dockerExec:
    "docker exec -it \"$(docker ps --filter label=com.docker.compose.service=fn-knock --format '{{.Names}}' | head -n 1)\" fn-knock-reset-panel-password",
} as const;
