import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { effectScope } from "vue";
import { useConfirmationDialog } from "../../../packages/admin-shared/src/composables/useConfirmationDialog";

describe("project confirmation dialog", () => {
  it("resolves explicit confirmation and dismissal", async () => {
    const scope = effectScope();
    const dialog = scope.run(useConfirmationDialog);
    assert.ok(dialog);

    const confirmed = dialog.requestConfirmation({
      description: "Save changes?",
      title: "Confirm",
    });
    assert.equal(dialog.confirmationDialogOpen.value, true);
    dialog.confirmPendingAction();
    assert.equal(await confirmed, true);
    assert.equal(dialog.confirmationDialogOpen.value, false);

    const dismissed = dialog.requestConfirmation({
      description: "Leave this page?",
      title: "Confirm",
    });
    dialog.handleConfirmationDialogOpenChange(false);
    assert.equal(await dismissed, false);

    scope.stop();
  });

  it("cancels an older request when a new confirmation replaces it", async () => {
    const scope = effectScope();
    const dialog = scope.run(useConfirmationDialog);
    assert.ok(dialog);

    const first = dialog.requestConfirmation({
      description: "First action",
      title: "Confirm",
    });
    const second = dialog.requestConfirmation({
      description: "Second action",
      title: "Confirm",
    });

    assert.equal(await first, false);
    assert.equal(
      dialog.confirmationDialogOptions.value.description,
      "Second action",
    );

    scope.stop();
    assert.equal(await second, false);
  });
});
