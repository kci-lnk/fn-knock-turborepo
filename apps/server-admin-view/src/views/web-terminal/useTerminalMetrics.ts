import { TerminalAPI } from "@/lib/api/terminal";
import {
  useTerminalResource,
  type TerminalResourceContext,
} from "./useTerminalResource";

export const useTerminalMetrics = (context: TerminalResourceContext) =>
  useTerminalResource(context, TerminalAPI.getAttachmentMetrics);
