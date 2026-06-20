import {
  getNotificationProviderDefinition,
  listNotificationProviderDefinitions,
  maskNotificationProvider,
  revealNotificationProvider,
  sendNotificationWithProvider,
} from "./definitions";
import {
  buildNotificationGroupKey,
  eventMatchesNotificationRule,
} from "./matcher";
import { redisNotificationStore } from "./redis-store";
import {
  buildNotificationMessage,
  buildNotificationRuleName,
} from "./templates";
import {
  NOTIFICATION_DELIVERY_STATUSES,
  NOTIFICATION_GROUP_BY_VALUES,
  NOTIFICATION_MESSAGE_TEMPLATE_MODES,
  NOTIFICATION_PROVIDER_TYPES,
  NOTIFICATION_TEMPLATE_OVERRIDE_MODES,
  type NotificationDeliveryClearQuery,
  type NotificationDelivery,
  type NotificationDeliveryListQuery,
  type NotificationProvider,
  type NotificationProviderDraftTestInput,
  type NotificationProviderUpsertInput,
  type NotificationRule,
  type NotificationRuleUpsertInput,
  type NotificationTargetBinding,
  type NotificationTrigger,
  type NotificationTriggerListQuery,
} from "./types";
import {
  isSystemEventLevel,
  isSystemEventSource,
  isSystemEventType,
} from "../system-events/constants";
import type { SystemEventEnvelope } from "../system-events/types";
import {
  asPlainRecord,
  buildNextSequentialName,
  createId,
  createStableId,
  nowIso,
  parseNumberField,
  serviceT,
  serviceTForLocale,
  toMs,
  uniqueStrings,
} from "./service/common";
import {
  DEFAULT_DELIVERY_POLICY,
  isTerminalDeliveryStatus,
  resolveDeliveryPolicy,
  resolveDeliveryReadyAtMs,
} from "./service/delivery";
import {
  buildProviderTestMessage,
  resolveMessageLocale,
} from "./service/message";
import {
  applySchemaDefaults,
  normalizeProviderConnectionConfig,
  normalizeProviderTargetConfig,
  normalizeSchemaPatch,
  validateRequiredSchemaFields,
} from "./service/provider-config";
import {
  type LocaleCode,
} from "../../../../../packages/i18n/src";
import { configManager } from "../redis";
const DEFAULT_RULE_WINDOW_SECONDS = 60;
const DEFAULT_RULE_COOLDOWN_SECONDS = 60;

export class SystemNotificationService {
  listProviderCatalog(locale?: LocaleCode) {
    return listNotificationProviderDefinitions(locale);
  }

  async listProviders() {
    const providers = await redisNotificationStore.listProviders();
    return providers.map(maskNotificationProvider);
  }

  async getProvider(id: string) {
    const provider = await redisNotificationStore.getProvider(id);
    if (!provider) {
      throw new Error(serviceT("providerNotFound"));
    }

    return revealNotificationProvider(provider);
  }

  async createProvider(input: NotificationProviderUpsertInput) {
    const requestedName = String(input.name || "").trim();
    const type = String(input.type || "").trim();
    if (!NOTIFICATION_PROVIDER_TYPES.includes(type as any)) {
      throw new Error(serviceT("unsupportedProviderType"));
    }

    const definition = getNotificationProviderDefinition(type);
    if (!definition) {
      throw new Error(serviceT("providerDefinitionMissing"));
    }

    const configPatch = normalizeSchemaPatch(
      normalizeProviderConnectionConfig(
        type,
        asPlainRecord(input.connection_config),
      ),
      definition.connection_schema,
    );
    const connectionConfig = applySchemaDefaults(
      configPatch,
      definition.connection_schema,
    );
    validateRequiredSchemaFields(
      connectionConfig,
      definition.connection_schema,
    );

    const existingProviders = await redisNotificationStore.listProviders();
    const name =
      requestedName ||
      buildNextSequentialName(
        definition.label,
        existingProviders.map((provider) => provider.name),
      );

    const provider: NotificationProvider = {
      id: createId("ntfprov"),
      name,
      type: definition.type,
      enabled: input.enabled ?? true,
      connection_config: connectionConfig,
      created_at: nowIso(),
      updated_at: nowIso(),
      last_test_status: "idle",
      last_error: null,
    };

    await redisNotificationStore.saveProvider(provider);
    return maskNotificationProvider(provider);
  }

  async updateProvider(id: string, input: NotificationProviderUpsertInput) {
    const provider = await redisNotificationStore.getProvider(id);
    if (!provider) {
      throw new Error(serviceT("providerNotFound"));
    }

    const definition = getNotificationProviderDefinition(provider.type);
    if (!definition) {
      throw new Error(serviceT("providerDefinitionMissing"));
    }

    const patch = normalizeSchemaPatch(
      normalizeProviderConnectionConfig(
        provider.type,
        asPlainRecord(input.connection_config),
      ),
      definition.connection_schema,
    );
    const connectionConfig = applySchemaDefaults(
      {
        ...provider.connection_config,
        ...patch,
      },
      definition.connection_schema,
    );
    validateRequiredSchemaFields(
      connectionConfig,
      definition.connection_schema,
    );

    const updatedProvider: NotificationProvider = {
      ...provider,
      name: input.name?.trim() ? input.name.trim() : provider.name,
      enabled: input.enabled ?? provider.enabled,
      connection_config: connectionConfig,
      updated_at: nowIso(),
    };

    await redisNotificationStore.saveProvider(updatedProvider);
    return maskNotificationProvider(updatedProvider);
  }

  async deleteProvider(id: string) {
    const rules = await redisNotificationStore.listRules();
    const referencedByRule = rules.find((rule) =>
      rule.targets.some((target) => target.provider_id === id),
    );
    if (referencedByRule) {
      throw new Error(
        serviceT("providerReferencedByRule", {
          rule: referencedByRule.name,
        }),
      );
    }

    await redisNotificationStore.deleteProvider(id);
  }

  async testProvider(id: string) {
    const locale = await configManager.getLocaleConfig();
    const t = (
      key: string,
      params?: Record<string, string | number | boolean | null | undefined>,
    ) => serviceTForLocale(locale.default_locale, key, params);
    const provider = await redisNotificationStore.getProvider(id);
    if (!provider) {
      throw new Error(serviceT("providerNotFound"));
    }

    const timeoutSeconds = parseNumberField(
      provider.connection_config.timeout_seconds,
      DEFAULT_DELIVERY_POLICY.timeout_seconds,
      { min: 1, max: 30 },
    );
    const message = buildProviderTestMessage(locale.default_locale);

    const result = await sendNotificationWithProvider(
      provider,
      message,
      undefined,
      timeoutSeconds,
      locale.default_locale,
    );

    const updatedProvider: NotificationProvider = {
      ...provider,
      last_test_at: nowIso(),
      last_test_status: result.success ? "success" : "failed",
      last_error: result.success ? null : result.reason || t("testSendFailed"),
      updated_at: nowIso(),
    };
    await redisNotificationStore.saveProvider(updatedProvider);

    return {
      success: result.success,
      message: result.success
        ? t("testSendSuccess")
        : result.reason || t("testSendFailed"),
      data: {
        provider: maskNotificationProvider(updatedProvider),
        request_summary: result.request_summary,
        response_summary: result.response_summary,
      },
    };
  }

  async testProviderDraft(input: NotificationProviderDraftTestInput) {
    const locale = await configManager.getLocaleConfig();
    const t = (
      key: string,
      params?: Record<string, string | number | boolean | null | undefined>,
    ) => serviceTForLocale(locale.default_locale, key, params);
    const requestedId = String(input.id || "").trim();
    const requestedType = String(input.type || "").trim();
    if (!NOTIFICATION_PROVIDER_TYPES.includes(requestedType as any)) {
      throw new Error(serviceT("unsupportedProviderType"));
    }

    const definition = getNotificationProviderDefinition(requestedType);
    if (!definition) {
      throw new Error(serviceT("providerDefinitionMissing"));
    }

    const existingProvider = requestedId
      ? await redisNotificationStore.getProvider(requestedId)
      : null;
    if (requestedId && !existingProvider) {
      throw new Error(serviceT("providerNotFound"));
    }
    if (existingProvider && existingProvider.type !== definition.type) {
      throw new Error(serviceT("providerTypeMismatch"));
    }

    const patch = normalizeSchemaPatch(
      normalizeProviderConnectionConfig(
        definition.type,
        asPlainRecord(input.connection_config),
      ),
      definition.connection_schema,
    );
    const connectionConfig = applySchemaDefaults(
      {
        ...(existingProvider?.connection_config ?? {}),
        ...patch,
      },
      definition.connection_schema,
    );
    validateRequiredSchemaFields(
      connectionConfig,
      definition.connection_schema,
    );

    const now = nowIso();
    const provider: NotificationProvider = {
      id: existingProvider?.id || createId("ntfprovtest"),
      name:
        input.name?.trim() ||
        existingProvider?.name ||
        t("providerTestName", { provider: definition.label }),
      type: definition.type,
      enabled: input.enabled ?? existingProvider?.enabled ?? true,
      connection_config: connectionConfig,
      created_at: existingProvider?.created_at || now,
      updated_at: now,
      last_test_at: existingProvider?.last_test_at,
      last_test_status: existingProvider?.last_test_status,
      last_error: existingProvider?.last_error ?? null,
    };

    const timeoutSeconds = parseNumberField(
      provider.connection_config.timeout_seconds,
      DEFAULT_DELIVERY_POLICY.timeout_seconds,
      { min: 1, max: 30 },
    );
    const result = await sendNotificationWithProvider(
      provider,
      buildProviderTestMessage(locale.default_locale),
      undefined,
      timeoutSeconds,
      locale.default_locale,
    );

    const testedProvider: NotificationProvider = {
      ...provider,
      last_test_at: nowIso(),
      last_test_status: result.success ? "success" : "failed",
      last_error: result.success ? null : result.reason || t("testSendFailed"),
      updated_at: nowIso(),
    };

    return {
      success: result.success,
      message: result.success
        ? t("testSendSuccess")
        : result.reason || t("testSendFailed"),
      data: {
        provider: maskNotificationProvider(testedProvider),
        request_summary: result.request_summary,
        response_summary: result.response_summary,
      },
    };
  }

  async listRules() {
    return redisNotificationStore.listRules();
  }

  private async normalizeRuleTargets(
    inputTargets: NotificationRuleUpsertInput["targets"],
    currentTargets: NotificationTargetBinding[] = [],
  ) {
    const providers = await redisNotificationStore.listProviders();
    const providerMap = new Map(
      providers.map((provider) => [provider.id, provider]),
    );
    const currentTargetMap = new Map(
      currentTargets.map((target) => [target.id, target]),
    );

    const targets: NotificationTargetBinding[] = [];
    for (const inputTarget of inputTargets || []) {
      const provider = providerMap.get(inputTarget.provider_id);
      if (!provider) {
        throw new Error(serviceT("ruleProviderMissing"));
      }

      const definition = getNotificationProviderDefinition(provider.type);
      if (!definition) {
        throw new Error(serviceT("providerDefinitionMissing"));
      }

      const existingTarget = inputTarget.id
        ? currentTargetMap.get(inputTarget.id)
        : null;
      const targetPatch = normalizeSchemaPatch(
        normalizeProviderTargetConfig(
          provider.type,
          asPlainRecord(inputTarget.target_config),
        ),
        definition.target_schema,
      );
      // Target config is edited as a full snapshot in the UI, so blank values
      // should clear older overrides instead of silently keeping them.
      const targetConfig = applySchemaDefaults(
        targetPatch,
        definition.target_schema,
      );

      const now = nowIso();
      const templateOverrideMode = String(
        inputTarget.template_override_mode ||
          existingTarget?.template_override_mode ||
          "inherit",
      ).trim();
      if (
        !NOTIFICATION_TEMPLATE_OVERRIDE_MODES.includes(
          templateOverrideMode as any,
        )
      ) {
        throw new Error(serviceT("invalidTemplateOverrideMode"));
      }

      targets.push({
        id: inputTarget.id || createId("ntftarget"),
        provider_id: provider.id,
        enabled: inputTarget.enabled ?? existingTarget?.enabled ?? true,
        target_config: targetConfig,
        template_override_mode:
          templateOverrideMode as NotificationTargetBinding["template_override_mode"],
        template_override:
          inputTarget.template_override ??
          existingTarget?.template_override ??
          null,
        delivery_policy:
          inputTarget.delivery_policy ??
          existingTarget?.delivery_policy ??
          null,
        created_at: existingTarget?.created_at || now,
        updated_at: now,
      });
    }

    return targets;
  }

  async createRule(input: NotificationRuleUpsertInput) {
    const eventType = String(input.event_type || "").trim();
    const groupBy = String(input.group_by || "").trim();
    const messageTemplateMode = String(
      input.message_template_mode || "default",
    ).trim();

    if (!isSystemEventType(eventType)) {
      throw new Error(serviceT("unsupportedEventType"));
    }
    if (!NOTIFICATION_GROUP_BY_VALUES.includes(groupBy as any)) {
      throw new Error(serviceT("invalidGroupBy"));
    }
    if (
      !NOTIFICATION_MESSAGE_TEMPLATE_MODES.includes(messageTemplateMode as any)
    ) {
      throw new Error(serviceT("invalidMessageTemplateMode"));
    }

    const eventLevelFilter = uniqueStrings(input.event_level_filter);
    if (!eventLevelFilter.every((value) => isSystemEventLevel(value))) {
      throw new Error(serviceT("invalidEventLevelFilter"));
    }

    const eventSourceFilter = uniqueStrings(input.event_source_filter);
    if (!eventSourceFilter.every((value) => isSystemEventSource(value))) {
      throw new Error(serviceT("invalidEventSourceFilter"));
    }

    const targets = await this.normalizeRuleTargets(input.targets);
    if (!targets.length) {
      throw new Error(serviceT("targetRequired"));
    }

    const existingRules = await redisNotificationStore.listRules();
    if (existingRules.some((rule) => rule.event_type === eventType)) {
      throw new Error(serviceT("duplicateEventRule"));
    }

    const locale = await configManager.getLocaleConfig();
    const now = nowIso();
    const rule: NotificationRule = {
      id: createId("ntfrule"),
      name: buildNotificationRuleName(eventType, locale.default_locale),
      enabled: input.enabled ?? true,
      event_type: eventType,
      ...(eventLevelFilter.length
        ? { event_level_filter: eventLevelFilter }
        : {}),
      ...(eventSourceFilter.length
        ? { event_source_filter: eventSourceFilter }
        : {}),
      window_seconds: parseNumberField(
        input.window_seconds,
        DEFAULT_RULE_WINDOW_SECONDS,
        {
          min: 1,
          max: 86400,
        },
      ),
      threshold_count: parseNumberField(input.threshold_count, 1, {
        min: 1,
        max: 9999,
      }),
      group_by: groupBy as NotificationRule["group_by"],
      cooldown_seconds: parseNumberField(
        input.cooldown_seconds,
        DEFAULT_RULE_COOLDOWN_SECONDS,
        {
          min: 0,
          max: 86400,
        },
      ),
      targets,
      message_template_mode:
        messageTemplateMode as NotificationRule["message_template_mode"],
      message_template: input.message_template ?? null,
      created_at: now,
      updated_at: now,
      last_triggered_at: null,
    };

    await redisNotificationStore.saveRule(rule);
    return rule;
  }

  async updateRule(id: string, input: NotificationRuleUpsertInput) {
    const currentRule = await redisNotificationStore.getRule(id);
    if (!currentRule) {
      throw new Error(serviceT("ruleNotFound"));
    }

    const eventType = input.event_type
      ? String(input.event_type).trim()
      : currentRule.event_type;
    const groupBy = input.group_by
      ? String(input.group_by).trim()
      : currentRule.group_by;
    const messageTemplateMode = input.message_template_mode
      ? String(input.message_template_mode).trim()
      : currentRule.message_template_mode;

    if (!isSystemEventType(eventType)) {
      throw new Error(serviceT("unsupportedEventType"));
    }
    if (!NOTIFICATION_GROUP_BY_VALUES.includes(groupBy as any)) {
      throw new Error(serviceT("invalidGroupBy"));
    }
    if (
      !NOTIFICATION_MESSAGE_TEMPLATE_MODES.includes(messageTemplateMode as any)
    ) {
      throw new Error(serviceT("invalidMessageTemplateMode"));
    }

    const eventLevelFilter =
      input.event_level_filter !== undefined
        ? uniqueStrings(input.event_level_filter)
        : currentRule.event_level_filter || [];
    if (!eventLevelFilter.every((value) => isSystemEventLevel(value))) {
      throw new Error(serviceT("invalidEventLevelFilter"));
    }

    const eventSourceFilter =
      input.event_source_filter !== undefined
        ? uniqueStrings(input.event_source_filter)
        : currentRule.event_source_filter || [];
    if (!eventSourceFilter.every((value) => isSystemEventSource(value))) {
      throw new Error(serviceT("invalidEventSourceFilter"));
    }

    const targets =
      input.targets !== undefined
        ? await this.normalizeRuleTargets(input.targets, currentRule.targets)
        : currentRule.targets;
    if (!targets.length) {
      throw new Error(serviceT("targetRequired"));
    }

    if (
      eventType !== currentRule.event_type &&
      (await redisNotificationStore.listRules()).some(
        (rule) => rule.id !== currentRule.id && rule.event_type === eventType,
      )
    ) {
      throw new Error(serviceT("duplicateEventRule"));
    }

    const locale = await configManager.getLocaleConfig();
    const updatedRule: NotificationRule = {
      ...currentRule,
      name: buildNotificationRuleName(eventType, locale.default_locale),
      enabled: input.enabled ?? currentRule.enabled,
      event_type: eventType,
      event_level_filter: eventLevelFilter.length
        ? eventLevelFilter
        : undefined,
      event_source_filter: eventSourceFilter.length
        ? eventSourceFilter
        : undefined,
      window_seconds:
        input.window_seconds !== undefined
          ? parseNumberField(input.window_seconds, currentRule.window_seconds, {
              min: 1,
              max: 86400,
            })
          : currentRule.window_seconds,
      threshold_count:
        input.threshold_count !== undefined
          ? parseNumberField(
              input.threshold_count,
              currentRule.threshold_count,
              {
                min: 1,
                max: 9999,
              },
            )
          : currentRule.threshold_count,
      group_by: groupBy as NotificationRule["group_by"],
      cooldown_seconds:
        input.cooldown_seconds !== undefined
          ? parseNumberField(
              input.cooldown_seconds,
              currentRule.cooldown_seconds,
              {
                min: 0,
                max: 86400,
              },
            )
          : currentRule.cooldown_seconds,
      targets,
      message_template_mode:
        messageTemplateMode as NotificationRule["message_template_mode"],
      message_template:
        input.message_template !== undefined
          ? input.message_template
          : currentRule.message_template,
      updated_at: nowIso(),
    };

    await redisNotificationStore.saveRule(updatedRule);
    return updatedRule;
  }

  async deleteRule(id: string) {
    await redisNotificationStore.deleteRule(id);
  }

  async listTriggers(query: NotificationTriggerListQuery) {
    return redisNotificationStore.listTriggers(query);
  }

  async listDeliveries(query: NotificationDeliveryListQuery) {
    return redisNotificationStore.listDeliveries(query);
  }

  async clearDeliveries(query: NotificationDeliveryClearQuery) {
    return redisNotificationStore.clearDeliveries(query);
  }

  async handleEvent(event: SystemEventEnvelope) {
    const locale = await configManager.getLocaleConfig();
    const rules = await redisNotificationStore.listRules();
    const matchingRules = rules.filter((rule) =>
      eventMatchesNotificationRule(event, rule),
    );
    if (!matchingRules.length) return;

    for (const rule of matchingRules) {
      const triggerId = createStableId("ntftrig", rule.id, event.id);
      let trigger = await redisNotificationStore.getTrigger(triggerId);
      let triggerCreated = false;

      if (!trigger) {
        const groupKey = buildNotificationGroupKey(event, rule.group_by);
        const matchedCount = await redisNotificationStore.appendWindowHit({
          ruleId: rule.id,
          groupKey,
          eventId: event.id,
          happenedAtMs: Date.parse(event.happened_at) || Date.now(),
          windowSeconds: rule.window_seconds,
        });
        if (matchedCount < rule.threshold_count) {
          continue;
        }

        const cooldownUntil = await redisNotificationStore.getCooldownUntil(
          rule.id,
          groupKey,
        );
        if (cooldownUntil && toMs(cooldownUntil) > Date.now()) {
          continue;
        }

        const draftTrigger: NotificationTrigger = {
          id: triggerId,
          rule_id: rule.id,
          event_id: event.id,
          group_key: groupKey,
          matched_count: matchedCount,
          message_snapshot: buildNotificationMessage({
            event,
            rule,
            matchedCount,
            groupKey,
            locale: locale.default_locale,
          }),
          rule_snapshot: rule,
          status: "created",
          created_at: nowIso(),
        };
        triggerCreated =
          await redisNotificationStore.saveTriggerIfAbsent(draftTrigger);
        trigger = triggerCreated
          ? draftTrigger
          : await redisNotificationStore.getTrigger(draftTrigger.id);
      }

      if (!trigger) {
        continue;
      }

      const fanoutRule = trigger.rule_snapshot;
      const fanoutMessage = trigger.message_snapshot;
      const triggerCreatedAt = trigger.created_at;

      for (const target of fanoutRule.targets) {
        const provider = await redisNotificationStore.getProvider(
          target.provider_id,
        );
        const deliveryId = createStableId("ntfdel", trigger.id, target.id);
        if (!provider || !provider.enabled || !target.enabled) {
          const skippedDelivery: NotificationDelivery = {
            id: deliveryId,
            trigger_id: trigger.id,
            rule_id: trigger.rule_id,
            target_id: target.id,
            provider_id: target.provider_id,
            event_id: event.id,
            status: "skipped",
            reason: !provider
              ? "provider_missing"
              : !provider.enabled
                ? "provider_disabled"
                : "target_disabled",
            provider_type: provider?.type || "webhook",
            message_snapshot: fanoutMessage,
            target_snapshot: target,
            provider_snapshot: provider
              ? maskNotificationProvider(provider)
              : {
                  id: target.provider_id,
                  name: serviceT("deletedProvider"),
                  type: "webhook",
                  enabled: false,
                  created_at: triggerCreatedAt,
                  updated_at: triggerCreatedAt,
                  connection_config_masked: {},
                },
            attempt_count: 0,
            triggered_at: triggerCreatedAt,
          };
          await redisNotificationStore.saveDeliveryIfAbsent(skippedDelivery);
          continue;
        }

        const delivery: NotificationDelivery = {
          id: deliveryId,
          trigger_id: trigger.id,
          rule_id: trigger.rule_id,
          target_id: target.id,
          provider_id: provider.id,
          event_id: event.id,
          status: "queued",
          provider_type: provider.type,
          message_snapshot: fanoutMessage,
          target_snapshot: target,
          provider_snapshot: maskNotificationProvider(provider),
          attempt_count: 0,
          triggered_at: triggerCreatedAt,
          next_retry_at: triggerCreatedAt,
        };
        const deliveryCreated =
          await redisNotificationStore.saveDeliveryIfAbsent(delivery);
        if (deliveryCreated) {
          await redisNotificationStore.enqueueDelivery(delivery.id, Date.now());
          continue;
        }

        const existingDelivery = await redisNotificationStore.getDelivery(
          delivery.id,
        );
        if (
          existingDelivery &&
          !isTerminalDeliveryStatus(existingDelivery.status)
        ) {
          await redisNotificationStore.enqueueDelivery(
            existingDelivery.id,
            resolveDeliveryReadyAtMs(existingDelivery),
          );
        }
      }

      if (triggerCreated) {
        const updatedRule: NotificationRule = {
          ...rule,
          last_triggered_at: triggerCreatedAt,
          updated_at: triggerCreatedAt,
        };
        await redisNotificationStore.saveRule(updatedRule);
      }

      const latestTrigger = await redisNotificationStore.getTrigger(trigger.id);
      if (latestTrigger?.status === "created") {
        await redisNotificationStore.saveTrigger({
          ...latestTrigger,
          status: "fanout_done",
        });
      }
      await this.refreshTriggerStatus(trigger.id);

      if (fanoutRule.cooldown_seconds > 0 && triggerCreated) {
        const until = new Date(
          Date.now() + fanoutRule.cooldown_seconds * 1000,
        ).toISOString();
        await redisNotificationStore.setCooldown({
          ruleId: fanoutRule.id,
          groupKey: trigger.group_key,
          until,
          cooldownSeconds: fanoutRule.cooldown_seconds,
        });
      }
    }
  }

  async processReadyDeliveries(limit = 10) {
    const deliveryIds =
      await redisNotificationStore.pullReadyDeliveryIds(limit);
    for (const deliveryId of deliveryIds) {
      await this.processDelivery(deliveryId);
    }
    return deliveryIds.length;
  }

  private async processDelivery(deliveryId: string) {
    const delivery = await redisNotificationStore.getDelivery(deliveryId);
    if (!delivery) return;
    if (
      delivery.status === "success" ||
      delivery.status === "gave_up" ||
      delivery.status === "skipped"
    ) {
      return;
    }

    const trigger = await redisNotificationStore.getTrigger(
      delivery.trigger_id,
    );
    const rule = await redisNotificationStore.getRule(delivery.rule_id);
    const provider = await redisNotificationStore.getProvider(
      delivery.provider_id,
    );
    if (!trigger || !rule || !provider) {
      await redisNotificationStore.saveDelivery({
        ...delivery,
        status: "gave_up",
        reason: "missing_trigger_rule_or_provider",
      });
      if (trigger) {
        await this.refreshTriggerStatus(trigger.id);
      }
      return;
    }

    const target =
      rule.targets.find((item) => item.id === delivery.target_id) ||
      delivery.target_snapshot;
    const eventId = delivery.event_id;
    const policy = resolveDeliveryPolicy(target.delivery_policy);
    const sendingDelivery: NotificationDelivery = {
      ...delivery,
      status: "sending",
      attempt_count: delivery.attempt_count + 1,
      reason: null,
      next_retry_at: null,
    };
    await redisNotificationStore.saveDelivery(sendingDelivery);

    const definition = getNotificationProviderDefinition(provider.type);
    if (!definition) {
      await redisNotificationStore.saveDelivery({
        ...sendingDelivery,
        status: "gave_up",
        reason: `unsupported_provider:${provider.type}`,
      });
      await this.refreshTriggerStatus(trigger.id);
      return;
    }

    const event = {
      id: eventId,
      type: rule.event_type,
      source: rule.event_source_filter?.[0] || "SERVER_ADMIN",
      level: rule.event_level_filter?.[0] || "WARN",
      happened_at: delivery.message_snapshot.occurred_at,
      payload: {},
    } as SystemEventEnvelope;

    const result = await sendNotificationWithProvider(
      provider,
      delivery.message_snapshot,
      {
        trigger,
        delivery: sendingDelivery,
        rule,
        target,
        provider,
        event,
        effective_delivery_policy: policy,
      },
      policy.timeout_seconds,
      resolveMessageLocale(delivery.message_snapshot),
    );

    if (result.success) {
      await redisNotificationStore.saveDelivery({
        ...sendingDelivery,
        status: "success",
        request_summary: result.request_summary || null,
        response_summary: result.response_summary || null,
        sent_at: nowIso(),
        next_retry_at: null,
      });
      await this.refreshTriggerStatus(trigger.id);
      return;
    }

    const shouldRetry =
      result.retryable && sendingDelivery.attempt_count < policy.max_attempts;
    if (shouldRetry) {
      const nextRetryAt = new Date(
        Date.now() + policy.backoff_seconds * 1000,
      ).toISOString();
      await redisNotificationStore.saveDelivery({
        ...sendingDelivery,
        status: "failed",
        reason: result.reason || "delivery_failed",
        request_summary: result.request_summary || null,
        response_summary: result.response_summary || null,
        next_retry_at: nextRetryAt,
      });
      await redisNotificationStore.enqueueDelivery(
        sendingDelivery.id,
        Date.parse(nextRetryAt),
      );
      return;
    }

    await redisNotificationStore.saveDelivery({
      ...sendingDelivery,
      status: "gave_up",
      reason: result.reason || "delivery_gave_up",
      request_summary: result.request_summary || null,
      response_summary: result.response_summary || null,
      next_retry_at: null,
    });
    await this.refreshTriggerStatus(trigger.id);
  }

  private async refreshTriggerStatus(triggerId: string) {
    const trigger = await redisNotificationStore.getTrigger(triggerId);
    if (!trigger) return;

    const deliveries =
      await redisNotificationStore.listDeliveriesByTrigger(triggerId);
    if (!deliveries.length) return;

    const hasPending = deliveries.some(
      (delivery) => !isTerminalDeliveryStatus(delivery.status),
    );
    if (hasPending) {
      return;
    }

    const allSucceeded = deliveries.every(
      (delivery) =>
        delivery.status === "success" || delivery.status === "skipped",
    );
    await redisNotificationStore.saveTrigger({
      ...trigger,
      status: allSucceeded ? "completed" : "partially_failed",
    });
  }
}

export const systemNotificationService = new SystemNotificationService();
