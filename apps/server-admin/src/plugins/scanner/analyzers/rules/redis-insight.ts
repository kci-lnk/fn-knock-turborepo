import { AnalyzerRule } from "../../types";

export const redisInsightRule: AnalyzerRule = {
  name: "redisinsight",
  label: 'Redis Insight',
  rule: {
    path: '/redisi',
    rewrite_html: false,
    use_auth: true,
    use_root_mode: true,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    return !!result.body && result.body.includes("<title>Redis Insight</title>");
  },
};