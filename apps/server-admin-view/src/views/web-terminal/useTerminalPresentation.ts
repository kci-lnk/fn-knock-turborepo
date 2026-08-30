import { computed, type ComputedRef, type Ref } from "vue";
import type {
  TerminalAttachmentRecord,
  TerminalDestination,
  TerminalErrorCode,
  TerminalSessionRecord,
} from "@/lib/api/terminal";
import type { TerminalAttachmentState } from "./useTerminalAttachment";
import { localizeTerminalError } from "./terminal-errors";
import { toolbarModifierLabels, type ArmedModifier } from "./terminal-runtime";

export const useTerminalPresentation = ({
  activeAttachment,
  armedModifier,
  attachmentState,
  compactViewport,
  isTerminalFullscreen,
  lastAttachmentError,
  lastAttachmentErrorCode,
  pageError,
  readOnly,
  selectedSession,
  selectedTarget,
  translate,
}: {
  activeAttachment: Ref<TerminalAttachmentRecord | null>;
  armedModifier: Ref<ArmedModifier | null>;
  attachmentState: Ref<TerminalAttachmentState>;
  compactViewport: Ref<boolean>;
  isTerminalFullscreen: Ref<boolean>;
  lastAttachmentError: Ref<string>;
  lastAttachmentErrorCode: Ref<TerminalErrorCode | null>;
  pageError: Ref<string>;
  readOnly: Readonly<Ref<boolean>>;
  selectedSession: ComputedRef<TerminalSessionRecord | null>;
  selectedTarget: ComputedRef<TerminalDestination | null>;
  translate: (key: string, params?: Record<string, string>) => string;
}) => {
  const sessionPhaseLabel = computed(() => {
    const phase = selectedSession.value?.phase;
    return phase ? translate(`admin.webTerminal.sessionPhase.${phase}`) : "";
  });
  const connectionState = computed<
    "idle" | "connecting" | "connected" | "error"
  >(() => {
    const phase = selectedSession.value?.phase;
    if (phase === "lost" || phase === "failed") return "error";
    if (phase === "closed" || phase === "exited") return "idle";
    if (phase && phase !== "running") return "connecting";
    const kind = attachmentState.value.kind;
    if (kind === "controller" || kind === "viewer") return "connected";
    if (
      kind === "opening" ||
      kind === "snapshotting" ||
      kind === "reconnecting"
    ) {
      return "connecting";
    }
    if (kind === "error" || pageError.value) return "error";
    return "idle";
  });
  const connectionError = computed(() => {
    if (pageError.value) return pageError.value;
    return localizeTerminalError(
      {
        errorCode: lastAttachmentErrorCode.value,
        message: lastAttachmentError.value,
      },
      (key) => translate(key),
    );
  });
  const terminalWindowTitle = computed(
    () =>
      selectedSession.value?.title?.trim() ||
      translate("admin.webTerminal.title"),
  );
  const terminalWindowSubtitle = computed(() => {
    const target = selectedTarget.value;
    const session = selectedSession.value;
    if (!target) return translate("admin.webTerminal.statusDisconnected");
    const endpoint =
      target.kind === "local"
        ? `${target.executionIdentity}@${translate("admin.webTerminal.localTarget")}`
        : `${target.username}@${target.host}:${target.port}`;
    return `${endpoint}${session ? ` · ${sessionPhaseLabel.value}` : ""}`;
  });
  const statusTone = computed(() => {
    if (selectedSession.value?.phase !== "running" && sessionPhaseLabel.value) {
      return sessionPhaseLabel.value;
    }
    if (readOnly.value) return translate("admin.webTerminal.statusViewer");
    if (connectionState.value === "connected") {
      return translate("admin.webTerminal.statusConnected");
    }
    if (connectionState.value === "connecting") {
      return translate("admin.webTerminal.statusConnecting");
    }
    if (connectionState.value === "error") {
      return translate("admin.webTerminal.statusError");
    }
    return translate("admin.webTerminal.statusDisconnected");
  });
  const destroySessionDescription = computed(() => {
    const title = selectedSession.value?.title?.trim();
    return title
      ? translate("admin.webTerminal.destroyDescriptionWithTitle", { title })
      : translate("admin.webTerminal.destroyDescription");
  });
  const terminalFullscreenLabel = computed(() =>
    isTerminalFullscreen.value
      ? translate("admin.webTerminal.exitFullscreen")
      : translate("admin.webTerminal.enterFullscreen"),
  );
  const armedModifierLabel = computed(() =>
    armedModifier.value ? toolbarModifierLabels[armedModifier.value] : "",
  );

  return {
    armedModifierLabel,
    connectionError,
    connectionState,
    destroySessionDescription,
    showMobileToolbar: computed(() => compactViewport.value),
    sessionPhaseLabel,
    statusTone,
    terminalFullscreenLabel,
    terminalWindowSubtitle,
    terminalWindowTitle,
    toolbarDisabled: computed(
      () =>
        readOnly.value ||
        !activeAttachment.value ||
        activeAttachment.value.role !== "controller",
    ),
  };
};
