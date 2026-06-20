export class MaintenanceBackupError extends Error {
  status: number;

  constructor(message: string, status = 500) {
    super(message);
    this.name = "MaintenanceBackupError";
    this.status = status;
  }
}
