import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import type { TerminalDestination } from "../src/lib/api/terminal";
import TerminalTargetList from "../src/views/web-terminal/TerminalTargetList.vue";

const targets: TerminalDestination[] = [
  {
    kind: "ssh",
    id: "ssh-1",
    name: "Production",
    host: "server.example.com",
    port: 22,
    username: "deploy",
    authMethod: "password",
    trustedHostKey: null,
    credentialConfigured: true,
    passphraseConfigured: false,
    revision: 1,
    lastVerifiedAt: null,
    createdAt: "2026-08-28T00:00:00Z",
    updatedAt: "2026-08-28T00:00:00Z",
  },
  {
    kind: "local",
    id: "local",
    name: "Local",
    targetId: "local",
    supported: true,
    enabled: true,
    ready: true,
    executionIdentity: "fn-knock",
    privileged: false,
    shell: "/bin/sh",
    workingDirectory: "/tmp",
    blockedReason: null,
    revision: 0,
  },
];

const mountList = (collapsed = false) =>
  mount(TerminalTargetList, {
    props: {
      collapsed,
      loading: false,
      selectedSessionId: "",
      selectedTargetId: "",
      sessions: [],
      targets,
    },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          missingWarn: false,
          fallbackWarn: false,
          messages: { en: {} },
        }),
      ],
    },
  });

describe("terminal target selection", () => {
  for (const pointerType of ["touch", "pen", "mouse"]) {
    for (const collapsed of [false, true]) {
      it(`selects immediately and repeatedly with ${pointerType}, collapsed=${collapsed}`, async () => {
        const wrapper = mountList(collapsed);
        try {
          const rows = wrapper.findAll("[data-terminal-target-row]");
          for (const index of [0, 1, 0]) {
            const button = rows[index]!.get("button");
            await button.trigger("pointerdown", { pointerType });
            await button.trigger("pointerup", { pointerType });
            await button.trigger("click");
          }
          expect(wrapper.emitted("select")).toEqual([
            ["ssh-1"],
            ["local"],
            ["ssh-1"],
          ]);
          expect(wrapper.emitted("edit")).toBeUndefined();
          expect(wrapper.emitted("configureLocal")).toBeUndefined();
          if (collapsed)
            expect(wrapper.find(".terminal-target-actions").exists()).toBe(
              false,
            );
        } finally {
          wrapper.unmount();
        }
      });
    }
  }

  it("keeps edit and local settings separate from target selection", async () => {
    const wrapper = mountList();
    try {
      const actions = wrapper.findAll(".terminal-target-actions button");
      for (const action of actions) {
        await action.trigger("pointerdown", { pointerType: "touch" });
        await action.trigger("pointerup", { pointerType: "touch" });
        await action.trigger("click");
      }
      expect(wrapper.emitted("edit")).toEqual([[targets[0]]]);
      expect(wrapper.emitted("configureLocal")).toEqual([[]]);
      expect(wrapper.emitted("select")).toBeUndefined();
    } finally {
      wrapper.unmount();
    }
  });
});
