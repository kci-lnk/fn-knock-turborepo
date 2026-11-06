import { flushPromises, mount } from "@vue/test-utils";
import { ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import { AcmeAPI, type AcmePreflight } from "../src/lib/api/acme";
import AcmeDnsReview from "../src/views/ssl-settings/AcmeDnsReview.vue";
import type { AcmeCertificateController } from "../src/views/ssl-settings/acme-certificate-contract";

const report: AcmePreflight = {
  applicationId: "app",
  checkedAt: "2026-01-01T00:00:00Z",
  snapshot: "token",
  status: "review",
  cooldown: null,
  renewalPending: false,
  groups: [
    {
      name: "_acme-challenge.example.test",
      domains: ["example.test", "*.example.test"],
      status: "existing",
      zone: "example.test",
      values: ["one", "two", "three"],
      selectable: ["one", "two", "three"],
      delegation: [],
    },
  ],
};
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => {
  wrappers.splice(0).forEach((w) => w.unmount());
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});
function setup(current: AcmePreflight) {
  const controller = {
    dnsReport: ref(current),
    checkingDns: ref(false),
    fetchOverview: vi.fn(),
    requestCertificate: vi.fn(),
    showDnsError: vi.fn(),
    openEditDialog: vi.fn(),
    t: (key: string) =>
      enAdmin.acmeDns[key.split(".").pop() as keyof typeof enAdmin.acmeDns],
  };
  wrappers.push(
    mount(AcmeDnsReview, {
      props: { controller: controller as unknown as AcmeCertificateController },
      attachTo: document.body,
    }),
  );
  return controller;
}
function button(label: string) {
  return [...document.querySelectorAll("button")].find(
    (b) => b.textContent?.trim() === label,
  );
}
describe("ACME DNS review", () => {
  it("requires explicit selection and confirmation and never issues after deleting", async () => {
    const controller = setup(report);
    await flushPromises();
    const boxes = [
      ...document.querySelectorAll<HTMLInputElement>("input[type=checkbox]"),
    ];
    expect(boxes.map((b) => b.checked)).toEqual([false, false, false]);
    boxes[1]!.click();
    await flushPromises();
    const remove = vi.spyOn(AcmeAPI, "deleteTxt").mockResolvedValue({
      results: [
        { name: report.groups[0]!.name, value: "two", outcome: "deleted" },
      ],
      report: {
        ...report,
        snapshot: "next",
        status: "blocked",
        groups: [{ ...report.groups[0]!, status: "propagating" }],
      },
    });
    button(enAdmin.acmeDns.remove)!.click();
    await flushPromises();
    expect(remove).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain(enAdmin.acmeDns.impact);
    button(enAdmin.acmeDns.remove)!.click();
    await flushPromises();
    expect(remove).toHaveBeenCalledWith(report, [
      { name: report.groups[0]!.name, value: "two" },
    ]);
    expect(document.body.textContent).toContain(enAdmin.acmeDns.propagation);
    expect(controller.requestCertificate).not.toHaveBeenCalled();
    expect(button(enAdmin.acmeDns.keep)).toBeUndefined();
  });
  it("retries only the failed value after partial cleanup", async () => {
    setup(report);
    await flushPromises();
    const [first, second] = document.querySelectorAll<HTMLInputElement>(
      "input[type=checkbox]",
    );
    first!.click();
    second!.click();
    await flushPromises();
    const next = {
      ...report,
      snapshot: "partial",
      status: "blocked",
      groups: [
        {
          ...report.groups[0]!,
          status: "propagating",
          selectable: ["two", "three"],
        },
      ],
    };
    const remove = vi.spyOn(AcmeAPI, "deleteTxt").mockResolvedValue({
      results: [
        { name: report.groups[0]!.name, value: "one", outcome: "deleted" },
        { name: report.groups[0]!.name, value: "two", outcome: "failed" },
      ],
      report: next,
    });
    for (let i = 0; i < 4; i++) {
      button(enAdmin.acmeDns.remove)!.click();
      await flushPromises();
    }
    expect(remove.mock.calls[1]).toEqual([
      next,
      [{ name: report.groups[0]!.name, value: "two" }],
    ]);
  });
  it("continues once with the selected application's snapshot", async () => {
    const controller = setup(report);
    await flushPromises();
    button(enAdmin.acmeDns.keep)!.click();
    expect(controller.requestCertificate).toHaveBeenCalledWith("app", "token");
  });
  it("lets users recheck delegation without exposing a delete action", async () => {
    const delegated = {
      ...report,
      status: "blocked",
      groups: [
        {
          ...report.groups[0]!,
          status: "delegated",
          values: [],
          selectable: [],
          delegation: ["CNAME validation.other.test."],
        },
      ],
    };
    const controller = setup(delegated);
    await flushPromises();
    expect(document.querySelector("input[type=checkbox]")).toBeNull();
    expect(button(enAdmin.acmeDns.keep)).toBeUndefined();
    vi.spyOn(AcmeAPI, "checkDns").mockResolvedValue({
      ...report,
      status: "ready",
    });
    button(enAdmin.acmeDns.recheck)!.click();
    await flushPromises();
    expect(button(enAdmin.acmeDns.proceed)).toBeTruthy();
    expect(controller.requestCertificate).not.toHaveBeenCalled();
  });
});
