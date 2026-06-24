import { dockerAdminPanelManager } from "./lib/docker-admin-panel";
import { tDefault } from "./lib/i18n";
import { redis, waitForRedis } from "./lib/redis";

const printHelp = () => {
  console.log(tDefault("server.dockerAdminPanel.resetHelp"));
};

const main = async () => {
  const args = new Set(process.argv.slice(2));
  if (args.has("-h") || args.has("--help")) {
    printHelp();
    return;
  }

  await waitForRedis();

  const summary = await dockerAdminPanelManager.resetPasswordState();

  console.log(tDefault("server.dockerAdminPanel.resetCleared"));
  console.log(
    JSON.stringify(
      {
        passwordCleared: summary.password_cleared,
        sessionsCleared: summary.sessions_cleared,
        loginFailuresCleared: summary.login_failures_cleared,
      },
      null,
      2,
    ),
  );
  console.log(tDefault("server.dockerAdminPanel.resetNextVisit"));
};

main()
  .catch((error) => {
    console.error(
      tDefault("server.dockerAdminPanel.resetFailed"),
      error instanceof Error ? error.message : error,
    );
    process.exitCode = 1;
  })
  .finally(async () => {
    try {
      await redis.quit();
    } catch {
      // ignore redis shutdown errors during process exit
    }
  });
