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
    case "NotAllowedError":
      return messages.cancelled;
    case "InvalidStateError":
      return messages.alreadyRegistered;
    case "ConstraintError":
    case "NotSupportedError":
    case "OperationError":
    case "SecurityError":
    case "UnknownError":
      return messages.unavailable;
    default:
      return details.message
        .toLowerCase()
        .includes("unknown transient reason")
        ? messages.unavailable
        : messages.failed;
  }
};
