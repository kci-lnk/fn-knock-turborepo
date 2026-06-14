import { AnalyzerRule } from "../../types";
import { scannerServiceLabel } from "../labels";

export const nowenRule: AnalyzerRule = {
  name: "nowen",
  label: scannerServiceLabel("nowen"),
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
