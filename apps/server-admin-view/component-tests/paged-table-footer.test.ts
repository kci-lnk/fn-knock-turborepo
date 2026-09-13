import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";

describe("shared table pagination", () => {
  it.each([false, true])(
    "renders interactive numbered pages (floating=%s)",
    async (floating) => {
      const wrapper = mount(PagedTableFooter, {
        props: { total: 45, page: 1, limit: "20", itemsPerPage: 20, floating },
        global: {
          plugins: [
            createI18n({
              legacy: false,
              locale: "en",
              missingWarn: false,
              fallbackWarn: false,
            }),
          ],
        },
      });
      const second = wrapper
        .findAll("button")
        .find((button) => button.text() === "2");
      expect(second).toBeDefined();
      await second!.trigger("click");
      expect(wrapper.emitted("update:page")?.at(-1)).toEqual([2]);
      wrapper.unmount();
    },
  );
});
