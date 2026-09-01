const FORM_CONTROL_NAMES = new Set([
  "Checkbox",
  "ComboboxInput",
  "Input",
  "InputGroupInput",
  "InputGroupTextarea",
  "InputOTP",
  "RadioGroupItem",
  "SelectTrigger",
  "Slider",
  "Switch",
  "TagsInputInput",
  "Textarea",
]);

const NATIVE_FORM_CONTROLS = new Set([
  "input",
  "meter",
  "output",
  "progress",
  "select",
  "textarea",
]);

const INTERACTIVE_NAMES = new Set([
  "Button",
  "InputGroupButton",
  "SidebarMenuAction",
  "SidebarMenuButton",
  "button",
]);

function getAttribute(node, name) {
  return node.startTag.attributes.find((attribute) => {
    if (!attribute.directive) {
      return attribute.key.name === name;
    }

    return (
      attribute.key.name.name === "bind" &&
      attribute.key.argument?.type === "VIdentifier" &&
      attribute.key.argument.name === name
    );
  });
}

function getStaticAttributeValue(node, name) {
  const attribute = getAttribute(node, name);
  if (!attribute || attribute.directive || !attribute.value) {
    return undefined;
  }
  return attribute.value.value;
}

function isNestedInLabel(node) {
  let parent = node.parent;
  while (parent?.type === "VElement") {
    if (parent.rawName === "label" || parent.rawName === "Label") {
      return true;
    }
    parent = parent.parent;
  }
  return false;
}

function collectPatternIdentifiers(pattern, identifiers) {
  if (!pattern) return;

  if (pattern.type === "Identifier") {
    identifiers.add(pattern.name);
    return;
  }

  if (pattern.type === "RestElement") {
    collectPatternIdentifiers(pattern.argument, identifiers);
    return;
  }

  if (pattern.type === "AssignmentPattern") {
    collectPatternIdentifiers(pattern.left, identifiers);
    return;
  }

  if (pattern.type === "ArrayPattern") {
    for (const element of pattern.elements) {
      collectPatternIdentifiers(element, identifiers);
    }
    return;
  }

  if (pattern.type === "ObjectPattern") {
    for (const property of pattern.properties) {
      collectPatternIdentifiers(
        property.type === "Property" ? property.value : property.argument,
        identifiers,
      );
    }
  }
}

function getLoopIdentifiers(node) {
  const identifiers = new Set();
  let current = node;

  while (current) {
    if (current.type === "VElement") {
      const forAttribute = current.startTag.attributes.find(
        (attribute) =>
          attribute.directive && attribute.key.name.name === "for",
      );
      const expression = forAttribute?.value?.expression;
      if (expression?.type === "VForExpression") {
        for (const pattern of expression.left) {
          collectPatternIdentifiers(pattern, identifiers);
        }
      }
    }
    current = current.parent;
  }

  return identifiers;
}

function expressionUsesIdentifier(expression, identifiers) {
  if (!expression || identifiers.size === 0) return false;

  const pending = [expression];
  const visited = new Set();
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || typeof current !== "object" || visited.has(current)) {
      continue;
    }
    visited.add(current);

    if (current.type === "Identifier" && identifiers.has(current.name)) {
      return true;
    }

    for (const [key, value] of Object.entries(current)) {
      if (key === "parent") continue;
      if (Array.isArray(value)) {
        pending.push(...value);
      } else if (value && typeof value === "object") {
        pending.push(value);
      }
    }
  }

  return false;
}

const noStaticFormFieldIdInLoop = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Require form field ids and label targets inside loops to use a loop binding.",
    },
    messages: {
      repeated:
        "Form field {{attribute}} inside v-for must reference a loop binding so every rendered field remains unique.",
    },
    schema: [],
  },
  create(context) {
    const visitor = {
      VElement(node) {
        const isLabel = node.rawName === "label" || node.rawName === "Label";
        const isFormControl =
          NATIVE_FORM_CONTROLS.has(node.rawName) ||
          FORM_CONTROL_NAMES.has(node.rawName);
        if (!isLabel && !isFormControl) return;

        const loopIdentifiers = getLoopIdentifiers(node);
        if (loopIdentifiers.size === 0) return;

        const attributeName = isLabel ? "for" : "id";
        const attribute = getAttribute(node, attributeName);
        if (!attribute) return;

        const expression = attribute.directive
          ? attribute.value?.expression
          : null;
        if (expressionUsesIdentifier(expression, loopIdentifiers)) return;

        context.report({
          node: attribute,
          messageId: "repeated",
          data: { attribute: attributeName },
        });
      },
    };

    return (
      context.sourceCode.parserServices.defineTemplateBodyVisitor?.(visitor) ??
      {}
    );
  },
};

const formControlHasAccessibleName = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Require native and project form controls to expose an accessible name.",
    },
    messages: {
      missing:
        "Form control must be nested in a label or expose id, aria-label, or aria-labelledby.",
      unmatchedId:
        'Form control id "{{id}}" must match a label for attribute or expose aria-label/aria-labelledby.',
    },
    schema: [],
  },
  create(context) {
    const controlsWithStaticIds = [];
    const staticLabelTargets = new Set();

    const visitor = {
      VElement(node) {
        if (node.rawName === "label" || node.rawName === "Label") {
          const labelFor = getStaticAttributeValue(node, "for");
          if (labelFor) staticLabelTargets.add(labelFor);
        }

        const isNative = NATIVE_FORM_CONTROLS.has(node.rawName);
        const isProjectControl = FORM_CONTROL_NAMES.has(node.rawName);
        if (!isNative && !isProjectControl) {
          return;
        }

        if (node.rawName === "input") {
          const type = getStaticAttributeValue(node, "type");
          if (
            type &&
            ["button", "hidden", "image", "reset", "submit"].includes(type)
          ) {
            return;
          }
        }

        const staticClass = getStaticAttributeValue(node, "class") ?? "";
        if (
          getAttribute(node, "hidden") ||
          getStaticAttributeValue(node, "aria-hidden") === "true" ||
          staticClass.split(/\s+/u).includes("hidden")
        ) {
          return;
        }

        if (
          isNestedInLabel(node) ||
          getAttribute(node, "aria-label") ||
          getAttribute(node, "aria-labelledby")
        ) {
          return;
        }

        const idAttribute = getAttribute(node, "id");
        if (idAttribute) {
          if (idAttribute.directive) return;
          const id = getStaticAttributeValue(node, "id");
          if (id) {
            controlsWithStaticIds.push({ id, node });
            return;
          }
        }

        context.report({ node, messageId: "missing" });
      },
      "VDocumentFragment:exit"() {
        for (const control of controlsWithStaticIds) {
          if (!staticLabelTargets.has(control.id)) {
            context.report({
              node: control.node,
              messageId: "unmatchedId",
              data: { id: control.id },
            });
          }
        }
      },
    };

    return (
      context.sourceCode.parserServices.defineTemplateBodyVisitor?.(visitor) ??
      {}
    );
  },
};

function hasAccessibleText(node) {
  return (node.children ?? []).some((child) => {
    if (child.type === "VText") return child.value.trim().length > 0;
    if (child.type === "VExpressionContainer") return Boolean(child.expression);
    if (child.type !== "VElement") return false;
    if (child.rawName === "slot") return true;
    if (
      child.rawName === "OverflowTooltipText" &&
      getAttribute(child, "text")
    ) {
      return true;
    }
    return hasAccessibleText(child);
  });
}

const interactiveHasAccessibleName = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Require project button primitives to expose text or an accessible name.",
    },
    messages: {
      missing:
        "Interactive control must expose text, aria-label, aria-labelledby, or title.",
    },
    schema: [],
  },
  create(context) {
    const visitor = {
      VElement(node) {
        if (!INTERACTIVE_NAMES.has(node.rawName)) return;
        if (
          getAttribute(node, "aria-label") ||
          getAttribute(node, "aria-labelledby") ||
          getAttribute(node, "title") ||
          hasAccessibleText(node)
        ) {
          return;
        }
        context.report({ node, messageId: "missing" });
      },
    };

    return (
      context.sourceCode.parserServices.defineTemplateBodyVisitor?.(visitor) ??
      {}
    );
  },
};

export const vueA11yProjectPlugin = {
  rules: {
    "form-control-has-accessible-name": formControlHasAccessibleName,
    "interactive-has-accessible-name": interactiveHasAccessibleName,
    "no-static-form-field-id-in-loop": noStaticFormFieldIdInLoop,
  },
};
