import type { useMaintenanceBackupWorkflow } from "./useMaintenanceBackupWorkflow";
import type { useMaintenanceClearData } from "./useMaintenanceClearData";

export type MaintenanceBackupController = ReturnType<
  typeof useMaintenanceBackupWorkflow
>;
export type MaintenanceClearDataController = ReturnType<
  typeof useMaintenanceClearData
>;
