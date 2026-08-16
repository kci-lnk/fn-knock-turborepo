import { defineAsyncComponent } from "vue";

const asyncSheetComponent = (
  name: "Sheet" | "SheetContent" | "SheetHeader" | "SheetTitle",
) =>
  defineAsyncComponent(
    async () => (await import("@/components/ui/sheet"))[name],
  );

export const ConfirmDangerPopover = defineAsyncComponent(
  () => import("@admin-shared/components/common/ConfirmDangerPopover.vue"),
);
export const ConsoleApplicationBar = defineAsyncComponent(
  () => import("./ConsoleApplicationBar.vue"),
);
export const LayoutLocaleDialog = defineAsyncComponent(
  () => import("./LayoutLocaleDialog.vue"),
);
export const Sheet = asyncSheetComponent("Sheet");
export const SheetContent = asyncSheetComponent("SheetContent");
export const SheetHeader = asyncSheetComponent("SheetHeader");
export const SheetTitle = asyncSheetComponent("SheetTitle");
