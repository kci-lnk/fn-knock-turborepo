const synologyCgiPathPattern = /(?:^|\/)fn-knock-synology\/index\.cgi(?:\/|$)/;

export const isSynologyCgiApiPath = (path: string) =>
  synologyCgiPathPattern.test(path);
