import { AnalyzerRule } from "../../types";

export const sunPanelRule: AnalyzerRule = {
  name: "sun-panel",
  label: "Sun-Panel",
  rule: {
    path: "/sp",
    rewrite_html: true,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: "",
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>Sun-Panel</title>");
  },
};
