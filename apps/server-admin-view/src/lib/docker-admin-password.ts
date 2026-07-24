export const DOCKER_ADMIN_PASSWORD_MIN_BYTES = 6;
export const DOCKER_ADMIN_PASSWORD_MAX_BYTES = 128;

export type DockerAdminPasswordValidationError =
  | "tooShort"
  | "tooLong"
  | "containsWhitespace"
  | "missingLetterOrNumber";

export const dockerAdminPasswordValidationMessageKeys = {
  tooShort: "admin.dockerAdmin.passwordMin",
  tooLong: "admin.dockerAdmin.passwordMax",
  containsWhitespace: "admin.dockerAdmin.passwordNoWhitespace",
  missingLetterOrNumber: "admin.dockerAdmin.passwordRequireLetterNumber",
} as const satisfies Record<DockerAdminPasswordValidationError, string>;

const textEncoder = new TextEncoder();

const isDockerAdminPasswordWhitespace = (character: string) => {
  const codePoint = character.codePointAt(0);
  return (
    codePoint !== undefined &&
    ((codePoint >= 0x0009 && codePoint <= 0x000d) ||
      codePoint === 0x0020 ||
      codePoint === 0x0085 ||
      codePoint === 0x00a0 ||
      codePoint === 0x1680 ||
      (codePoint >= 0x2000 && codePoint <= 0x200a) ||
      codePoint === 0x2028 ||
      codePoint === 0x2029 ||
      codePoint === 0x202f ||
      codePoint === 0x205f ||
      codePoint === 0x3000)
  );
};

export const getDockerAdminPasswordByteLength = (password: string) =>
  textEncoder.encode(password).byteLength;

export const validateDockerAdminPassword = (
  password: string,
): DockerAdminPasswordValidationError | null => {
  const byteLength = getDockerAdminPasswordByteLength(password);

  if (byteLength < DOCKER_ADMIN_PASSWORD_MIN_BYTES) {
    return "tooShort";
  }
  if (byteLength > DOCKER_ADMIN_PASSWORD_MAX_BYTES) {
    return "tooLong";
  }
  if (Array.from(password).some(isDockerAdminPasswordWhitespace)) {
    return "containsWhitespace";
  }
  if (!/[A-Za-z]/u.test(password) || !/\d/u.test(password)) {
    return "missingLetterOrNumber";
  }

  return null;
};
