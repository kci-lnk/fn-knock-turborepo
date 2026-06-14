import { randomBytes } from "node:crypto";
import { hostname as getHostname } from "node:os";
import net, { type Socket } from "node:net";
import tls, { type TLSSocket } from "node:tls";
import type {
  NotificationDispatchContext,
  NotificationMessage,
  NotificationProvider,
  NotificationProviderDefinition,
  NotificationSchemaField,
  NotificationSendResult,
} from "../types";
import {
  splitCommaSeparatedValues,
  toPlainRecord,
  toTrimmedString,
  truncateText,
} from "./shared";
import { tDefault } from "../../i18n";

type EmailTransportSecurity = "ssl_tls" | "starttls" | "none";
type EmailAuthMode = "auto" | "plain" | "login" | "none";

type SmtpResponse = {
  code: number;
  lines: string[];
  message: string;
};

const emailT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.notifications.providers.catalog.email.${key}`, params);

class SmtpCommandError extends Error {
  readonly retryable: boolean;
  readonly response?: SmtpResponse;

  constructor(
    message: string,
    options?: {
      retryable?: boolean;
      response?: SmtpResponse;
    },
  ) {
    super(message);
    this.name = "SmtpCommandError";
    this.retryable = options?.retryable ?? true;
    this.response = options?.response;
  }
}

const EMAIL_CONNECTION_SCHEMA: NotificationSchemaField[] = [
  {
    key: "smtp_host",
    label: emailT("fields.smtp_host.label"),
    description: emailT("fields.smtp_host.description"),
    placeholder: "smtp.example.com",
    type: "string",
    required: true,
  },
  {
    key: "smtp_port",
    label: emailT("fields.smtp_port.label"),
    description: emailT("fields.smtp_port.description"),
    type: "number",
    required: true,
    default_value: 465,
    min: 1,
    max: 65535,
  },
  {
    key: "smtp_security",
    label: emailT("fields.smtp_security.label"),
    type: "select",
    required: true,
    default_value: "ssl_tls",
    options: [
      { label: "SSL/TLS", value: "ssl_tls" },
      { label: "STARTTLS", value: "starttls" },
      { label: emailT("fields.smtp_security.options.none"), value: "none" },
    ],
  },
  {
    key: "smtp_auth_mode",
    label: emailT("fields.smtp_auth_mode.label"),
    description: emailT("fields.smtp_auth_mode.description"),
    type: "select",
    required: true,
    default_value: "auto",
    options: [
      { label: emailT("fields.smtp_auth_mode.options.auto"), value: "auto" },
      { label: "AUTH PLAIN", value: "plain" },
      { label: "AUTH LOGIN", value: "login" },
      { label: emailT("fields.smtp_auth_mode.options.none"), value: "none" },
    ],
  },
  {
    key: "smtp_username",
    label: emailT("fields.smtp_username.label"),
    placeholder: "no-reply@example.com",
    type: "string",
  },
  {
    key: "smtp_password",
    label: emailT("fields.smtp_password.label"),
    placeholder: "password",
    type: "string",
    sensitive: true,
  },
  {
    key: "from_address",
    label: emailT("fields.from_address.label"),
    description: emailT("fields.from_address.description"),
    placeholder: "no-reply@example.com",
    type: "string",
    required: true,
  },
  {
    key: "from_name",
    label: emailT("fields.from_name.label"),
    placeholder: "fn-knock",
    type: "string",
  },
  {
    key: "to_addresses",
    label: emailT("fields.to_addresses.label"),
    description: emailT("fields.to_addresses.description"),
    placeholder: "ops@example.com, admin@example.com",
    type: "string",
    required: true,
  },
  {
    key: "cc_addresses",
    label: emailT("fields.cc_addresses.label"),
    placeholder: "audit@example.com",
    type: "string",
  },
  {
    key: "bcc_addresses",
    label: emailT("fields.bcc_addresses.label"),
    placeholder: "archive@example.com",
    type: "string",
  },
  {
    key: "reply_to",
    label: emailT("fields.reply_to.label"),
    placeholder: "support@example.com",
    type: "string",
  },
  {
    key: "allow_invalid_tls",
    label: emailT("fields.allow_invalid_tls.label"),
    description: emailT("fields.allow_invalid_tls.description"),
    type: "boolean",
    default_value: false,
  },
  {
    key: "timeout_seconds",
    label: emailT("fields.timeout_seconds.label"),
    type: "number",
    required: true,
    default_value: 10,
    min: 1,
    max: 30,
  },
  {
    key: "imap_host",
    label: emailT("fields.imap_host.label"),
    description: emailT("fields.imap_host.description"),
    placeholder: "imap.example.com",
    type: "string",
  },
  {
    key: "imap_port",
    label: emailT("fields.imap_port.label"),
    type: "number",
    default_value: 993,
    min: 1,
    max: 65535,
  },
  {
    key: "imap_security",
    label: emailT("fields.imap_security.label"),
    type: "select",
    default_value: "ssl_tls",
    options: [
      { label: "SSL/TLS", value: "ssl_tls" },
      { label: "STARTTLS", value: "starttls" },
      { label: emailT("fields.imap_security.options.none"), value: "none" },
    ],
  },
  {
    key: "imap_username",
    label: emailT("fields.imap_username.label"),
    placeholder: "no-reply@example.com",
    type: "string",
  },
  {
    key: "imap_password",
    label: emailT("fields.imap_password.label"),
    placeholder: "password",
    type: "string",
    sensitive: true,
  },
  {
    key: "imap_mailbox",
    label: emailT("fields.imap_mailbox.label"),
    placeholder: "INBOX",
    type: "string",
    default_value: "INBOX",
  },
];

const EMAIL_TARGET_SCHEMA: NotificationSchemaField[] = [
  {
    key: "to_addresses",
    label: emailT("fields.to_addresses.targetLabel"),
    description: emailT("fields.to_addresses.targetDescription"),
    placeholder: "team@example.com",
    type: "string",
  },
  {
    key: "cc_addresses",
    label: emailT("fields.cc_addresses.targetLabel"),
    placeholder: "audit@example.com",
    type: "string",
  },
  {
    key: "bcc_addresses",
    label: emailT("fields.bcc_addresses.targetLabel"),
    placeholder: "archive@example.com",
    type: "string",
  },
  {
    key: "reply_to",
    label: emailT("fields.reply_to.targetLabel"),
    placeholder: "support@example.com",
    type: "string",
  },
  {
    key: "subject_prefix",
    label: emailT("fields.subject_prefix.label"),
    description: emailT("fields.subject_prefix.description"),
    placeholder: emailT("fields.subject_prefix.placeholder"),
    type: "string",
  },
];

export const emailProviderDefinition: NotificationProviderDefinition = {
  type: "email",
  label: emailT("label"),
  description: emailT("description"),
  connection_schema: EMAIL_CONNECTION_SCHEMA,
  target_schema: EMAIL_TARGET_SCHEMA,
  sensitive_fields: ["smtp_password", "imap_password"],
  capabilities: {
    supports_text: true,
    supports_markdown: false,
    supports_rich_blocks: false,
    supports_actions: true,
    supports_mentions: false,
    supports_attachments: false,
    supports_provider_dedupe_key: false,
    max_body_length: null,
  },
};

const ASCII_HEADER_PATTERN = /^[\t\x20-\x7e]*$/;
const EMAIL_ADDRESS_PATTERN =
  /^[A-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Z0-9-]+(?:\.[A-Z0-9-]+)+$/i;

const sanitizeHeaderValue = (value: string) =>
  value.replace(/[\r\n]+/g, " ").trim();

const encodeHeaderValue = (value: string) => {
  const sanitized = sanitizeHeaderValue(value);
  if (!sanitized) return "";
  if (ASCII_HEADER_PATTERN.test(sanitized)) {
    return sanitized;
  }
  return `=?UTF-8?B?${Buffer.from(sanitized, "utf8").toString("base64")}?=`;
};

const chunkBase64 = (value: string, lineLength = 76) => {
  const lines: string[] = [];
  for (let index = 0; index < value.length; index += lineLength) {
    lines.push(value.slice(index, index + lineLength));
  }
  return lines.join("\r\n");
};

const normalizeSecurityMode = (value: unknown): EmailTransportSecurity => {
  const candidate = toTrimmedString(value).toLowerCase();
  if (
    candidate === "ssl_tls" ||
    candidate === "starttls" ||
    candidate === "none"
  ) {
    return candidate;
  }
  return "ssl_tls";
};

const normalizeAuthMode = (value: unknown): EmailAuthMode => {
  const candidate = toTrimmedString(value).toLowerCase();
  if (
    candidate === "auto" ||
    candidate === "plain" ||
    candidate === "login" ||
    candidate === "none"
  ) {
    return candidate;
  }
  return "auto";
};

const parsePort = (value: unknown, fallback: number) => {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  const port = Math.floor(parsed);
  if (port < 1 || port > 65535) return fallback;
  return port;
};

const extractEmailAddress = (value: string) => {
  const trimmed = value.trim();
  const angleMatch = trimmed.match(/<([^<>]+)>/);
  return (angleMatch?.[1] || trimmed).trim();
};

const parseEmailList = (value: unknown, fieldLabel: string) => {
  const items = splitCommaSeparatedValues(value);
  const result: string[] = [];

  for (const item of items) {
    const address = extractEmailAddress(item);
    if (!EMAIL_ADDRESS_PATTERN.test(address)) {
      throw new Error(
        emailT("errors.invalidEmailAddress", {
          field: fieldLabel,
          value: item,
        }),
      );
    }
    result.push(address);
  }

  return Array.from(new Set(result));
};

const formatMailbox = (address: string, displayName?: string) => {
  const encodedName = encodeHeaderValue(displayName || "");
  return encodedName ? `${encodedName} <${address}>` : address;
};

const resolveClientHostname = () => {
  const candidate = getHostname().trim().toLowerCase();
  if (!candidate) return "localhost";
  const normalized = candidate.replace(/[^a-z0-9.-]/g, "-");
  return normalized || "localhost";
};

const buildEmailSubject = (
  message: NotificationMessage,
  subjectPrefix = "",
) => {
  const title = toTrimmedString(message.title || emailT("message.fallbackTitle"));
  const prefix = toTrimmedString(subjectPrefix);
  return (
    [prefix, title].filter(Boolean).join(" ").trim() ||
    emailT("message.fallbackTitle")
  );
};

const buildPlainTextBody = (message: NotificationMessage) => {
  const sections: string[] = [];
  const summary = toTrimmedString(message.summary);
  const bodyText = toTrimmedString(message.body_text);

  if (summary) {
    sections.push(summary);
  }

  if (bodyText) {
    sections.push(bodyText);
  }

  if (message.facts.length > 0) {
    sections.push(
      [
        emailT("message.details"),
        ...message.facts.map((fact) => `${fact.label}: ${fact.value}`),
      ].join("\n"),
    );
  }

  if (message.actions.length > 0) {
    sections.push(
      [
        emailT("message.actionLinks"),
        ...message.actions.map((action) => `${action.label}: ${action.url}`),
      ].join("\n"),
    );
  }

  const footer: string[] = [
    emailT("message.severity", { value: message.severity }),
  ];
  if (message.event_id) {
    footer.push(emailT("message.eventId", { value: message.event_id }));
  }
  footer.push(emailT("message.occurredAt", { value: message.occurred_at }));
  sections.push(footer.join("\n"));

  return (
    sections.filter(Boolean).join("\n\n").trim() ||
    emailT("message.fallbackTitle")
  );
};

const normalizeMessageForData = (message: string) =>
  message
    .replace(/\r?\n/g, "\n")
    .split("\n")
    .map((line) => (line.startsWith(".") ? `.${line}` : line))
    .join("\r\n");

const buildMimeMessage = (args: {
  fromAddress: string;
  fromName?: string;
  to: string[];
  cc: string[];
  replyTo: string[];
  subject: string;
  bodyText: string;
}) => {
  const messageIdDomain =
    args.fromAddress.split("@")[1]?.trim() || resolveClientHostname();
  const headers = [
    `From: ${formatMailbox(args.fromAddress, args.fromName)}`,
    `To: ${args.to.length > 0 ? args.to.join(", ") : "undisclosed-recipients:;"}`,
    ...(args.cc.length > 0 ? [`Cc: ${args.cc.join(", ")}`] : []),
    ...(args.replyTo.length > 0
      ? [`Reply-To: ${args.replyTo.join(", ")}`]
      : []),
    `Subject: ${encodeHeaderValue(args.subject)}`,
    `Date: ${new Date().toUTCString()}`,
    `Message-ID: <${randomBytes(12).toString("hex")}@${messageIdDomain}>`,
    "MIME-Version: 1.0",
    "Content-Type: text/plain; charset=UTF-8",
    "Content-Transfer-Encoding: base64",
    "X-Mailer: fn-knock",
  ];

  const bodyBase64 = chunkBase64(
    Buffer.from(args.bodyText, "utf8").toString("base64"),
  );

  return `${headers.join("\r\n")}\r\n\r\n${bodyBase64}\r\n`;
};

type LineReader = {
  readLine: () => Promise<string>;
  dispose: () => void;
};

const createLineReader = (socket: Socket | TLSSocket): LineReader => {
  socket.setEncoding("utf8");

  let buffer = "";
  const pendingLines: string[] = [];
  const waiters: Array<{
    resolve: (value: string) => void;
    reject: (reason?: unknown) => void;
  }> = [];

  const rejectAll = (reason: unknown) => {
    while (waiters.length > 0) {
      waiters.shift()!.reject(reason);
    }
  };

  const flushBufferedLines = () => {
    while (pendingLines.length > 0 && waiters.length > 0) {
      waiters.shift()!.resolve(pendingLines.shift()!);
    }
  };

  const handleData = (chunk: string | Buffer) => {
    buffer += chunk.toString();
    while (true) {
      const lineBreakIndex = buffer.indexOf("\n");
      if (lineBreakIndex < 0) break;
      const rawLine = buffer.slice(0, lineBreakIndex);
      buffer = buffer.slice(lineBreakIndex + 1);
      pendingLines.push(rawLine.replace(/\r$/, ""));
    }
    flushBufferedLines();
  };

  const handleClose = () => {
    rejectAll(new Error(emailT("errors.smtpConnectionClosed")));
  };

  const handleError = (error: Error) => {
    rejectAll(error);
  };

  socket.on("data", handleData);
  socket.on("close", handleClose);
  socket.on("error", handleError);

  return {
    readLine: () =>
      new Promise<string>((resolve, reject) => {
        if (pendingLines.length > 0) {
          resolve(pendingLines.shift()!);
          return;
        }
        waiters.push({ resolve, reject });
      }),
    dispose: () => {
      socket.off("data", handleData);
      socket.off("close", handleClose);
      socket.off("error", handleError);
      rejectAll(new Error(emailT("errors.smtpReaderDisposed")));
    },
  };
};

const writeToSocket = (socket: Socket | TLSSocket, payload: string) =>
  new Promise<void>((resolve, reject) => {
    socket.write(payload, (error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });

const readSmtpResponse = async (reader: LineReader): Promise<SmtpResponse> => {
  const firstLine = await reader.readLine();
  const code = Number.parseInt(firstLine.slice(0, 3), 10);
  if (!Number.isFinite(code)) {
    throw new Error(
      emailT("errors.invalidSmtpResponse", { line: firstLine }),
    );
  }

  const lines = [firstLine];
  while (lines[lines.length - 1]?.startsWith(`${code}-`)) {
    lines.push(await reader.readLine());
  }

  return {
    code,
    lines,
    message: lines
      .map((line) => line.slice(4).trim())
      .filter(Boolean)
      .join("\n"),
  };
};

const connectSocket = async (args: {
  host: string;
  port: number;
  security: EmailTransportSecurity;
  rejectUnauthorized: boolean;
  timeoutMs: number;
}) => {
  const { host, port, security, rejectUnauthorized, timeoutMs } = args;

  if (security === "ssl_tls") {
    return new Promise<TLSSocket>((resolve, reject) => {
      const socket = tls.connect({
        host,
        port,
        servername: host,
        rejectUnauthorized,
      });
      const onError = (error: Error) => {
        reject(error);
      };

      socket.setTimeout(timeoutMs, () => {
        socket.destroy(new Error(emailT("errors.smtpConnectionTimeout")));
      });
      socket.once("secureConnect", () => {
        socket.off("error", onError);
        resolve(socket);
      });
      socket.once("error", onError);
    });
  }

  return new Promise<Socket>((resolve, reject) => {
    const socket = net.connect({ host, port });
    const onError = (error: Error) => {
      reject(error);
    };

    socket.setTimeout(timeoutMs, () => {
      socket.destroy(new Error(emailT("errors.smtpConnectionTimeout")));
    });
    socket.once("connect", () => {
      socket.off("error", onError);
      resolve(socket);
    });
    socket.once("error", onError);
  });
};

const upgradeSocketToTls = async (args: {
  socket: Socket;
  host: string;
  rejectUnauthorized: boolean;
  timeoutMs: number;
}) => {
  const { socket, host, rejectUnauthorized, timeoutMs } = args;

  return new Promise<TLSSocket>((resolve, reject) => {
    const secureSocket = tls.connect({
      socket,
      servername: host,
      rejectUnauthorized,
    });

    const onError = (error: Error) => {
      reject(error);
    };

    secureSocket.setTimeout(timeoutMs, () => {
      secureSocket.destroy(new Error(emailT("errors.smtpTlsHandshakeTimeout")));
    });
    secureSocket.once("secureConnect", () => {
      secureSocket.off("error", onError);
      resolve(secureSocket);
    });
    secureSocket.once("error", onError);
  });
};

const expectResponseCode = (
  response: SmtpResponse,
  expectedCodes: number[],
  fallbackMessage: string,
) => {
  if (expectedCodes.includes(response.code)) {
    return response;
  }

  throw new SmtpCommandError(
    emailT("errors.smtpCommandFailed", {
      message: fallbackMessage,
      code: response.code,
      response: response.message || emailT("errors.unknownResponse"),
    }),
    {
      retryable: response.code >= 400 && response.code < 500,
      response,
    },
  );
};

const sendCommand = async (args: {
  socket: Socket | TLSSocket;
  reader: LineReader;
  command: string;
  expectedCodes: number[];
  fallbackMessage: string;
}) => {
  await writeToSocket(args.socket, `${args.command}\r\n`);
  const response = await readSmtpResponse(args.reader);
  return expectResponseCode(response, args.expectedCodes, args.fallbackMessage);
};

const parseEhloCapabilities = (response: SmtpResponse) =>
  response.lines.map((line) => line.slice(4).trim()).filter(Boolean);

const extractAuthMechanisms = (capabilities: string[]) => {
  const authLine = capabilities.find((line) => /^AUTH(?:\s|=)/i.test(line));
  if (!authLine) return [];

  const value = authLine.replace(/^AUTH(?:\s|=)+/i, "").trim();
  return value
    .split(/\s+/)
    .map((item) => item.trim().toUpperCase())
    .filter(Boolean);
};

const chooseAuthMechanism = (
  authMode: EmailAuthMode,
  capabilities: string[],
) => {
  if (authMode === "none") return null;

  const mechanisms = extractAuthMechanisms(capabilities);
  if (authMode === "plain") {
    if (!mechanisms.includes("PLAIN")) {
      throw new Error(emailT("errors.authPlainUnsupported"));
    }
    return "PLAIN";
  }
  if (authMode === "login") {
    if (!mechanisms.includes("LOGIN")) {
      throw new Error(emailT("errors.authLoginUnsupported"));
    }
    return "LOGIN";
  }

  if (mechanisms.includes("PLAIN")) return "PLAIN";
  if (mechanisms.includes("LOGIN")) return "LOGIN";
  if (mechanisms.length === 0) return null;

  throw new Error(
    emailT("errors.unsupportedAuthMechanisms", {
      mechanisms: mechanisms.join(", "),
    }),
  );
};

const performSmtpAuth = async (args: {
  socket: Socket | TLSSocket;
  reader: LineReader;
  mechanism: "PLAIN" | "LOGIN";
  username: string;
  password: string;
}) => {
  if (args.mechanism === "PLAIN") {
    const token = Buffer.from(
      `\u0000${args.username}\u0000${args.password}`,
      "utf8",
    ).toString("base64");
    await sendCommand({
      socket: args.socket,
      reader: args.reader,
      command: `AUTH PLAIN ${token}`,
      expectedCodes: [235],
      fallbackMessage: emailT("errors.authFailed"),
    });
    return;
  }

  await sendCommand({
    socket: args.socket,
    reader: args.reader,
    command: "AUTH LOGIN",
    expectedCodes: [334],
    fallbackMessage: emailT("errors.authFailed"),
  });
  await sendCommand({
    socket: args.socket,
    reader: args.reader,
    command: Buffer.from(args.username, "utf8").toString("base64"),
    expectedCodes: [334],
    fallbackMessage: emailT("errors.usernameAuthFailed"),
  });
  await sendCommand({
    socket: args.socket,
    reader: args.reader,
    command: Buffer.from(args.password, "utf8").toString("base64"),
    expectedCodes: [235],
    fallbackMessage: emailT("errors.passwordAuthFailed"),
  });
};

const sendDataBlock = async (args: {
  socket: Socket | TLSSocket;
  reader: LineReader;
  data: string;
}) => {
  await sendCommand({
    socket: args.socket,
    reader: args.reader,
    command: "DATA",
    expectedCodes: [354],
    fallbackMessage: emailT("errors.dataStartFailed"),
  });
  await writeToSocket(
    args.socket,
    `${normalizeMessageForData(args.data)}\r\n.\r\n`,
  );
  const response = await readSmtpResponse(args.reader);
  return expectResponseCode(response, [250], emailT("errors.submitFailed"));
};

const resolveEmailTargetConfig = (
  provider: NotificationProvider,
  context?: Partial<NotificationDispatchContext>,
) => {
  const providerConfig = provider.connection_config;
  const targetConfig = toPlainRecord(context?.target?.target_config);

  const to = parseEmailList(
    targetConfig.to_addresses ?? providerConfig.to_addresses,
    emailT("fields.to_addresses.addressLabel"),
  );
  const cc = parseEmailList(
    targetConfig.cc_addresses ?? providerConfig.cc_addresses,
    emailT("fields.cc_addresses.addressLabel"),
  );
  const bcc = parseEmailList(
    targetConfig.bcc_addresses ?? providerConfig.bcc_addresses,
    emailT("fields.bcc_addresses.addressLabel"),
  );
  const replyTo = parseEmailList(
    targetConfig.reply_to ?? providerConfig.reply_to,
    emailT("fields.reply_to.addressLabel"),
  );
  const fromAddress = extractEmailAddress(
    toTrimmedString(providerConfig.from_address),
  );

  if (!EMAIL_ADDRESS_PATTERN.test(fromAddress)) {
    throw new Error(emailT("errors.invalidFromAddress"));
  }
  if (to.length === 0 && cc.length === 0 && bcc.length === 0) {
    throw new Error(emailT("errors.recipientRequired"));
  }

  return {
    fromAddress,
    fromName: toTrimmedString(providerConfig.from_name),
    to,
    cc,
    bcc,
    replyTo,
    subjectPrefix: toTrimmedString(targetConfig.subject_prefix),
  };
};

export const sendEmailMessage = async (args: {
  provider: NotificationProvider;
  message: NotificationMessage;
  context?: Partial<NotificationDispatchContext>;
  timeoutSeconds: number;
}): Promise<NotificationSendResult> => {
  const providerConfig = args.provider.connection_config;
  const smtpHost = toTrimmedString(providerConfig.smtp_host);
  const smtpPort = parsePort(providerConfig.smtp_port, 465);
  const smtpSecurity = normalizeSecurityMode(providerConfig.smtp_security);
  const smtpAuthMode = normalizeAuthMode(providerConfig.smtp_auth_mode);
  const smtpUsername = toTrimmedString(providerConfig.smtp_username);
  const smtpPassword = toTrimmedString(providerConfig.smtp_password);
  const allowInvalidTls = Boolean(providerConfig.allow_invalid_tls);

  if (!smtpHost) {
    return {
      success: false,
      retryable: false,
      reason: emailT("errors.missingSmtpHost"),
    };
  }

  const requestSummary: Record<string, unknown> = {
    host: smtpHost,
    port: smtpPort,
    security: smtpSecurity,
    auth_mode: smtpAuthMode,
    timeout_seconds: Math.max(1, args.timeoutSeconds),
    imap_configured: Boolean(toTrimmedString(providerConfig.imap_host)),
  };

  let lastResponse: SmtpResponse | undefined;
  let socket: Socket | TLSSocket | null = null;
  let reader: LineReader | null = null;

  try {
    const target = resolveEmailTargetConfig(args.provider, args.context);
    const allRecipients = Array.from(
      new Set([...target.to, ...target.cc, ...target.bcc]),
    );
    const timeoutMs = Math.max(1, args.timeoutSeconds) * 1000;
    const subject = buildEmailSubject(args.message, target.subjectPrefix);
    const bodyText = buildPlainTextBody(args.message);
    const data = buildMimeMessage({
      fromAddress: target.fromAddress,
      fromName: target.fromName,
      to: target.to,
      cc: target.cc,
      replyTo: target.replyTo,
      subject,
      bodyText,
    });

    requestSummary.from_address = target.fromAddress;
    requestSummary.recipient_count = allRecipients.length;
    requestSummary.to_count = target.to.length;
    requestSummary.cc_count = target.cc.length;
    requestSummary.bcc_count = target.bcc.length;
    requestSummary.subject_preview = truncateText(subject, 120);

    socket = await connectSocket({
      host: smtpHost,
      port: smtpPort,
      security: smtpSecurity,
      rejectUnauthorized: !allowInvalidTls,
      timeoutMs,
    });
    reader = createLineReader(socket);

    lastResponse = await readSmtpResponse(reader);
    expectResponseCode(lastResponse, [220], emailT("errors.handshakeFailed"));

    const ehloCommand = `EHLO ${resolveClientHostname()}`;
    lastResponse = await sendCommand({
      socket,
      reader,
      command: ehloCommand,
      expectedCodes: [250],
      fallbackMessage: emailT("errors.ehloFailed"),
    });

    let capabilities = parseEhloCapabilities(lastResponse);

    if (smtpSecurity === "starttls") {
      const supportsStartTls = capabilities.some((line) =>
        /^STARTTLS$/i.test(line),
      );
      if (!supportsStartTls) {
        throw new Error(emailT("errors.startTlsUnsupported"));
      }

      lastResponse = await sendCommand({
        socket,
        reader,
        command: "STARTTLS",
        expectedCodes: [220],
        fallbackMessage: emailT("errors.startTlsFailed"),
      });

      reader.dispose();
      reader = null;
      socket = await upgradeSocketToTls({
        socket: socket as Socket,
        host: smtpHost,
        rejectUnauthorized: !allowInvalidTls,
        timeoutMs,
      });
      reader = createLineReader(socket);

      lastResponse = await sendCommand({
        socket,
        reader,
        command: ehloCommand,
        expectedCodes: [250],
        fallbackMessage: emailT("errors.ehloAfterTlsFailed"),
      });
      capabilities = parseEhloCapabilities(lastResponse);
    }

    const authMechanism = chooseAuthMechanism(smtpAuthMode, capabilities);
    if (authMechanism) {
      if (!smtpUsername || !smtpPassword) {
        throw new Error(emailT("errors.credentialsRequired"));
      }
      requestSummary.auth_mechanism = authMechanism;
      await performSmtpAuth({
        socket,
        reader,
        mechanism: authMechanism,
        username: smtpUsername,
        password: smtpPassword,
      });
    } else if (smtpAuthMode !== "none" && (smtpUsername || smtpPassword)) {
      throw new Error(emailT("errors.noAuthMechanism"));
    }

    lastResponse = await sendCommand({
      socket,
      reader,
      command: `MAIL FROM:<${target.fromAddress}>`,
      expectedCodes: [250],
      fallbackMessage: emailT("errors.mailFromFailed"),
    });

    for (const recipient of allRecipients) {
      lastResponse = await sendCommand({
        socket,
        reader,
        command: `RCPT TO:<${recipient}>`,
        expectedCodes: [250, 251],
        fallbackMessage: emailT("errors.recipientSetFailed", {
          recipient,
        }),
      });
    }

    lastResponse = await sendDataBlock({
      socket,
      reader,
      data,
    });

    await sendCommand({
      socket,
      reader,
      command: "QUIT",
      expectedCodes: [221],
      fallbackMessage: emailT("errors.quitFailed"),
    }).catch(() => {});

    reader.dispose();
    reader = null;
    socket.destroy();
    socket = null;

    return {
      success: true,
      retryable: false,
      request_summary: requestSummary,
      response_summary: {
        code: lastResponse.code,
        message_preview: truncateText(lastResponse.message, 240),
      },
    };
  } catch (error) {
    const smtpError = error instanceof SmtpCommandError ? error : null;
    const response = smtpError?.response || lastResponse;
    const reason =
      error instanceof Error ? error.message : emailT("errors.deliveryFailed");

    if (reader) {
      reader.dispose();
    }
    if (socket && !socket.destroyed) {
      socket.destroy();
    }

    return {
      success: false,
      retryable:
        smtpError?.retryable ??
        (response ? response.code >= 400 && response.code < 500 : true),
      reason,
      request_summary: requestSummary,
      response_summary: response
        ? {
            code: response.code,
            lines: response.lines.map((line) => truncateText(line, 240)),
          }
        : undefined,
    };
  }
};
