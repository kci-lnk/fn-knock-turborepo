'use strict';
'require view';
'require uci';
'require form';
'require fs';
'require ui';
'require dom';
'require rpc';

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

var getPendingChanges = rpc.declare({
	object: 'uci',
	method: 'changes',
	reject: true
});

var FIREWALL_HELPER = '/usr/libexec/fn-knock-firewall';
var firewallErrors = {
	unavailable: '当前系统缺少所需的 OpenWrt 防火墙服务或工具。',
	session_failed: '无法建立独立的防火墙配置会话。',
	config_unavailable: '无法读取已提交的服务或防火墙配置。',
	pending_firewall: '存在未提交的防火墙修改，请先在防火墙页面或 CLI 中应用或撤销，再点击放行。',
	invalid_port: '网关端口无效，请先保存并应用有效端口。',
	port_changed: '已提交的网关端口已变化，请刷新页面后重试。',
	invalid_zone: '所选防火墙区域不存在，请刷新页面重新选择。',
	conflict: '同名规则不属于敲门 Knock，或包含额外限制，请先在防火墙页面检查。',
	busy: '另一个放行操作正在执行，请稍后重试；若持续出现，请检查 /var/run/fn-knock-firewall.lock.d 残留锁。',
	write_failed: '防火墙规则写入失败，未提交配置。',
	commit_failed: '防火墙配置提交失败，请检查后重试。',
	reload_failed: '规则已保存，但防火墙重载失败，请检查防火墙配置后再次点击重试。'
};

function readFirewallResult(result) {
	var value;
	try { value = JSON.parse(result.stdout || '{}'); }
	catch (e) { throw new Error('无法解析防火墙操作结果。'); }
	if (result.code !== 0 || !value.state)
		throw new Error(firewallErrors[value.state] || '防火墙操作失败，请检查系统日志。');
	return value;
}

var servicePortLabels = {
	admin_view_port: '管理后台端口',
	backend_port: '内部后端 API 端口',
	auth_port: '认证服务端口',
	go_backend_port: '网关内部 gRPC 端口',
	go_reproxy_port: '网关代理端口'
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
			L.resolveDefault(fs.exec('/etc/init.d/fn-knock', [ 'status' ]), null),
			fs.exec(FIREWALL_HELPER, [ 'status' ]).then(readFirewallResult).catch(function(err) {
				return { error: err.message || String(err), zones: [] };
			})
		]);
	},

	render: function(data) {
		var status = data && data[1] ? data[1] : null;
		var running = status && status.code === 0;
		var port = optionValue('admin_view_port', '7991');
		var targetUrl = buildAdminUrl(port);
		var m, s, o;
		var firewall = data && data[2];

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

		var zones = firewall && Array.isArray(firewall.zones) ? firewall.zones : [];
		var defaultZone = zones.indexOf(firewall && firewall.source_zone) !== -1
			? firewall.source_zone : zones.indexOf('wan') !== -1 ? 'wan' : '';
		var zoneOption = s.option(form.ListValue, '_firewall_zone', '防火墙来源区域');
		zoneOption.value('', '请选择来源区域');
		zones.forEach(function(zone) { zoneOption.value(zone, zone); });
		zoneOption.cfgvalue = function() { return defaultZone; };
		// This is an action parameter, never a saved fn-knock option.
		zoneOption.write = function() {};
		zoneOption.remove = function() {};
		zoneOption.rmempty = true;

		o = s.option(form.DummyValue, '_firewall_info', '网关防火墙');
		o.cfgvalue = function() {
			if (!firewall) return '无法读取防火墙配置，当前系统可能缺少所需服务或工具。';
			if (firewall.error) return '无法读取防火墙配置：' + firewall.error;
			var text = '手动放行 TCP ' + firewall.port + '（IPv4/IPv6），规则重启后保留。修改端口后需再次点击；撤销请到防火墙页面删除规则。';
			if (firewall.state === 'conflict') return text + ' ' + firewallErrors.conflict;
			if (firewall.state === 'configured')
				text += ' 已保存规则：' + firewall.source_zone + ' / TCP ' + firewall.configured_port + '（不代表实时连通状态）。';
			return text;
		};

		o = s.option(form.Button, '_open_firewall', '手动放行');
		o.inputtitle = '放行防火墙';
		o.inputstyle = 'action';
		o.readonly = !firewall || !zones.length || firewall.state === 'conflict' || m.readonly;
		o.onclick = function(ev, sectionId) {
			var button = ev.currentTarget;
			var zone = zoneOption.formvalue(sectionId);
			button.disabled = true;
			return Promise.resolve().then(function() {
				if (!zone) throw new Error('请先选择防火墙来源区域。');
				var currentPort = s.getOption('go_reproxy_port').formvalue(sectionId);
				if (!/^[0-9]+$/.test(String(currentPort)) || Number(currentPort) !== Number(firewall.port))
					throw new Error('请先保存并应用网关端口，然后刷新页面再放行。');
				return getPendingChanges();
			}).then(function(response) {
				var changes = response && response.changes;
				if (!changes || typeof changes !== 'object' || Array.isArray(changes))
					throw new Error('无法确认待应用配置，请刷新页面后重试。');
				if (changes['fn-knock'] && changes['fn-knock'].length)
					throw new Error('敲门 Knock 配置尚未应用，请先保存并应用，再刷新页面放行。');
				if (changes.firewall && changes.firewall.length)
					throw new Error(firewallErrors.pending_firewall);
				return fs.exec(FIREWALL_HELPER, [ 'allow', zone, firewall.port ]);
			}).then(readFirewallResult).then(function(result) {
				ui.addNotification(null, E('p', {}, [
					'已保存并重载防火墙：' + result.source_zone + ' → 本机 TCP ' + result.port + '（IPv4/IPv6）。'
				]), 'info');
			}).catch(function(err) {
				ui.addNotification(null, E('p', {}, [ err.message || String(err) ]), 'danger');
			}).then(function() { button.disabled = false; });
		};

		o = s.option(form.Flag, 'enabled', '启用服务');
		o.default = o.enabled;
		o.rmempty = false;

		addPortOption(s, 'admin_view_port', '管理后台端口', '7991', '从 LuCI 打开敲门 Knock 管理后台时使用的公网 Web 端口。');
		addPortOption(s, 'go_reproxy_port', '网关代理端口', '7999', 'Go 网关对外提供服务的代理端口。');

		addPortOption(s, 'backend_port', '内部后端 API 端口', '17998', '绑定到 127.0.0.1 的内部 Rust 后端 API 端口。');
		addPortOption(s, 'auth_port', '认证服务端口', '7997', '绑定到 127.0.0.1 的内部认证服务端口。');
		addPortOption(s, 'go_backend_port', '网关内部 gRPC 端口', '7996', '绑定到 127.0.0.1 的内部 Go gRPC 端口。');

		o = s.option(form.Value, 'admin_view_host', '管理后台监听地址');
		o.placeholder = '0.0.0.0';
		o.datatype = 'ipaddr';
		o.rmempty = false;

		o = s.option(form.Value, 'data_dir', '数据目录');
		o.placeholder = '/etc/fn-knock/data';
		o.rmempty = false;

		o = s.option(form.Value, 'gateway_config_dir', '网关配置目录');
		o.placeholder = '/etc/fn-knock/gateway';
		o.rmempty = false;

		return m.render();
	}
});
