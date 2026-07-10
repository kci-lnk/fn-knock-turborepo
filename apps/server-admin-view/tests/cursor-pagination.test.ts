/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { ref } from "vue";

import { useCursorPagination } from "../src/composables/useCursorPagination";

describe("useCursorPagination", () => {
  it("tracks older pages and restores cursor history", () => {
    const loading = ref(false);
    const pagination = useCursorPagination({ loading });

    pagination.nextCursor.value = "cursor-1";
    assert.equal(pagination.loadOlder(), true);
    assert.equal(pagination.currentCursor.value, "cursor-1");
    assert.deepEqual(pagination.cursorHistory.value, [""]);

    pagination.nextCursor.value = "cursor-2";
    assert.equal(pagination.loadOlder(), true);
    assert.equal(pagination.currentCursor.value, "cursor-2");
    assert.deepEqual(pagination.cursorHistory.value, ["", "cursor-1"]);

    assert.equal(pagination.loadNewer(), true);
    assert.equal(pagination.currentCursor.value, "cursor-1");
    assert.deepEqual(pagination.cursorHistory.value, [""]);

    assert.equal(pagination.loadFirst(), true);
    assert.equal(pagination.currentCursor.value, "");
    assert.equal(pagination.nextCursor.value, "");
    assert.deepEqual(pagination.cursorHistory.value, []);
  });

  it("does not mutate pagination while a page load is pending", () => {
    const loading = ref(true);
    const pagination = useCursorPagination({ loading });
    pagination.currentCursor.value = "cursor-1";
    pagination.nextCursor.value = "cursor-2";
    pagination.cursorHistory.value = [""];

    assert.equal(pagination.loadOlder(), false);
    assert.equal(pagination.loadNewer(), false);
    assert.equal(pagination.loadFirst(), false);
    assert.equal(pagination.currentCursor.value, "cursor-1");
    assert.equal(pagination.nextCursor.value, "cursor-2");
    assert.deepEqual(pagination.cursorHistory.value, [""]);
  });
});
