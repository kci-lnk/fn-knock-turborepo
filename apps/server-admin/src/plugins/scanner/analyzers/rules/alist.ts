import { AnalyzerRule } from "../../types";

export const alistRule: AnalyzerRule = {
  name: "alist",
  label: 'Alist',
  rule: {
    path: '/alist',
    rewrite_html: true,
    use_auth: true,
    use_root_mode: false,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>AList</title>");
  },
};