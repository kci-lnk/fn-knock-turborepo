import { computed, toValue, type MaybeRefOrGetter, type Ref } from "vue";
import {
  normalizeDDNSDomainTargetInput,
  validateDDNSDomainTargets,
  type DDNSDomainTargetParseResult,
} from "@/lib/ddns-domain";
import { findProviderDef, type Provider, type ProviderField } from "./model";

type Translate = (key: string, params?: Record<string, unknown>) => string;

export const useDDNSDomainField = ({
  config,
  includeWildcardHint = false,
  providerName,
  providers,
  translate,
}: {
  config: Ref<Record<string, string>>;
  includeWildcardHint?: MaybeRefOrGetter<boolean>;
  providerName: MaybeRefOrGetter<string>;
  providers: Ref<Provider[]>;
  translate: Translate;
}) => {
  const providerDef = computed(() =>
    findProviderDef(providers.value, toValue(providerName)),
  );

  const domainTargets = computed(
    () => providerDef.value?.capabilities?.domainTargets,
  );

  const hasDomainField = computed(() =>
    providerDef.value?.fields.some((field) => field.key === "domain"),
  );

  const supportsWildcardRootPair = computed(
    () => domainTargets.value?.mode === "single_or_wildcard_root_pair",
  );

  const normalizeDomain = () => {
    if (
      !hasDomainField.value &&
      !Object.prototype.hasOwnProperty.call(config.value, "domain")
    ) {
      return;
    }

    const domain = normalizeDDNSDomainTargetInput(config.value.domain);
    if (domain !== (config.value.domain ?? "")) {
      config.value = { ...config.value, domain };
    }
  };

  const validateDomain = (): DDNSDomainTargetParseResult | null => {
    if (!hasDomainField.value) {
      return null;
    }

    const domain = config.value.domain ?? "";
    if (!domain || /^\p{White_Space}*$/u.test(domain)) {
      return null;
    }

    const capability = domainTargets.value;
    return validateDDNSDomainTargets(domain, {
      capability,
      rootDomain: capability?.rootField
        ? config.value[capability.rootField]
        : undefined,
    });
  };

  const normalizeForSubmit = () => {
    normalizeDomain();
    return validateDomain();
  };

  const formatOnBlur = normalizeDomain;

  const getFieldDescription = (field: ProviderField) => {
    const description = field.description?.trim() || "";
    if (field.key !== "domain") {
      return description;
    }

    const parts = [description];
    if (toValue(includeWildcardHint) && !supportsWildcardRootPair.value) {
      parts.push(translate("admin.ddns.wildcardHint"));
    }
    parts.push(
      translate(
        supportsWildcardRootPair.value
          ? "admin.ddns.domainTargetsPairHint"
          : "admin.ddns.domainTargetsSingleHint",
      ),
    );

    return [...new Set(parts.map((part) => part.trim()).filter(Boolean))].join(
      " ",
    );
  };

  return {
    domainTargets,
    formatOnBlur,
    getFieldDescription,
    normalizeForSubmit,
    providerDef,
    supportsWildcardRootPair,
    validateDomain,
  };
};
