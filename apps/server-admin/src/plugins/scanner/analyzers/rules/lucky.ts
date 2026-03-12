import { AnalyzerRule } from "../../types";

export const luckyRule: AnalyzerRule = {
  name: "lucky",
  label: 'Lucky',
  rule: {
    path: '/lucky',
    rewrite_html: true,
    use_auth: true,
    use_root_mode: false,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>Lucky</title>");
  },
};