const escapeHtml = (value: string): string =>
  value
    .replace(/&/gu, "&amp;")
    .replace(/</gu, "&lt;")
    .replace(/>/gu, "&gt;")
    .replace(/"/gu, "&quot;")
    .replace(/'/gu, "&#39;");

const renderStrongText = (value: string): string =>
  escapeHtml(value).replace(/\*\*([^*\n]+)\*\*/gu, "<strong>$1</strong>");

const LINK_PATTERN = /(?<!!)\[([^\]\n]+)\]\((https?:\/\/[^\s<>"')]+)\)/gu;

const ALERT_PATTERN =
  /^>\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*$/iu;

const ALERT_LABELS = {
  note: "Note",
  tip: "Tip",
  important: "Important",
  warning: "Warning",
  caution: "Caution",
} as const;

type AlertKind = keyof typeof ALERT_LABELS;

const renderInline = (source: string): string => {
  let html = "";
  let cursor = 0;

  for (const match of source.matchAll(LINK_PATTERN)) {
    const index = match.index;
    html += renderStrongText(source.slice(cursor, index));

    const label = match[1] ?? "";
    const href = match[2] ?? "";
    let url: URL;
    try {
      url = new URL(href);
    } catch {
      html += renderStrongText(match[0]);
      cursor = index + match[0].length;
      continue;
    }

    html += `<a href="${escapeHtml(url.href)}" target="_blank" rel="noopener noreferrer">${renderStrongText(label)}</a>`;
    cursor = index + match[0].length;
  }

  return html + renderStrongText(source.slice(cursor));
};

export const renderReleaseNotesHtml = (
  releaseNotes: string | null | undefined,
  fallback: string,
): string => {
  const source = (releaseNotes || fallback).trim().replace(/\r\n?/gu, "\n");
  return renderBlocks(source.split("\n"));
};

const renderBlocks = (lines: string[]): string => {
  const html: string[] = [];
  let paragraph: string[] = [];
  let listItems: string[] = [];

  const flushParagraph = () => {
    if (paragraph.length === 0) return;
    html.push(`<p>${renderInline(paragraph.join(" "))}</p>`);
    paragraph = [];
  };

  const flushList = () => {
    if (listItems.length === 0) return;
    html.push(
      `<ul>${listItems.map((item) => `<li>${renderInline(item)}</li>`).join("")}</ul>`,
    );
    listItems = [];
  };

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex] ?? "";
    const trimmed = line.trim();
    if (!trimmed) {
      flushParagraph();
      flushList();
      continue;
    }

    const alert = trimmed.match(ALERT_PATTERN);
    if (alert) {
      flushParagraph();
      flushList();

      const alertLines: string[] = [];
      while (lineIndex + 1 < lines.length) {
        const quotedLine = (lines[lineIndex + 1] ?? "").match(/^\s*>\s?(.*)$/u);
        if (!quotedLine) break;
        alertLines.push(quotedLine[1] ?? "");
        lineIndex += 1;
      }

      const kind = (alert[1] ?? "note").toLowerCase() as AlertKind;
      const label = ALERT_LABELS[kind];
      html.push(
        `<aside class="release-note-alert release-note-alert--${kind}" aria-label="${label}">` +
          `<p class="release-note-alert__title">${label}</p>` +
          `<div class="release-note-alert__body">${renderBlocks(alertLines)}</div>` +
          "</aside>",
      );
      continue;
    }

    const heading = trimmed.match(/^#\s+(.+)$/u);
    if (heading) {
      flushParagraph();
      flushList();
      html.push(`<h4>${renderInline(heading[1] ?? "")}</h4>`);
      continue;
    }

    if (/^-{3,}$/u.test(trimmed)) {
      flushParagraph();
      flushList();
      html.push("<hr>");
      continue;
    }

    const listItem = trimmed.match(/^-\s+(.+)$/u);
    if (listItem) {
      flushParagraph();
      listItems.push(listItem[1] ?? "");
      continue;
    }

    flushList();
    paragraph.push(trimmed);
  }

  flushParagraph();
  flushList();
  return html.join("");
};
