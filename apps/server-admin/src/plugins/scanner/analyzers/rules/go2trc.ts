import { AnalyzerRule } from "../../types";

export const go2rtcRule: AnalyzerRule = {
  name: "go2rtc",
  label: 'Go2RTC',
  rule: {
    path: '/go2rtc',
    rewrite_html: true,
    use_auth: true,
    use_root_mode: false,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>go2rtc</title>");
  },
};