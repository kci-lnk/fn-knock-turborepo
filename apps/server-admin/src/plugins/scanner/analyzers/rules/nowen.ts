import { AnalyzerRule } from "../../types";

export const nowenRule: AnalyzerRule = {
  name: "nowen",
  label: '星云门户',
  rule: {
    path: '/nowen',
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: '',
  },
  isDefault: true,
  match: (result) => {
    return !!result.body && result.body.includes("<title>Digital Zen Garden</title>");
  },
};