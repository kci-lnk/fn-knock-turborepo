import { vueConfig } from "@repo/eslint-config/vue";

export default [
  ...vueConfig,
  {
    files: [
      "src/components/ui/input/Input.vue",
      "src/components/ui/input-group/InputGroupInput.vue",
      "src/components/ui/input-group/InputGroupTextarea.vue",
      "src/components/ui/select/SelectTrigger.vue",
      "src/components/ui/sidebar/SidebarInput.vue",
      "src/components/ui/tags-input/TagsInputInput.vue",
      "src/components/ui/textarea/Textarea.vue",
    ],
    rules: {
      // These primitives forward IDs and accessible-name attributes from their
      // call sites. Usage-level Input/Textarea controls remain linted.
      "project-a11y/form-control-has-accessible-name": "off",
    },
  },
  {
    files: [
      "src/components/ui/button/Button.vue",
      "src/components/ui/input-group/InputGroupButton.vue",
      "src/components/ui/sidebar/SidebarMenuAction.vue",
      "src/components/ui/sidebar/SidebarMenuButton.vue",
    ],
    rules: {
      // Button primitives render caller-provided slots and forward accessible
      // naming attributes. Concrete call sites remain linted.
      "project-a11y/interactive-has-accessible-name": "off",
    },
  },
  {
    files: ["src/components/ui/label/Label.vue"],
    rules: {
      // The primitive forwards Reka LabelProps (including `for`) to LabelRoot.
      // Associations are enforced where the Label component is consumed.
      "vuejs-accessibility/label-has-for": "off",
    },
  },
];
