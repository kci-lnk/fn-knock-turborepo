import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  automaticBackupAttemptCompleted,
  automaticBackupAttemptSucceeded,
  automaticBackupSourceIsAvailable,
  backupSourceMenuIsRequired,
  buildAutomaticBackupSelectionSummary,
  isAutomaticBackupConfigValid,
} from "../src/lib/automatic-backup";

describe("automatic backup settings", () => {
  it("validates the documented interval and retention boundaries", () => {
    assert.equal(isAutomaticBackupConfigValid(1, 1), true);
    assert.equal(isAutomaticBackupConfigValid(8760, 3650), true);
    assert.equal(isAutomaticBackupConfigValid(0, 7), false);
    assert.equal(isAutomaticBackupConfigValid(24, 3651), false);
    assert.equal(isAutomaticBackupConfigValid(1.5, 7), false);
  });

  it("offers automatic restore only when an automatic archive exists", () => {
    assert.equal(automaticBackupSourceIsAvailable(0), false);
    assert.equal(automaticBackupSourceIsAvailable(1), true);
  });

  it("uses a source menu for FNOS or automatic server-side backups", () => {
    assert.equal(backupSourceMenuIsRequired(false, 0), false);
    assert.equal(backupSourceMenuIsRequired(true, 0), true);
    assert.equal(backupSourceMenuIsRequired(false, 2), true);
  });

  it("keeps polling through unchanged attempts and detects the new result", () => {
    assert.equal(automaticBackupAttemptCompleted("old", null), false);
    assert.equal(automaticBackupAttemptCompleted("old", "old"), false);
    assert.equal(automaticBackupAttemptCompleted("old", "new"), true);
    assert.equal(automaticBackupAttemptSucceeded("old", "old"), false);
    assert.equal(automaticBackupAttemptSucceeded("old", "new"), true);
  });

  it("maps an automatic file into the restore selection summary", () => {
    assert.deepEqual(
      buildAutomaticBackupSelectionSummary(
        {
          name: "backup.knock",
          relativePath: "backup.knock",
          extension: ".knock",
          size: 2048,
          modifiedAt: "2026-07-24T00:00:00Z",
        },
        "2.0 KB",
        "Automatic backup",
      ),
      {
        name: "backup.knock",
        size: "2.0 KB",
        sourceLabel: "Automatic backup",
        location: "backup.knock",
      },
    );
  });
});
