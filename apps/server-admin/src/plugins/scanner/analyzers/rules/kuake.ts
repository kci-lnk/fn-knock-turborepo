import { AnalyzerRule } from "../../types";
import { scannerServiceLabel } from "../labels";

export const kuakeRule: AnalyzerRule = {
  name: "Kuake",
  label: scannerServiceLabel("kuake"),
  rule: {
    path: '/kuake',
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>登录</title>") && result.port === 5005;
  },
};
