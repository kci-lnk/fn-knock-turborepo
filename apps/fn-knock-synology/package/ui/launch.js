(function () {
  'use strict';

  var cookieName = 'fn_knock_synotoken_stage';
  var packagePath = '/webman/3rdparty/fn-knock-synology/';
  var token = '';

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

  if (token) {
    document.cookie = cookieName + '=' + encodeURIComponent(token) +
      '; Path=' + packagePath + '; Secure; SameSite=Strict';
  }

  window.location.replace(packagePath + 'index.cgi/');
}());
