export type ParsedHostPort = {
  host: string;
  port: number;
  isIPv6: boolean;
};

const INVALID_HOST_CHARACTER_PATTERN = /[\s/\\?#@]/;
const IPV4_SEGMENT_PATTERN =
  /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/;
const IPV6_SEGMENT_PATTERN = /^[\da-f]{1,4}$/i;

const isValidHostname = (host: string): boolean => {
  if (!host) return false;
  if (INVALID_HOST_CHARACTER_PATTERN.test(host)) return false;
  if (host.includes(":") || host.includes("[") || host.includes("]")) {
    return false;
  }
  return true;
};

const isValidIPv4Address = (value: string): boolean => {
  const segments = value.split(".");
  return (
    segments.length === 4 &&
    segments.every((segment) => IPV4_SEGMENT_PATTERN.test(segment))
  );
};

const countIPv6Segments = (segments: string[]): number => {
  let count = 0;

  for (const [index, segment] of segments.entries()) {
    if (!segment) return -1;

    if (segment.includes(".")) {
      if (index !== segments.length - 1 || !isValidIPv4Address(segment)) {
        return -1;
      }
      count += 2;
      continue;
    }

    if (!IPV6_SEGMENT_PATTERN.test(segment)) {
      return -1;
    }

    count += 1;
  }

  return count;
};

const isValidIPv6Address = (value: string): boolean => {
  if (!value || INVALID_HOST_CHARACTER_PATTERN.test(value)) return false;
  if (!value.includes(":") || value.includes(":::")) return false;

  const compressionCount = value.match(/::/g)?.length ?? 0;
  if (compressionCount > 1) return false;

  const [leftRaw = "", rightRaw = ""] = value.split("::");
  const leftSegments = leftRaw ? leftRaw.split(":") : [];
  const rightSegments = rightRaw ? rightRaw.split(":") : [];

  const leftCount = countIPv6Segments(leftSegments);
  if (leftCount < 0) return false;

  const rightCount = countIPv6Segments(rightSegments);
  if (rightCount < 0) return false;

  const totalSegments = leftCount + rightCount;
  if (compressionCount === 1) {
    return totalSegments < 8;
  }

  return totalSegments === 8;
};

export const parseHostPort = (value: string): ParsedHostPort | null => {
  const trimmed = value.trim();
  if (!trimmed) return null;

  let host = "";
  let portText = "";
  let isIPv6 = false;

  if (trimmed.startsWith("[")) {
    const closingBracketIndex = trimmed.indexOf("]");
    if (closingBracketIndex <= 1) return null;

    host = trimmed.slice(1, closingBracketIndex);
    portText = trimmed.slice(closingBracketIndex + 1);
    if (!portText.startsWith(":")) return null;

    portText = portText.slice(1);
    isIPv6 = true;

    if (!isValidIPv6Address(host)) return null;
  } else {
    const separatorIndex = trimmed.lastIndexOf(":");
    if (separatorIndex <= 0) return null;
    if (trimmed.indexOf(":") !== separatorIndex) return null;

    host = trimmed.slice(0, separatorIndex);
    portText = trimmed.slice(separatorIndex + 1);

    if (!isValidHostname(host)) return null;
  }

  if (!/^\d+$/.test(portText)) return null;

  const port = Number.parseInt(portText, 10);
  if (!Number.isFinite(port) || port <= 0 || port > 65535) {
    return null;
  }

  return {
    host,
    port,
    isIPv6,
  };
};

export const isValidHostPort = (value: string): boolean =>
  parseHostPort(value) !== null;
