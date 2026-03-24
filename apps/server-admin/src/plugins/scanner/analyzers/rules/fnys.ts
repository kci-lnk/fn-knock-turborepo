import { AnalyzerRule } from "../../types";

export const fnysRule: AnalyzerRule = {
  name: "fnys",
  label: '飞牛影视',
  rule: {
    path: '/v',
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>飞牛影视</title>");
  },
};