const TARGET_NAME_CHARACTERS =
  "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

export const createRandomTargetName = (prefix: string): string => {
  const entropy = new Uint8Array(5);
  globalThis.crypto.getRandomValues(entropy);
  const suffix = Array.from(
    entropy,
    (value) => TARGET_NAME_CHARACTERS[value % TARGET_NAME_CHARACTERS.length],
  ).join("");
  return `${prefix}${suffix}`;
};
