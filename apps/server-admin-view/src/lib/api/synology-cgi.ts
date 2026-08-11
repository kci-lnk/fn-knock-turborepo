const synologyCgiPathPattern = /(?:^|\/)fn-knock-synology\/index\.cgi(?:\/|$)/;
const fnKnockCgiPathPattern =
  /(?:^|\/)fn-knock(?:-lite|-synology)?\/index\.cgi(?:\/|$)/;

export const isSynologyCgiApiPath = (path: string) =>
  synologyCgiPathPattern.test(path);

export const isFnKnockCgiApiPath = (path: string) =>
  fnKnockCgiPathPattern.test(path);
