import assert from "node:assert/strict";
import test from "node:test";
import { localizeProviderDefinition } from "./ddns/catalog";
import { providerDefinitions } from "./ddns/providers";
import { withDDNSLocale } from "./ddns/providers/helpers";

const localizedProvider = (locale: string, name: string) =>
  withDDNSLocale(locale, () =>
    localizeProviderDefinition(
      providerDefinitions.find((provider) => provider.name === name)!,
    ),
  );

const fieldByKey = (
  provider: ReturnType<typeof localizedProvider>,
  key: string,
) => {
  const field = provider.fields.find((item) => item.key === key);
  assert.ok(field, `missing field ${key}`);
  return field;
};

test("DDNS provider catalog localizes credential labels and descriptions", () => {
  const tencentcloud = localizedProvider("zh-CN", "tencentcloud");

  assert.equal(fieldByKey(tencentcloud, "secret_id").label, "SecretId（密钥 ID）");
  assert.match(
    fieldByKey(tencentcloud, "secret_id").description || "",
    /腾讯云 API 访问密钥/,
  );
  assert.equal(fieldByKey(tencentcloud, "secret_key").label, "SecretKey（密钥）");
  assert.match(
    fieldByKey(tencentcloud, "secret_key").description || "",
    /SecretId 配套/,
  );
  assert.equal(fieldByKey(tencentcloud, "domain").description, "要更新的完整域名");
});

test("DDNS provider catalog fills common descriptions for fields without provider fallbacks", () => {
  const dnspod = localizedProvider("zh-CN", "dnspod");
  const huawei = localizedProvider("zh-CN", "huaweicloud");

  assert.equal(fieldByKey(dnspod, "token_id").label, "Token ID（令牌 ID）");
  assert.match(fieldByKey(dnspod, "token_key").description || "", /DNSPod 控制台/);
  assert.equal(fieldByKey(dnspod, "root_domain").description, "用于确定 Zone，例如 example.com");
  assert.equal(fieldByKey(huawei, "access_key_id").label, "访问密钥 ID");
  assert.match(
    fieldByKey(huawei, "secret_access_key").description || "",
    /配套的密钥 Secret/,
  );
});
