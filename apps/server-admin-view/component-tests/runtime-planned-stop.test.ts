import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import RuntimeComponentCard from "../src/views/event-center/RuntimeComponentCard.vue";
import type { RuntimeComponentHealth } from "../src/types";
import { zhCNAdmin } from "../../../packages/i18n/src/messages/admin/zh-CN";

const component: RuntimeComponentHealth = {
  id: "gateway_process",
  status: "healthy",
  process_state: "running",
  version: "2.4.11",
  commit: null,
  pid: 3307,
  instance_id: "gateway-one",
  started_at: null,
  uptime_ms: 100,
  last_checked_at: null,
  last_success_at: null,
  consecutive_failures: 0,
  reason_code: "serving",
};
const lifecycle = {
  phase: "stopping",
  operation_id: "stop-one",
  reason: "platform_stop",
  requested_at: "2026-09-09T23:40:13Z",
  deadline_at: "2026-09-09T23:43:13Z",
  management_instance: "manager-one",
  gateway_instance: "gateway-one",
};
function render(value: RuntimeComponentHealth) {
  return mount(RuntimeComponentCard, {
    props: { component: value, variant: "service" },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "zh-CN",
          messages: { "zh-CN": { admin: zhCNAdmin } },
        }),
      ],
    },
  });
}
describe("planned stop presentation", () => {
  it("shows stopping instead of stale healthy status", () => {
    const wrapper = render({ ...component, lifecycle });
    expect(wrapper.text()).toContain("正在停止");
    expect(wrapper.text()).not.toContain("健康");
    wrapper.unmount();
  });
  it("retains an existing unhealthy warning alongside stopping", () => {
    const wrapper = render({ ...component, status: "unhealthy", lifecycle });
    expect(wrapper.text()).toContain("正在停止");
    expect(wrapper.text()).toContain("异常");
    wrapper.unmount();
  });
  it("continues to render older snapshots without lifecycle", () => {
    const wrapper = render(component);
    expect(wrapper.text()).toContain("健康");
    expect(wrapper.text()).not.toContain("正在停止");
    wrapper.unmount();
  });
});
