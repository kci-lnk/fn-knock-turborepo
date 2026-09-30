import type { GhosttyTerminal } from "ghostty-web";

type TextBuffer = Pick<
  GhosttyTerminal,
  | "rows"
  | "isAlternateScreen"
  | "getScrollbackLength"
  | "getScrollbackLine"
  | "getLine"
  | "getGraphemeString"
  | "getScrollbackGraphemeString"
>;

/** Snapshot rendered rows, including retained history; no raw ANSI stream replay. */
export function snapshotTerminalText(buffer: TextBuffer | undefined): string {
  if (!buffer) return "";
  const history = buffer.isAlternateScreen() ? 0 : buffer.getScrollbackLength();
  const lines: string[] = [];
  for (let index = 0; index < history + buffer.rows; index += 1) {
    const historical = index < history;
    const row = historical ? index : index - history;
    const cells = historical
      ? buffer.getScrollbackLine(row)
      : buffer.getLine(row);
    let text = "";
    for (const [column, cell] of (cells ?? []).entries()) {
      if (cell.width === 0) continue; // Continuation cell of a wide glyph.
      if (!cell.codepoint) text += " ";
      else if (cell.grapheme_len > 0) {
        text += historical
          ? buffer.getScrollbackGraphemeString(row, column)
          : buffer.getGraphemeString(row, column);
      } else text += String.fromCodePoint(cell.codepoint);
    }
    lines.push(text.replace(/ +$/u, ""));
  }
  while (lines.length && lines[lines.length - 1] === "") lines.pop();
  return lines.join("\n");
}
