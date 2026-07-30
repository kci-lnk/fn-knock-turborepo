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

  for (const line of source.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) {
      flushParagraph();
      flushList();
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
