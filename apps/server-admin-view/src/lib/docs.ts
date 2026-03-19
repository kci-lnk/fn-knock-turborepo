const DOCS_BASE_URL = "https://docs.fnknock.cn";

const toDocsUrl = (path: string) =>
  `${DOCS_BASE_URL}${path.startsWith("/") ? path : `/${path}`}`;

export const docsUrls = {
  runModes: {
    direct: toDocsUrl("/quick-start/direct-mode"),
    reverse: toDocsUrl("/quick-start/reverse-proxy-mode"),
    subdomain: toDocsUrl("/quick-start/subdomain-mode"),
  },
  guides: {
    auth: toDocsUrl("/guide/auth"),
    ddns: toDocsUrl("/guide/ddns"),
    security: toDocsUrl("/guide/security"),
    sessionManagement: toDocsUrl("/guide/session-management"),
    ssl: toDocsUrl("/guide/ssl"),
    whitelist: toDocsUrl("/guide/whitelist"),
    reverseProxy: toDocsUrl("/guide/reverse-proxy"),
    subdomainProxy: toDocsUrl("/guide/subdomain-proxy"),
    tunnel: toDocsUrl("/guide/tunnel"),
    fnosShareBypass: toDocsUrl("/guide/fnos-share-bypass"),
    requestLogs: toDocsUrl("/guide/request-logs"),
  },
} as const;
