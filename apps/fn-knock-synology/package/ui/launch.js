(function () {
  'use strict';

  var cookieName = 'fn_knock_synotoken_stage';
  var packagePath = '/webman/3rdparty/fn-knock-synology/';
  var secureAttribute = window.location.protocol === 'https:' ? '; Secure' : '';
  var token = '';
  var decodedToken = '';

  try {
    if (window.opener &&
        window.opener.SYNO &&
        window.opener.SYNO.SDS &&
        window.opener.SYNO.SDS.Session) {
      token = String(window.opener.SYNO.SDS.Session.SynoToken || '');
    }
  } catch (error) {
    token = '';
  }

  if (!token) {
    document.getElementById('launch-status').textContent =
      '无法读取 DSM 会话，请从 DSM 桌面重新打开“敲门 knock”。';
    return;
  }

  try {
    decodedToken = decodeURIComponent(token);
  } catch (error) {
    decodedToken = token;
  }

  document.cookie = cookieName + '=' + encodeURIComponent(decodedToken) +
    '; Path=' + packagePath + secureAttribute + '; SameSite=Strict';
  window.location.replace(packagePath + 'index.cgi/?fn_knock_auth_bootstrap=1');
}());
