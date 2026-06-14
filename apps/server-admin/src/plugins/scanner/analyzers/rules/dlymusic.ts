import { AnalyzerRule } from "../../types";
import { scannerServiceLabel } from "../labels";

export const dlymusicRule: AnalyzerRule = {
  name: "DLYMusic",
  label: scannerServiceLabel("dlymusic"),
  rule: {
    path: '/music',
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>道理鱼音乐管理</title>");
  },
};
