import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';

const root = new URL('../../', import.meta.url);
const read = path => readFileSync(new URL(path, root), 'utf8');
const source = read('deploy/openwrt/www/luci-static/resources/view/fn-knock.js');
const defaultStatus = { state: 'absent', zones: ['lan', 'wan'], port: '7999', source_zone: '' };

function harness(status = defaultStatus) {
  const options = {}, calls = [], notices = [];
  const values = { go_reproxy_port: '7999' };
  let pending = {}, reply = { code: 0, stdout: JSON.stringify({ state: 'applied', port: '7999', source_zone: 'wan' }) };
  const section = {
    option(type, name) {
      const option = { value() {}, formvalue() { return values[name] ?? option.cfgvalue?.(); } };
      options[name] = option;
      return option;
    },
    getOption(name) { return options[name]; }
  };
  const context = {
    view: { extend: obj => obj },
    rpc: { declare: spec => { assert.equal(spec.reject, true); return async () => {
      if (pending instanceof Error) throw pending;
      return { changes: pending };
    }; } },
    uci: { get() {}, load: async () => {}, changes: async () => pending },
    fs: { exec: async (path, args) => { calls.push([path, args]); return args[0] === 'status'
      ? { code: 0, stdout: JSON.stringify(status) } : reply; } },
    ui: { addNotification: (_, node, kind) => notices.push({ text: node.children.join(''), kind }) },
    E: (tag, attrs, children) => ({ children }),
    form: { Map: function() { this.section = () => section; this.render = () => options; } },
    window: { location: { hostname: 'router' } },
    L: { resolveDefault: (promise, fallback) => promise.catch(() => fallback) },
    dom: {}
  };
  const view = vm.runInNewContext('(function(){' + source + '})()', context);
  view.render([null, { code: 0 }, status]);
  return {
    view, options, values, calls, notices,
    pending(value) { pending = value; }, reply(value) { reply = value; },
    async click() {
      const button = { disabled: false };
      const promise = options._open_firewall.onclick({ currentTarget: button }, 'main');
      assert.equal(button.disabled, true);
      await promise;
      assert.equal(button.disabled, false);
    }
  };
}

test('both LuCI entries match; lifecycle has no firewall helper calls', () => {
  assert.equal(source, read('deploy/openwrt/www/luci-static/resources/view/fn-knock-openwrt.js'));
  for (const path of ['etc/init.d/fn-knock', 'etc/config/fn-knock', 'control/postinst', 'control/prerm', 'control/postrm']) {
    assert.doesNotMatch(read('deploy/openwrt/' + path), /fn-knock-firewall|auto_open_firewall/);
  }
  const acl = JSON.parse(read('deploy/openwrt/usr/share/rpcd/acl.d/luci-app-fn-knock.json'))['luci-app-fn-knock'];
  assert.deepEqual(acl.read.file['/usr/libexec/fn-knock-firewall status'], ['exec']);
  assert.deepEqual(acl.write.file['/usr/libexec/fn-knock-firewall allow *'], ['exec']);
  assert.ok(!acl.write.uci.includes('firewall'));
  assert.ok(!Object.keys(acl.read.file).some(key => key.includes('allow')));
});
test('load is read-only; zone selection never writes UCI', async () => {
  const h = harness();
  await h.view.load();
  assert.ok(h.calls.every(([, args]) => args[0] === 'status'));
  assert.equal(h.options._firewall_zone.cfgvalue(), 'wan');
  assert.equal(h.options._firewall_zone.write(), undefined);
  assert.equal(h.options._firewall_zone.remove(), undefined);
});
test('prefer existing region; require a choice without wan', async () => {
  assert.equal(harness({ ...defaultStatus, source_zone: 'lan' }).options._firewall_zone.cfgvalue(), 'lan');
  const h = harness({ ...defaultStatus, zones: ['external'] });
  assert.equal(h.options._firewall_zone.cfgvalue(), '');
  await h.click();
  assert.equal(h.calls.length, 0);
  assert.match(h.notices[0].text, /选择/);
});
test('only explicit click invokes allow with saved port and selected zone', async () => {
  const h = harness();
  h.values._firewall_zone = 'lan';
  await h.click();
  assert.deepEqual(JSON.parse(JSON.stringify(h.calls)), [['/usr/libexec/fn-knock-firewall', ['allow', 'lan', '7999']]]);
  assert.equal(h.notices[0].kind, 'info');
});
test('unsaved port or staged configuration blocks allow', async () => {
  for (const staged of [false, true]) {
    const h = harness();
    if (staged) h.pending({ 'fn-knock': [['set', 'main', 'go_reproxy_port', '8888']] });
    else h.values.go_reproxy_port = '8888';
    await h.click();
    assert.equal(h.calls.length, 0);
    assert.match(h.notices[0].text, /保存并应用/);
  }
});
test('reload failure is distinct and re-enables button for retry', async () => {
  const h = harness();
  h.reply({ code: 1, stdout: '{"state":"reload_failed"}' });
  await h.click();
  assert.match(h.notices[0].text, /已保存.*重载失败/);
  h.reply({ code: 0, stdout: '{"state":"applied","port":"7999","source_zone":"wan"}' });
  await h.click();
  assert.equal(h.notices[1].kind, 'info');
});
test('missing capability, no zones and conflicting rule disable button', () => {
  for (const state of [null, { ...defaultStatus, zones: [] }, { ...defaultStatus, state: 'conflict' }])
    assert.equal(harness(state).options._open_firewall.readonly, true);
});

test('failed or malformed pending query and staged firewall edits block allow', async () => {
  for (const pending of [new Error('RPC failed'), null, [], { firewall: [['add', 'rule']] }]) {
    const h = harness();
    h.pending(pending);
    await h.click();
    assert.equal(h.calls.length, 0);
    assert.equal(h.notices[0].kind, 'danger');
  }
});
test('leading-zero port matches the same committed numeric port', async () => {
  const h = harness();
  h.values.go_reproxy_port = '07999';
  await h.click();
  assert.equal(h.calls.length, 1);
  assert.equal(h.notices[0].kind, 'info');
});
