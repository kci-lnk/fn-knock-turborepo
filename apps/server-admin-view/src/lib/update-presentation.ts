export const FULL_VERSION_WEBSITE_URL = "https://www.fnknock.cn/";

export const renderReleaseNotesHtml = (
  releaseNotes: string | null | undefined,
  fallback: string,
): string => {
  const raw = releaseNotes || fallback;
  let html = raw
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  html = html.replace(
    /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer" class="underline underline-offset-4 decoration-primary/50 text-primary hover:decoration-primary hover:opacity-80 transition-all font-medium">$1</a>',
  );

  return html;
};

export const shouldShowOneClickUpdate = ({
  hasUpdate,
  canSelfUpdate,
  isFpkLite,
}: {
  hasUpdate: boolean;
  canSelfUpdate: boolean;
  isFpkLite: boolean;
}): boolean => hasUpdate && canSelfUpdate && !isFpkLite;

export const resolveUpdateDetailsAction = (
  isFpkLite: boolean,
): { type: "external"; url: string } | { type: "route"; path: string } =>
  isFpkLite
    ? { type: "external", url: FULL_VERSION_WEBSITE_URL }
    : { type: "route", path: "/about" };
