import { parse, stringify } from 'smol-toml'

export interface FrpcVisualDefaults {
  localPort: string
}

export interface FrpcVisualFields {
  serverAddr: string
  serverPort: string
  serverToken: string
  webUser: string
  webPassword: string
  localPort: string
  remotePort: string
}

type TomlTable = Record<string, unknown>

const DEFAULT_SERVER_PORT = '7000'
const DEFAULT_REMOTE_PORT = '0'
const DEFAULT_WEB_USER = 'admin'
const DEFAULT_WEB_ADDR = '127.0.0.1'
const DEFAULT_WEB_PORT = 7995
const DEFAULT_PROXY_NAME = 'reproxy'
const DEFAULT_PROXY_TYPE = 'tcp'
const DEFAULT_LOCAL_IP = '127.0.0.1'
const DEFAULT_PROXY_PROTOCOL_VERSION = 'v2'

function isTomlTable(value: unknown): value is TomlTable {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function parseTomlDocument(raw: string): TomlTable {
  const parsed = parse(raw.trim() ? raw : '')
  if (!isTomlTable(parsed)) {
    throw new Error('frpc.toml 必须是一个 TOML 表结构')
  }
  return parsed
}

function stringifyTomlDocument(doc: TomlTable): string {
  return stringify(doc).trimEnd().concat('\n')
}

function readString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback
}

function normalizePortString(value: unknown, fallback: string): string {
  if (typeof value === 'number' && Number.isInteger(value) && value > 0 && value <= 65535) {
    return String(value)
  }
  if (typeof value === 'bigint' && value > 0n && value <= 65535n) {
    return value.toString()
  }
  if (typeof value === 'string') {
    const trimmed = value.trim()
    if (/^\d+$/.test(trimmed)) {
      const parsed = Number.parseInt(trimmed, 10)
      if (Number.isInteger(parsed) && parsed > 0 && parsed <= 65535) {
        return String(parsed)
      }
    }
  }
  return fallback
}

function resolvePortNumber(value: string, fallback: string): number {
  return Number.parseInt(normalizePortString(value, fallback), 10)
}

function readAliasedString(table: TomlTable, keys: string[], fallback = ''): string {
  for (const key of keys) {
    const value = table[key]
    if (typeof value === 'string') {
      return value
    }
  }
  return fallback
}

function readAliasedPort(table: TomlTable, keys: string[], fallback: string): string {
  for (const key of keys) {
    if (!(key in table)) continue
    return normalizePortString(table[key], fallback)
  }
  return fallback
}

function writeAliasedString(table: TomlTable, keys: string[], value: string) {
  const existingKeys = keys.filter((key) => key in table)
  if (!existingKeys.length) {
    table[keys[0]!] = value
    return
  }
  for (const key of existingKeys) {
    table[key] = value
  }
}

function writeAliasedNumber(table: TomlTable, keys: string[], value: number) {
  const existingKeys = keys.filter((key) => key in table)
  if (!existingKeys.length) {
    table[keys[0]!] = value
    return
  }
  for (const key of existingKeys) {
    table[key] = value
  }
}

function ensureTable(table: TomlTable, key: string): TomlTable {
  const current = table[key]
  if (isTomlTable(current)) {
    return current
  }
  const next: TomlTable = {}
  table[key] = next
  return next
}

function findManagedProxy(doc: TomlTable): TomlTable | null {
  if (!Array.isArray(doc.proxies)) return null

  let firstProxy: TomlTable | null = null
  for (const entry of doc.proxies) {
    if (!isTomlTable(entry)) continue
    if (!firstProxy) {
      firstProxy = entry
    }
    if (readString(entry.name) === DEFAULT_PROXY_NAME) {
      return entry
    }
  }
  return firstProxy
}

function ensureManagedProxy(doc: TomlTable): TomlTable {
  const existing = findManagedProxy(doc)
  if (existing) {
    return existing
  }

  const proxy: TomlTable = {
    name: DEFAULT_PROXY_NAME,
    type: DEFAULT_PROXY_TYPE,
    localIP: DEFAULT_LOCAL_IP,
    transport: {
      proxyProtocolVersion: DEFAULT_PROXY_PROTOCOL_VERSION,
    },
  }

  const proxies = Array.isArray(doc.proxies) ? doc.proxies : []
  proxies.push(proxy)
  doc.proxies = proxies
  return proxy
}

function generatePassword(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`
}

export function extractVisualFieldsFromToml(raw: string, defaults: FrpcVisualDefaults): FrpcVisualFields {
  const doc = parseTomlDocument(raw)
  const auth = isTomlTable(doc.auth) ? doc.auth : null
  const webServer = isTomlTable(doc.webServer) ? doc.webServer : null
  const proxy = findManagedProxy(doc)

  return {
    serverAddr: readAliasedString(doc, ['serverAddr', 'server_addr'], '').trim(),
    serverPort: readAliasedPort(doc, ['serverPort', 'server_port'], DEFAULT_SERVER_PORT),
    serverToken: (readString(auth?.token) || readString(doc.token)).trim(),
    webUser: (webServer ? readString(webServer.user, DEFAULT_WEB_USER) : DEFAULT_WEB_USER).trim() || DEFAULT_WEB_USER,
    webPassword: (webServer ? readString(webServer.password) : '').trim(),
    localPort: proxy
      ? readAliasedPort(proxy, ['localPort', 'local_port'], defaults.localPort)
      : defaults.localPort,
    remotePort: proxy
      ? readAliasedPort(proxy, ['remotePort', 'remote_port'], DEFAULT_REMOTE_PORT)
      : DEFAULT_REMOTE_PORT,
  }
}

export function mergeVisualFieldsIntoToml(
  raw: string,
  fields: FrpcVisualFields,
  defaults: FrpcVisualDefaults,
): string {
  const doc = parseTomlDocument(raw)
  const serverAddr = fields.serverAddr.trim()
  const serverPort = resolvePortNumber(fields.serverPort, DEFAULT_SERVER_PORT)
  const serverToken = fields.serverToken.trim()
  const webUser = fields.webUser.trim() || DEFAULT_WEB_USER
  const webPassword = fields.webPassword.trim() || generatePassword()
  const localPort = resolvePortNumber(fields.localPort, defaults.localPort)
  const remotePort = resolvePortNumber(fields.remotePort, DEFAULT_REMOTE_PORT)

  writeAliasedString(doc, ['serverAddr', 'server_addr'], serverAddr)
  writeAliasedNumber(doc, ['serverPort', 'server_port'], serverPort)

  const webServer = ensureTable(doc, 'webServer')
  if (typeof webServer.addr !== 'string' || !webServer.addr.trim()) {
    webServer.addr = DEFAULT_WEB_ADDR
  }
  if (normalizePortString(webServer.port, String(DEFAULT_WEB_PORT)) !== String(DEFAULT_WEB_PORT)) {
    webServer.port = resolvePortNumber(String(webServer.port ?? DEFAULT_WEB_PORT), String(DEFAULT_WEB_PORT))
  } else {
    webServer.port = DEFAULT_WEB_PORT
  }
  webServer.user = webUser
  webServer.password = webPassword

  let wroteToken = false
  if (isTomlTable(doc.auth)) {
    doc.auth.token = serverToken
    if (typeof doc.auth.method !== 'string' || !doc.auth.method.trim()) {
      doc.auth.method = 'token'
    }
    wroteToken = true
  }
  if ('token' in doc) {
    doc.token = serverToken
    wroteToken = true
  }
  if (!wroteToken) {
    const auth = ensureTable(doc, 'auth')
    auth.method = typeof auth.method === 'string' && auth.method.trim() ? auth.method : 'token'
    auth.token = serverToken
  }

  const proxy = ensureManagedProxy(doc)
  writeAliasedNumber(proxy, ['localPort', 'local_port'], localPort)
  writeAliasedNumber(proxy, ['remotePort', 'remote_port'], remotePort)

  return stringifyTomlDocument(doc)
}
