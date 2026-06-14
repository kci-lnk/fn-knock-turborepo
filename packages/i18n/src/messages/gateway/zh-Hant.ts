import { zhCNGateway } from "./zh-CN";

export const zhHantGateway = {
  ...zhCNGateway,
  htmlLang: "zh-Hant",
  success: "成功",
  goToSelect: "前往選擇頁",
  welcomeTitle: "服務已啟動",
  welcomeMessage: "歡迎使用 Go Reauth Proxy",
  routeNotFoundTitle: "沒有匹配的路由",
  routeNotFoundMessage: "目前請求未匹配任何已配置的路由。",
  selectTitle: "選擇訪問入口",
  selectDescription: "請選擇一個已配置的代理入口繼續訪問。",
  routesEmpty: "暫無可用路由。",
  logout: "登出",
  logoutConfirmTitle: "登出",
  logoutConfirmMessage: "確定要登出目前登入狀態嗎？",
  noRoutesConfigured: "暫無已配置路由",
  wafBlockedTitle: "請求已攔截",
  wafBlockedMessage: "訪問被安全策略拒絕。",
  wafBlockedJson: "請求已被 WAF 攔截",
};
