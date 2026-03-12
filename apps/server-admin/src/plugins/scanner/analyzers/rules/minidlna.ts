import { AnalyzerRule } from "../../types";

export const miniDLNARule: AnalyzerRule = {
  name: "miniDLNA",
  label: 'miniDLNA',
  rule: {
    path: '/dlna',
    rewrite_html: true,
    use_auth: true,
    use_root_mode: false,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<TITLE>MiniDLNA");
  },
};