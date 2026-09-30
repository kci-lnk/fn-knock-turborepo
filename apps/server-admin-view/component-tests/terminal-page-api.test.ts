import { afterEach, describe, expect, it, vi } from "vitest";

const client = vi.hoisted(() => ({ basePath: "/api/admin" }));
vi.mock("@/lib/api/client", () => ({
  apiClient: {},
  get adminApiBasePath() {
    return client.basePath;
  },
}));
import { TerminalAPI } from "@/lib/api/terminal";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("terminal page departure API", () => {
  for (const prefix of [
    "/api/admin",
    "/panel/api/admin",
    "/webman/3rdparty/fn-knock/api.cgi/api/admin",
  ]) {
    it(`keeps credentials and POST requests alive under ${prefix}`, async () => {
      client.basePath = prefix;
      const fetch = vi.fn().mockResolvedValue({ ok: true });
      vi.stubGlobal("fetch", fetch);
      await TerminalAPI.releasePage("page-1");
      expect(fetch).toHaveBeenCalledWith(
        `${prefix}/terminal/pages/page-1/release`,
        {
          method: "POST",
          credentials: "include",
          keepalive: true,
        },
      );
    });
  }
  it("reports rejected departure requests so callers can defer to server expiry", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false }));
    await expect(TerminalAPI.releasePage("page-1")).rejects.toThrow(
      "Terminal page release failed",
    );
  });
});
