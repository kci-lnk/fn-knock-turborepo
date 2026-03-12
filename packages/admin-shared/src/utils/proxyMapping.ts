export type ProxyMappingFields = {
  path: string;
  target: string;
  rewrite_html?: boolean;
  use_auth?: boolean;
  use_root_mode?: boolean;
  strip_path?: boolean;
};

export const DEFAULT_PROXY_MAPPING_FLAGS = {
  rewrite_html: true,
  use_auth: true,
  use_root_mode: false,
  strip_path: true,
} as const;

export const buildProxyMapping = (
  input: ProxyMappingFields,
  defaults: Partial<typeof DEFAULT_PROXY_MAPPING_FLAGS> = DEFAULT_PROXY_MAPPING_FLAGS,
) => {
  return {
    path: input.path.trim(),
    target: input.target.trim(),
    rewrite_html: input.rewrite_html ?? defaults.rewrite_html ?? true,
    use_auth: input.use_auth ?? defaults.use_auth ?? true,
    use_root_mode: input.use_root_mode ?? defaults.use_root_mode ?? false,
    strip_path: input.strip_path ?? defaults.strip_path ?? true,
  };
};
