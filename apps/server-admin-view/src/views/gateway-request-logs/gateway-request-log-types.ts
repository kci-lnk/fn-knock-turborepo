export type GatewayLogTranslator = (
  key: string,
  params?: Record<string, unknown>,
) => string;
