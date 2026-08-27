import { beforeEach, describe, expect, it, vi } from "vitest";

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("../src/lib/api/client", () => ({
  apiClient: { get },
}));

import {
  configCoreApi,
  PANEL_BOOTSTRAP_TIMEOUT_MS,
} from "../src/lib/api/config-core-api";

describe("panel bootstrap request", () => {
  beforeEach(() => {
    get.mockReset();
  });

  it("bounds the request that gates the first visible application state", async () => {
    get.mockResolvedValue({ data: { data: { enabled: false } } });

    await expect(configCoreApi.getDockerAdminBootstrap()).resolves.toEqual({
      enabled: false,
    });
    expect(get).toHaveBeenCalledWith("/panel/bootstrap", {
      timeout: PANEL_BOOTSTRAP_TIMEOUT_MS,
    });
    expect(PANEL_BOOTSTRAP_TIMEOUT_MS).toBe(15_000);
  });
});
