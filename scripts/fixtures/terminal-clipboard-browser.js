import { createApp, h, nextTick, onMounted, ref, computed } from "vue";
import { createI18n } from "vue-i18n";
import { useTerminalEmulator } from "/src/views/web-terminal/useTerminalEmulator.ts";
import { useTerminalInteractions } from "/src/views/web-terminal/useTerminalInteractions.ts";
import TerminalContextMenu from "/src/views/web-terminal/TerminalContextMenu.vue";
import TerminalCopyDialog from "/src/views/web-terminal/TerminalCopyDialog.vue";
import "/src/assets/index.css";

const i18n = createI18n({
  legacy: false,
  locale: "en",
  missingWarn: false,
  fallbackWarn: false,
});
createApp({
  setup() {
    const frame = ref(null);
    const selectedSession = ref({ id: "session-a", title: "test" });
    const activeAttachment = ref(null);
    const sent = [];
    const emulator = useTerminalEmulator({
      applyFontSize() {},
      canAcceptInput: () => true,
      compactViewport: ref(false),
      persistFontSize() {},
      queueInput: (data) => sent.push(data),
      queueRemoteResponse() {},
      scheduleResize() {},
      terminalFontSize: ref(14),
      terminalFrameRef: frame,
      translate: (key) => key,
    });
    const actions = useTerminalInteractions({
      activeAttachment,
      cancelRenameSession() {},
      emulator,
      inputQueue: {
        queueTerminalInput: (data) => sent.push(data),
        sendTerminalPayloadNow: async () => {},
      },
      isTerminalFullscreen: ref(false),
      renameSession: async () => selectedSession.value,
      selectedSession: computed(() => selectedSession.value),
      sessions: ref([]),
      setTerminalFullscreen: async () => {},
      translate: (key) => key,
    });
    let cursor = 0;
    const write = (text) =>
      emulator.applyOutputEvent({
        cursor: ++cursor,
        dataBase64: btoa(
          String.fromCharCode(...new TextEncoder().encode(text)),
        ),
        reset: false,
      });
    onMounted(async () => {
      actions.start();
      await emulator.ensureTerminalReady();
      emulator.clearTerminal();
      write("LOG-123 中文 👩‍💻 e\u0301");
      await nextTick();
      window.terminalTest = {
        emulator,
        actions,
        selectedSession,
        activeAttachment,
        sent,
        write,
      };
    });
    return () =>
      h("div", [
        h(
          "div",
          { ref: frame, style: "position:relative;width:720px;height:260px" },
          [
            h("div", {
              id: "terminal",
              ref: emulator.terminalMountRef,
              style: "width:100%;height:100%",
              onContextmenuCapture: actions.handleTerminalContextMenu,
            }),
            h(TerminalContextMenu, {
              ref: actions.setTerminalContextMenuRef,
              open: actions.terminalContextMenuOpen.value,
              hasSelection: actions.terminalContextMenuHasSelection.value,
              canPaste: false,
              menuStyle: actions.terminalContextMenuStyle.value,
              onCopy: actions.copyTerminalSelectionFromMenu,
              onCopyAll: actions.copyAllTerminalText,
              onClose: actions.closeTerminalContextMenu,
            }),
          ],
        ),
        h("textarea", { id: "paste-target", "aria-label": "Paste target" }),
        h(TerminalCopyDialog, {
          open: actions.copyDialogOpen.value,
          text: actions.copyDialogText.value,
          copying: actions.copying.value,
          "onUpdate:open": actions.closeCopyDialog,
          onRetry: actions.retryTerminalCopy,
          onDownload: actions.downloadCopyText,
          onCloseAutoFocus: actions.focusAfterCopyDialog,
        }),
      ]);
  },
})
  .use(i18n)
  .mount("#app");
