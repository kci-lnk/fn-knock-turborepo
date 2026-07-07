'use strict';
'require view';
'require uci';
'require form';
'require fs';
'require ui';
'require dom';

function formatHost(hostname) {
	if (!hostname)
		return window.location.hostname;

	return hostname.indexOf(':') !== -1 ? '[' + hostname + ']' : hostname;
}

function buildAdminUrl(port) {
	return 'http://' + formatHost(window.location.hostname) + ':' + port + '/';
}

function optionValue(name, fallback) {
	return uci.get('fn-knock', 'main', name) || fallback;
}

var OFFICIAL_SITE_URL = 'https://www.fnknock.cn/';
var DOCUMENTATION_URL = 'https://docs.fnknock.cn/';

var servicePortLabels = {
	admin_view_port: '管理后台端口',
	backend_port: '内部后端 API 端口',
	auth_port: '认证服务端口',
	go_backend_port: '网关管理 API 端口',
	go_reproxy_port: '网关代理端口',
	redis_port: 'Redis 端口'
};

function buildExternalLinks() {
	return [
		'<a class="cbi-button cbi-button-action" href="%h" target="_blank" rel="noreferrer noopener">官网</a>'.format(OFFICIAL_SITE_URL),
		'<a class="cbi-button cbi-button-neutral" href="%h" target="_blank" rel="noreferrer noopener">文档站点</a>'.format(DOCUMENTATION_URL)
	].join(' ');
}

function validateServicePort(sectionId, value) {
	var ports = {};
	var names = Object.keys(servicePortLabels);
	var name, option, port, owner;

	for (var i = 0; i < names.length; i++) {
		name = names[i];
		option = this.section.getOption(name);
		port = name === this.option ? value : option ? option.formvalue(sectionId) : optionValue(name, '');

		if (!port)
			continue;

		owner = ports[port];
		if (owner)
			return '端口 %s 已被 %s 使用。'.format(port, servicePortLabels[owner]);

		ports[port] = name;
	}

	return true;
}

function addPortOption(section, name, title, placeholder, description) {
	var option = section.option(form.Value, name, title, description);
	option.datatype = 'port';
	option.placeholder = placeholder;
	option.rmempty = false;
	option.validate = validateServicePort;
	return option;
}

function saveAndApply(mode) {
	var tasks = [];

	document.getElementById('maincontent').querySelectorAll('.cbi-map').forEach(function(map) {
		tasks.push(dom.callClassMethod(map, 'save'));
	});

	return Promise.all(tasks).then(function() {
		ui.changes.apply(mode == '0');
	}).then(function() {
		ui.addNotification(null, E('p', {}, [ '敲门 Knock 配置已提交，服务会在配置写入后自动重载。' ]), 'info');
	}).catch(function(err) {
		ui.addNotification(null, E('p', {}, [ '应用敲门 Knock 配置失败：%s'.format(err.message || err) ]), 'danger');
		throw err;
	});
}

return view.extend({
	handleSave: null,

	handleSaveApply: function(ev, mode) {
		return saveAndApply(mode);
	},

	load: function() {
		return Promise.all([
			L.resolveDefault(uci.load('fn-knock'), null),
			L.resolveDefault(fs.exec('/etc/init.d/fn-knock', [ 'status' ]), null)
		]);
	},

	render: function(data) {
		var status = data && data[1] ? data[1] : null;
		var running = status && status.code === 0;
		var port = optionValue('admin_view_port', '7991');
		var targetUrl = buildAdminUrl(port);
		var m, s, o;

		m = new form.Map('fn-knock', '敲门 Knock', '配置 OpenWrt 上的敲门 Knock 服务端口，并打开管理后台。');

		s = m.section(form.NamedSection, 'main', 'fn_knock', '服务');
		s.addremove = false;

		o = s.option(form.DummyValue, '_status', '服务状态');
		o.cfgvalue = function() {
			return running ? '运行中' : '已停止';
		};

		o = s.option(form.DummyValue, '_admin_url', '管理后台地址');
		o.cfgvalue = function() {
			return targetUrl;
		};
		o.rawhtml = true;
		o.textvalue = function() {
			return '<a href="%h" target="_self" rel="noreferrer">%h</a>'.format(targetUrl, targetUrl);
		};

		o = s.option(form.Button, '_open_admin', '管理后台');
		o.inputtitle = '打开管理后台';
		o.inputstyle = 'action';
		o.onclick = function() {
			window.location.href = buildAdminUrl(optionValue('admin_view_port', '7991'));
			return false;
		};

		o = s.option(form.DummyValue, '_links', '相关链接');
		o.rawhtml = true;
		o.cfgvalue = buildExternalLinks;
		o.textvalue = buildExternalLinks;

		o = s.option(form.Flag, 'enabled', '启用服务');
		o.default = o.enabled;
		o.rmempty = false;

		addPortOption(s, 'admin_view_port', '管理后台端口', '7991', '从 LuCI 打开敲门 Knock 管理后台时使用的公网 Web 端口。');
		addPortOption(s, 'go_reproxy_port', '网关代理端口', '7999', 'Go 网关对外提供服务的代理端口。');

		addPortOption(s, 'backend_port', '内部后端 API 端口', '17998', '绑定到 127.0.0.1 的内部 Rust 后端 API 端口。');
		addPortOption(s, 'auth_port', '认证服务端口', '7997', '绑定到 127.0.0.1 的内部认证服务端口。');
		addPortOption(s, 'go_backend_port', '网关管理 API 端口', '7996', '绑定到 127.0.0.1 的内部 Go 网关管理 API 端口。');

		o = s.option(form.Value, 'admin_view_host', '管理后台监听地址');
		o.placeholder = '0.0.0.0';
		o.datatype = 'ipaddr';
		o.rmempty = false;

		o = s.option(form.Value, 'redis_host', 'Redis 地址');
		o.placeholder = '127.0.0.1';
		o.datatype = 'host';
		o.rmempty = false;

		addPortOption(s, 'redis_port', 'Redis 端口', '6379', '敲门 Knock 连接 Redis 服务时使用的端口。');

		o = s.option(form.Value, 'redis_password', 'Redis 密码');
		o.password = true;
		o.rmempty = true;

		o = s.option(form.Value, 'data_dir', '数据目录');
		o.placeholder = '/var/lib/fn-knock';
		o.rmempty = false;

		o = s.option(form.Value, 'gateway_config_dir', '网关配置目录');
		o.placeholder = '/etc/fn-knock/gateway';
		o.rmempty = false;

		return m.render();
	}
});
