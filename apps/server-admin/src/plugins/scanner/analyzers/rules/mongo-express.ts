import { AnalyzerRule } from "../../types";

export const mongoExpressRule: AnalyzerRule = {
  name: "mongoexpress",
  label: 'Mongo Express',
  rule: {
    path: '/mongoe',
    rewrite_html: true,
    use_auth: true,
    use_root_mode: false,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    const cookie = result.headers?.["set-cookie"];
    return !!cookie && cookie.includes("mongo-express=");
  },
};