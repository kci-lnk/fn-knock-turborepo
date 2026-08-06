export type PasskeyRegistrationErrorMessages = {
  alreadyRegistered: string;
  cancelled: string;
  failed: string;
  unavailable: string;
};

type ErrorDetails = {
  message: string;
  name: string;
};

export const getPasskeyErrorDetails = (error: unknown): ErrorDetails => {
  if (!error || typeof error !== "object") {
    return {
      name: "",
      message: typeof error === "string" ? error : "",
    };
  }

  const candidate = error as { message?: unknown; name?: unknown };
  return {
    name: typeof candidate.name === "string" ? candidate.name : "",
    message: typeof candidate.message === "string" ? candidate.message : "",
  };
};

export const resolvePasskeyRegistrationError = (
  error: unknown,
  messages: PasskeyRegistrationErrorMessages,
) => {
  const details = getPasskeyErrorDetails(error);

  switch (details.name) {
    case "AbortError":
      return messages.cancelled;
    // WebAuthn uses NotAllowedError as a catch-all for cancellation, timeout,
    // policy rejection, and an unavailable authenticator. Do not misdiagnose
    // every Windows provider failure as the user cancelling the prompt.
    case "NotAllowedError":
      return messages.unavailable;
    case "InvalidStateError":
      return messages.alreadyRegistered;
    case "ConstraintError":
    case "NotSupportedError":
    case "OperationError":
    case "SecurityError":
    case "UnknownError":
      return messages.unavailable;
    default:
      return details.message.toLowerCase().includes("unknown transient reason")
        ? messages.unavailable
        : messages.failed;
  }
};

const STANDARD_PROFILE_RETRY_ERRORS = new Set([
  "ConstraintError",
  "NotSupportedError",
  "OperationError",
  "UnknownError",
]);

export const shouldRetryPasskeyRegistrationWithStandardProfile = (
  error: unknown,
  isAndroid: boolean,
) =>
  isAndroid &&
  STANDARD_PROFILE_RETRY_ERRORS.has(getPasskeyErrorDetails(error).name);
