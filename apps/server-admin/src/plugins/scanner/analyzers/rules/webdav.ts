import { AnalyzerRule } from "../../types";

export const webdavRule: AnalyzerRule = {
  name: "webdav",
  label: 'WebDAV',
  rule: {
    path: '/webdav',
    rewrite_html: true,
    use_auth: true,
    use_root_mode: false,
    strip_path: true,
    target: '',
  },
  isDefault: false,
  match: (result) => {
    const auth = result.headers?.["www-authenticate"];
    return !!auth && auth.includes('Basic realm="Restricted"') && result.port === 5005;
  },
};