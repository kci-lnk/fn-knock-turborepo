import { AnalyzerRule } from "../../types";
import { scannerServiceLabel } from "../labels";

export const fnosRule: AnalyzerRule = {
  name: "fnos",
  label: scannerServiceLabel("fnos"),
  rule: {
    path: '/fnos',
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: '',
  },
  isDefault: true,
  match: (result) => {
    return !!result.body && result.body.includes("<title>飞牛 fnOS</title>");
  },
};
