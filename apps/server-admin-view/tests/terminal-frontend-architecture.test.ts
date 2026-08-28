import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { describe, it } from "node:test";
import enMessages from "../../../packages/i18n/src/messages/scopes/admin/en";
import jaJPMessages from "../../../packages/i18n/src/messages/scopes/admin/ja-JP";
import koKRMessages from "../../../packages/i18n/src/messages/scopes/admin/ko-KR";
import zhCNMessages from "../../../packages/i18n/src/messages/scopes/admin/zh-CN";
import zhHantMessages from "../../../packages/i18n/src/messages/scopes/admin/zh-Hant";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("SSH terminal frontend architecture", () => {
  it("keeps page orchestration split by responsibility", () => {
    const page = readSource("../src/views/web-terminal/useWebTerminalPage.ts");
    for (const composable of [
      "useTerminalTargets",
      "useTerminalTargetEditor",
      "useTerminalSessions",
      "useTerminalAttachment",
      "useTerminalEmulator",
      "useTerminalViewport",
    ]) {
      assert.match(page, new RegExp(`${composable}\\(`, "u"), composable);
    }
    assert.doesNotMatch(page, /terminal_feature|installTmux|getStatus/u);
  });

  it("resets the emulator before applying a reset snapshot", () => {
    const attachment = readSource(
      "../src/views/web-terminal/useTerminalAttachment.ts",
    );
    const reset = attachment.indexOf("if (event.reset) onReset()");
    const output = attachment.indexOf("onOutput(event)", reset);
    assert.ok(reset >= 0);
    assert.ok(output > reset);
  });

  it("keeps pre-running SSH phases read-only", () => {
    const attachment = readSource(
      "../src/views/web-terminal/useTerminalAttachment.ts",
    );
    assert.match(
      attachment,
      /const livePhase = ref<TerminalSessionPhase \| null>\(null\)/,
    );
    assert.match(attachment, /livePhase\.value === "running"/);
    assert.match(attachment, /livePhase\.value = event\.phase/);
    assert.match(attachment, /livePhase\.value = session\.phase/);
  });

  it("reconciles session selection after targets and sessions bootstrap", () => {
    const page = readSource("../src/views/web-terminal/useWebTerminalPage.ts");
    assert.match(
      page,
      /await Promise\.all\([\s\S]*sessionsController\.reconcileSelection\(\)/,
    );
  });

  it("reconciles session selection in the same target-click batch", () => {
    const page = readSource("../src/views/web-terminal/useWebTerminalPage.ts");
    assert.match(
      page,
      /targetsController\.selectTarget\(targetId\);\s*sessionsController\.reconcileSelection\(\);\s*viewport\.closeTargetDrawer\(\);/u,
    );
  });

  it("rebuilds attachments without terminating the SSH session", () => {
    const attachment = readSource(
      "../src/views/web-terminal/useTerminalAttachment.ts",
    );
    assert.match(attachment, /createAttachment/u);
    assert.match(attachment, /detachAttachment/u);
    assert.doesNotMatch(attachment, /deleteSession/u);
    assert.match(attachment, /generation: current\.generation/u);
    assert.match(attachment, /sequence: inputSequence/u);
    assert.match(attachment, /revision: resizeRevision/u);
    assert.match(attachment, /terminalPhase/u);
    assert.equal(
      attachment.match(
        /await TerminalAPI\.sendInput\(current\.id, payload, signal\)/gu,
      )?.length,
      2,
    );
    assert.doesNotMatch(
      attachment,
      /let reconnectAttempt = 0;\s*setReadyState\(record\)/u,
    );
  });

  it("provides desktop target navigation and a mobile drawer", () => {
    const navigation = readSource(
      "../src/views/web-terminal/TerminalTargetsNavigation.vue",
    );
    assert.match(navigation, /md:block/u);
    assert.match(navigation, /<Sheet/u);
    assert.match(navigation, /side="left"/u);
    assert.match(navigation, /TerminalTargetList/u);
    assert.match(navigation, /drawer/u);
    assert.match(navigation, /selected-session-id/u);
    const targetList = readSource(
      "../src/views/web-terminal/TerminalTargetList.vue",
    );
    assert.match(targetList, /sessionsForTarget/u);
    assert.match(targetList, /emit\(['"]selectSession['"], session\.id\)/u);
    assert.match(targetList, /drawer \? 'pr-14'/u);
    assert.doesNotMatch(targetList, /ChevronRight|KeyRound|targetReady/u);
  });

  it("keeps mobile terminal actions on one row", () => {
    const toolbar = readSource(
      "../src/views/web-terminal/TerminalSessionToolbar.vue",
    );
    assert.match(toolbar, /flex min-w-0 flex-nowrap items-center/u);
    assert.match(toolbar, /min-w-0 flex-1 max-w-\[210px\] md:hidden/u);
    assert.match(toolbar, /flex shrink-0 items-center gap-1/u);
    assert.doesNotMatch(toolbar, /flex flex-wrap items-center/u);
  });

  it("uses a high-contrast selected state for mobile modifier keys", () => {
    const toolbar = readSource(
      "../src/views/web-terminal/TerminalMobileToolbar.vue",
    );
    assert.match(toolbar, /:aria-pressed="armedModifier === modifier"/u);
    assert.match(
      toolbar,
      /border-primary bg-primary text-primary-foreground hover:bg-primary\/90 hover:text-primary-foreground/u,
    );
  });

  it("keeps the terminal mount alive while targets and sessions switch", () => {
    const workspace = readSource(
      "../src/views/web-terminal/TerminalWorkspacePanel.vue",
    );
    assert.match(
      workspace,
      /v-show="!isBooting && selectedTarget && selectedSession"[\s\S]*:ref="setTerminalMountElement"/u,
    );
    assert.doesNotMatch(
      workspace,
      /<template v-else>[\s\S]*:ref="setTerminalMountElement"/u,
    );
  });

  it("guards editor requests against stale async results", () => {
    const editor = readSource(
      "../src/views/web-terminal/useTerminalTargetEditor.ts",
    );
    assert.match(editor, /AbortController/u);
    assert.match(editor, /operationGeneration/u);
    assert.match(editor, /endpoint !== endpointKey\.value/u);
    assert.match(editor, /connectionGeneration/u);
    assert.match(editor, /testedConnectionGeneration/u);
    assert.match(editor, /verificationToken/u);
    assert.match(editor, /forceConfirmationToken/u);
    assert.doesNotMatch(editor, /JSON\.stringify/u);
    assert.match(editor, /pendingHostKey/u);
    assert.match(editor, /confirmHostKey/u);
    assert.match(editor, /TerminalAPI\.probeHostKey/u);
    assert.match(editor, /clearSensitiveDraft/u);
    assert.match(editor, /draft\.secret = ""/u);
    assert.match(editor, /draft\.passphrase = ""/u);
    assert.doesNotMatch(editor, /console\.(?:log|error).*credential/iu);
  });

  it("only forces target updates that invalidate active sessions", () => {
    const page = readSource("../src/views/web-terminal/useWebTerminalPage.ts");
    const editor = readSource(
      "../src/views/web-terminal/useTerminalTargetEditor.ts",
    );
    const dialog = readSource(
      "../src/views/web-terminal/TerminalTargetEditorDialog.vue",
    );
    assert.match(editor, /requiresSessionTermination/u);
    assert.match(editor, /forceConfirmationRequired/u);
    assert.match(editor, /credentialMutation\(\)\.action !== "keep"/u);
    assert.match(editor, /passphraseMutation\(\)\.action !== "keep"/u);
    assert.match(
      editor,
      /force &&[\s\S]*forceConfirmationRequired\.value &&[\s\S]*requiresSessionTermination\.value/u,
    );
    assert.match(dialog, /requiresTerminationConfirmation/u);
    assert.match(dialog, /!terminateActiveSessions/u);
    assert.match(dialog, /editor\.testable\.value/u);
    assert.doesNotMatch(dialog, /editor\.probeHostKey|probeHostKey/u);
    assert.match(dialog, /max-h-\[calc\(100dvh-2rem\)\]/u);
    assert.match(dialog, /overflow-x-hidden/u);
    assert.match(dialog, /field-sizing-fixed/u);
    assert.match(dialog, /\[overflow-wrap:anywhere\]/u);
    const deletion = readSource(
      "../src/views/web-terminal/useTerminalTargetDeletion.ts",
    );
    assert.match(page, /useTerminalTargetDeletion/u);
    assert.match(
      deletion,
      /requestDeleteTarget\(target\.id, target\.revision, false, undefined\)/u,
    );
    assert.match(
      deletion,
      /requestDeleteTarget\([\s\S]*target\.id,[\s\S]*target\.revision,[\s\S]*true,[\s\S]*confirmationToken/u,
    );
  });

  it("defines every static terminal translation key in every locale", () => {
    const directory = new URL("../src/views/web-terminal/", import.meta.url);
    const source = [
      readSource("../src/views/WebTerminal.vue"),
      ...readdirSync(directory, { recursive: true })
        .filter(
          (entry): entry is string =>
            typeof entry === "string" && /\.(?:ts|vue)$/u.test(entry),
        )
        .map((entry) => readFileSync(new URL(entry, directory), "utf8")),
    ].join("\n");
    const keys = new Set(
      [...source.matchAll(/\bt\(\s*["']([^"']+)["']/gu)].map(
        (match) => match[1],
      ),
    );
    const resolveMessage = (messages: unknown, key: string) =>
      key.split(".").reduce<unknown>((value, segment) => {
        if (!value || typeof value !== "object") return undefined;
        return (value as Record<string, unknown>)[segment];
      }, messages);

    for (const [locale, messages] of Object.entries({
      "zh-CN": zhCNMessages,
      "zh-Hant": zhHantMessages,
      en: enMessages,
      "ja-JP": jaJPMessages,
      "ko-KR": koKRMessages,
    })) {
      const missing = [...keys].filter(
        (key) => resolveMessage(messages, key) === undefined,
      );
      assert.deepEqual(missing, [], `${locale}: ${missing.join(", ")}`);
    }
  });

  it("models private-key credentials and passphrases independently", () => {
    const api = readSource("../src/lib/api/terminal.ts");
    const editor = readSource(
      "../src/views/web-terminal/useTerminalTargetEditor.ts",
    );
    assert.match(
      api,
      /TerminalPassphraseMutation = TerminalSchemas\["PassphraseMutation"\]/u,
    );
    assert.match(api, /TerminalTargetCreateInput = TerminalSchemas/u);
    assert.match(api, /TerminalTargetUpdateInput = TerminalSchemas/u);
    assert.match(editor, /clearPassphrase/u);
    assert.match(editor, /passphrase: passphraseMutation\(\)/u);
  });

  it("cancels stale mutations and detaches externally removed sessions", () => {
    const targets = readSource(
      "../src/views/web-terminal/useTerminalTargets.ts",
    );
    const sessions = readSource(
      "../src/views/web-terminal/useTerminalSessions.ts",
    );
    const page = readSource("../src/views/web-terminal/useWebTerminalPage.ts");
    const refresh = readSource(
      "../src/views/web-terminal/useTerminalSessionRefresh.ts",
    );
    for (const source of [targets, sessions]) {
      assert.match(source, /newOperationSlot/u);
      assert.match(source, /AbortController/u);
      assert.match(source, /cancelMutations/u);
    }
    assert.match(page, /refreshSessions/u);
    assert.match(refresh, /await detach\(\)/u);
    assert.match(refresh, /attachmentSessionId\.value/u);
    assert.match(refresh, /sessionExists\(previousSessionId\)/u);
  });

  it("localizes every SSH connection phase instead of rendering raw enums", () => {
    const presentation = readSource(
      "../src/views/web-terminal/useTerminalPresentation.ts",
    );
    assert.match(
      presentation,
      /admin\.webTerminal\.sessionPhase\.\$\{phase\}/u,
    );
    assert.doesNotMatch(presentation, /` · \$\{session\.phase\}`/u);
    assert.match(presentation, /phase !== "running"/u);
  });
});
