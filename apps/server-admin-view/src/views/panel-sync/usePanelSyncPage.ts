import { usePanelSyncConnections } from "./usePanelSyncConnections";
import { usePanelSyncEditor } from "./usePanelSyncEditor";
import { usePanelSyncRun } from "./usePanelSyncRun";

export const usePanelSyncPage = () => {
  const connections = usePanelSyncConnections();
  const editor = usePanelSyncEditor(
    {
      create: connections.create,
      update: connections.update,
      verify: connections.verifySaved,
    },
    () => connections.connections.value.map((connection) => connection.name),
  );
  const run = usePanelSyncRun(connections.load);

  return { connections, editor, run };
};
