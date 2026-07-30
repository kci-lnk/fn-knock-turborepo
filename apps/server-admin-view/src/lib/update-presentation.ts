export const OFFICIAL_WEBSITE_URL = "https://www.fnknock.cn/";
export const OFFICIAL_DOCUMENTATION_URL = "https://docs.fnknock.cn/";

export const FULL_VERSION_WEBSITE_URL = OFFICIAL_WEBSITE_URL;

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
