import { AnalyzerRule } from "../../types";
import { scannerServiceLabel } from "../labels";

export const lotteryRule: AnalyzerRule = {
  name: "cpzs",
  label: scannerServiceLabel("lottery"),
  rule: {
    path: '/cpzs',
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>彩票助手</title>");
  },
};
