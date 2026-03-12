import { AnalyzerRule } from "../../types";

export const homeAssistantRule: AnalyzerRule = {
  name: "homeassistant",
  label: 'Home Assistant',
  rule: {
    path: '/ha',
    rewrite_html: true,
    use_auth: true,
    use_root_mode: false,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>Home Assistant</title>");
  },
};