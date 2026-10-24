import assert from "node:assert/strict";
import { writeFile, rm, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { chromium } from "playwright";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const app = path.join(root, "apps/server-admin-view");
const fixture = path.join(app, "src/.ddns-proxy-browser-fixture.ts");
const htmlFixture = path.join(app, ".ddns-proxy-browser.html");
const output =
  process.env.DDNS_BROWSER_OUTPUT || "/tmp/fn-knock-ddns-proxy-browser";
const locale = process.env.DDNS_BROWSER_LOCALE === "en" ? "en" : "zh-CN";
let server;
let browser;
try {
  await mkdir(output, { recursive: true });
  await writeFile(
    fixture,
    `
import { createApp, defineComponent, ref } from 'vue/dist/vue.esm-bundler.js';
import { createI18n } from 'vue-i18n';
import { zhCNAdmin } from '../../../packages/i18n/src/messages/admin/zh-CN';
import { enAdmin } from '../../../packages/i18n/src/messages/admin/en';
import DDNSPublicCheckDialog from './views/ddns-management/DDNSPublicCheckDialog.vue';
import DDNSUpdateProxyModeField from './views/ddns-management/DDNSUpdateProxyModeField.vue';
import DDNSProviderCredentialsFields from './views/ddns-management/DDNSProviderCredentialsFields.vue';
import DDNSTargetProviderFields from './views/ddns-management/DDNSTargetProviderFields.vue';
import { createUpdateProxyDraft } from './views/ddns-management/ddns-proxy-model';
import './assets/index.css';
const App = defineComponent({
  components: { DDNSPublicCheckDialog, DDNSUpdateProxyModeField, DDNSProviderCredentialsFields, DDNSTargetProviderFields },
  setup() {
    const proxy = ref(createUpdateProxyDraft({enabled:true,url:'https://' + 'proxy-'.repeat(25) + 'example.com:7890',username:'demo',passwordConfigured:true}));
    const provider = {name:'fixture',label:'Example DNS',fields:[{key:'zone',label:'Zone ID',type:'text',required:true}]};
    const credentials = {providerDef:provider,providerConfig:{zone:'example-zone'},fieldVisibility:{},formatDomainField:()=>{},enableFieldEditing:()=>{},getFieldAutocomplete:()=> 'off',getFieldDescription:()=> 'DNS zone identifier',getFieldDomId:()=> 'primary-reference',getFieldInputName:()=> 'zone',isFieldEditReady:()=>true,setFieldValue:()=>{},toggleFieldVisibility:()=>{}};
    const targetCredentials = {providerDef:provider,state:{config:{zone:'example-zone'}},formatDomainField:()=>{},getFieldAutocomplete:()=> 'off',getFieldDescription:()=> 'DNS zone identifier',isFieldVisible:()=>false,toggleFieldVisibility:()=>{}};
    return { credentials, targetCredentials, proxy, open: ref(true), sources: ref({ipv4:['https://example.com/ip'],ipv6:['https://example.com/ipv6']}), transport:ref('node'), dns:ref('none'), primary:ref('inherit'), target:ref('proxy'),
      results: [{family:'ipv4',url:'https://'+'very-long-domain-'.repeat(16)+'example.com/ip',success:false,error:'连接失败：'+'检测错误说明'.repeat(30)}] };
  },
  template: \`<main class="mx-auto max-w-7xl p-4"><h1>DDNS</h1><button id="open-settings" @click="open=true">网络设置</button>
    <section id="primary-form" class="my-4 divide-y rounded-lg border">
    <DDNSUpdateProxyModeField id="primary-mode" v-model="primary" :default-enabled="proxy.enabled" :configured="true" />
    <DDNSProviderCredentialsFields v-bind="credentials" />
    </section>
    <section id="target-form" class="my-4 max-w-[712px] divide-y rounded-lg border">
    <DDNSUpdateProxyModeField id="target-mode" layout="target" v-model="target" :default-enabled="proxy.enabled" :configured="true" />
    <DDNSTargetProviderFields v-bind="targetCredentials" />
    </section>
    <DDNSPublicCheckDialog v-model:open="open" v-model:draft="sources" v-model:http-transport-draft="transport" v-model:public-dns-provider-draft="dns" v-model:update-proxy-draft="proxy" :is-saving="false" :is-testing="false" :test-results="results" @save="open=false" />
  </main>\`
});
createApp(App).use(createI18n({legacy:false,locale:'${locale}',messages:{en:{admin:enAdmin,common:{cancel:'Cancel',save:'Save'}},'zh-CN':{admin:zhCNAdmin,common:{cancel:'取消',save:'保存'}}}})).mount('#app');
`,
  );
  server = await createServer({
    root: app,
    configFile: path.join(app, "vite.config.ts"),
    server: { host: "127.0.0.1", port: 0, hmr: false },
  });
  await writeFile(
    htmlFixture,
    '<html lang="zh-CN"><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module" src="/src/.ddns-proxy-browser-fixture.ts"></script></body></html>',
  );
  await server.listen();
  const port = server.httpServer.address().port;
  browser = await chromium.launch({ headless: true });
  const errors = [];
  const page = await browser.newPage({ reducedMotion: "reduce" });
  page.setDefaultTimeout(10000);
  page.on("pageerror", (error) => errors.push(error.message));
  await page.route("**/ddns/update-proxy/test", async (route) => {
    assert.equal(route.request().method(), "POST");
    const body = route.request().postDataJSON();
    assert.equal(body.updateProxy.password, "");
    assert.equal(body.httpTransport, "node");
    await route.fulfill({
      json: {
        data: {
          success: true,
          elapsedMs: 42,
          status: 200,
          error: null,
          url: "https://www.cloudflare.com/cdn-cgi/trace",
        },
      },
    });
  });
  const results = [];
  for (const [width, height] of [
    [375, 812],
    [390, 844],
    [768, 1024],
    [1440, 1000],
    [667, 375],
  ]) {
    for (const theme of ["light", "dark"]) {
      await page.setViewportSize({ width, height });
      await page.goto(`http://127.0.0.1:${port}/.ddns-proxy-browser.html`);
      await page.evaluate(
        (dark) => document.documentElement.classList.toggle("dark", dark),
        theme === "dark",
      );
      await page.getByRole("dialog").waitFor();
      await page.locator("#ddns-update-proxy-url").waitFor();
      const dialog = page.getByRole("dialog");
      await page.evaluate(async () => {
        await Promise.all(
          document
            .getAnimations()
            .filter((a) => a.effect?.getTiming().iterations !== Infinity)
            .map((a) => a.finished.catch(() => {})),
        );
      });
      const geometry = await dialog.evaluate((el) => {
        const rect = el.getBoundingClientRect();
        return {
          left: rect.left,
          right: rect.right,
          top: rect.top,
          bottom: rect.bottom,
          scrollWidth: el.scrollWidth,
          clientWidth: el.clientWidth,
        };
      });
      assert(
        geometry.left >= 0 &&
          geometry.right <= width + 1 &&
          geometry.top >= 0 &&
          geometry.bottom <= height + 1,
        JSON.stringify({ width, height, theme, geometry }),
      );
      assert(
        geometry.scrollWidth <= geometry.clientWidth + 1,
        "dialog overflow",
      );
      const save = page.getByRole("button", {
        name: locale === "en" ? "Save" : "保存",
        exact: true,
      });
      const saveBox = await save.boundingBox();
      assert(
        saveBox.y + saveBox.height <= height && saveBox.y >= 0,
        "save offscreen",
      );
      await page.screenshot({
        animations: "disabled",
        path: path.join(output, `settings-${width}-${height}-${theme}.png`),
      });
      await page
        .getByRole("button", {
          name: locale === "en" ? "Test proxy" : "测试代理",
          exact: true,
        })
        .click();
      await page
        .getByRole("status")
        .filter({
          hasText: locale === "en" ? "Proxy connected" : "代理连接成功",
        })
        .waitFor();
      await page.screenshot({
        animations: "disabled",
        path: path.join(output, `proxy-test-${width}-${height}-${theme}.png`),
      });
      await page
        .locator("#ddns-update-proxy-url")
        .fill("http://localhost:7890");
      assert.equal(
        await page
          .getByRole("status")
          .filter({
            hasText: locale === "en" ? "Proxy connected" : "代理连接成功",
          })
          .count(),
        0,
      );
      // Scroll long detection errors; actions must remain visible and no content may overflow.
      const scroller = dialog.locator(".overflow-y-auto");
      await scroller.evaluate((el) => {
        el.scrollTop = el.scrollHeight;
      });
      const widths = await scroller.evaluate((el) => ({
        client: el.clientWidth,
        scroll: el.scrollWidth,
      }));
      assert(widths.scroll <= widths.client + 1, JSON.stringify(widths));
      await page.screenshot({
        animations: "disabled",
        path: path.join(output, `results-${width}-${height}-${theme}.png`),
      });
      if (width < 640) {
        await page.locator("#ddns-update-proxy-url").focus();
        await page.setViewportSize({ width, height: 480 });
        await page.evaluate(async () => {
          await Promise.all(
            document
              .getAnimations()
              .filter((a) => a.effect?.getTiming().iterations !== Infinity)
              .map((a) => a.finished.catch(() => {})),
          );
        });
        await page.screenshot({
          animations: "disabled",
          path: path.join(output, `keyboard-${width}-${height}-${theme}.png`),
        });
        const keyboardSave = await save.boundingBox();
        assert(
          keyboardSave.y >= 0 && keyboardSave.y + keyboardSave.height <= 481,
          JSON.stringify({
            message: "save obscured in reduced mobile viewport",
            width,
            height,
            theme,
            keyboardSave,
          }),
        );
        await page.setViewportSize({ width, height });
      }
      await page.keyboard.press("Escape");
      await dialog.waitFor({ state: "hidden" });
      for (const [modeId, referenceId] of [
        ["primary-mode", "primary-reference"],
        ["target-mode", "ddns-target-field-zone"],
      ]) {
        const modeBox = await page.locator(`#${modeId}`).boundingBox();
        const referenceBox = await page
          .locator(`#${referenceId}`)
          .boundingBox();
        const labelBox = await page
          .locator(`label[for="${modeId}"]`)
          .boundingBox();
        assert(
          Math.abs(modeBox.x - referenceBox.x) <= 1,
          `${modeId} field alignment`,
        );
        assert(
          Math.abs(modeBox.width - referenceBox.width) <= 1,
          `${modeId} field width`,
        );
        assert(modeBox.width <= 449, `${modeId} overly wide`);
        if (width >= 640)
          assert(labelBox.x < modeBox.x, `${modeId} label not in left column`);
        else assert(labelBox.y < modeBox.y, `${modeId} mobile label order`);
      }
      await page.screenshot({
        animations: "disabled",
        path: path.join(output, `routing-form-${width}-${height}-${theme}.png`),
      });
      await page.locator("#primary-mode").click();
      await page
        .getByRole("option", {
          name: locale === "en" ? "Direct" : "直连",
          exact: true,
        })
        .click();
      assert(
        (await page.locator("#primary-mode").textContent()).includes(
          locale === "en" ? "Direct" : "直连",
        ),
      );
      await page.locator("#target-mode").click();
      await page
        .getByRole("option", {
          name:
            locale === "en"
              ? "Follow global (Use proxy)"
              : "跟随全局（使用代理）",
          exact: true,
        })
        .click();
      assert(
        (await page.locator("#target-mode").textContent()).includes(
          locale === "en" ? "Follow global" : "跟随全局",
        ),
      );
      await page.locator("#open-settings").click();
      await page.getByRole("dialog").waitFor();
      await page.keyboard.press("Tab");
      assert(
        await page
          .getByRole("dialog")
          .evaluate((el) => el.contains(document.activeElement)),
        "focus escaped modal",
      );
      results.push({ width, height, theme, locale, passed: true });
    }
  }
  assert.deepEqual(errors, []);
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(
    `DDNS proxy browser checks passed for ${results.length} viewport/theme combinations. Screenshots: ${output}`,
  );
} finally {
  await browser?.close();
  await server?.close();
  await rm(fixture, { force: true });
  await rm(htmlFixture, { force: true });
}
