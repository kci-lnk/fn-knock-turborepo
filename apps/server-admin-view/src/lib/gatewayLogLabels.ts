export const AUTH_DECISION_LABEL_KEYS: Readonly<Record<string, string>> = {
  passed: "admin.gatewayRequestLogs.authDecisions.passed",
  redirected: "admin.gatewayRequestLogs.authDecisions.redirected",
  denied: "admin.gatewayRequestLogs.authDecisions.denied",
  access_denied: "admin.gatewayRequestLogs.authDecisions.accessDenied",
  root_mode_redirect: "admin.gatewayRequestLogs.authDecisions.rootModeRedirect",
  not_required: "admin.gatewayRequestLogs.authDecisions.notRequired",
  proxy: "admin.gatewayRequestLogs.authDecisions.proxy",
  error: "admin.gatewayRequestLogs.authDecisions.error",
  general_blacklist_blocked:
    "admin.gatewayRequestLogs.authDecisions.generalBlacklistBlocked",
  connection_reset: "admin.gatewayRequestLogs.authDecisions.connectionReset",
  subdomain_rule_allowed:
    "admin.gatewayRequestLogs.authDecisions.subdomainRuleAllowed",
  waf_blocked: "admin.gatewayRequestLogs.authDecisions.wafBlocked",
  robots_txt_served: "admin.gatewayRequestLogs.authDecisions.robotsTxtServed",
  crawler_blocked: "admin.gatewayRequestLogs.authDecisions.crawlerBlocked",
  visibility_denied: "admin.gatewayRequestLogs.authDecisions.visibilityDenied",
  fn_app_prompt: "admin.gatewayRequestLogs.authDecisions.fnAppPrompt",
  rate_limited: "admin.gatewayRequestLogs.authDecisions.rateLimited",
  bypassed: "admin.gatewayRequestLogs.authDecisions.bypassed",
  public: "admin.gatewayRequestLogs.authDecisions.public",
  rule_missing: "admin.gatewayRequestLogs.authDecisions.ruleMissing",
};
