import type {
  NotificationDeliveryListPayload,
  NotificationDeliveryStatus,
  NotificationProviderCatalogPayload,
  NotificationProviderDetailView,
  NotificationProviderListPayload,
  NotificationRuleListPayload,
  NotificationTriggerListPayload,
  NotificationTriggerStatus,
  SystemEventLevel,
  SystemEventListPayload,
  SystemEventSource,
  SystemEventType,
} from "../../types";
import { apiClient } from "./client";

export type {
  NotificationDeliveryListPayload,
  NotificationDeliveryStatus,
  NotificationProviderCatalogPayload,
  NotificationProviderDetailView,
  NotificationProviderListPayload,
  NotificationRuleListPayload,
  NotificationTriggerListPayload,
  NotificationTriggerStatus,
  OIDCBinding,
  OIDCProviderCatalogItem,
  OIDCProviderView,
  SystemEventLevel,
  SystemEventListPayload,
  SystemEventSource,
  SystemEventType,
} from "../../types";

export const EventCenterAPI = {
  async getEvents(params: {
    page: number;
    limit: string;
    search: string;
    type?: SystemEventType | "all";
    level?: SystemEventLevel | "all";
    source?: SystemEventSource | "all";
  }): Promise<{
    success: boolean;
    data: SystemEventListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/events", {
      params: {
        page: params.page,
        limit: params.limit,
        search: params.search,
        type: params.type && params.type !== "all" ? params.type : undefined,
        level:
          params.level && params.level !== "all" ? params.level : undefined,
        source:
          params.source && params.source !== "all" ? params.source : undefined,
      },
    });
    return res.data;
  },
  async deleteEvents(ids: string[]) {
    const res = await apiClient.delete("/events", { data: { ids } });
    return res.data;
  },
  async clearEvents(): Promise<{
    success: boolean;
    data: { deleted_count: number };
    message?: string;
  }> {
    const res = await apiClient.delete("/events/clear");
    return res.data;
  },
  async getNotificationProviderCatalog(): Promise<{
    success: boolean;
    data: NotificationProviderCatalogPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/providers/catalog");
    return res.data;
  },
  async getNotificationProviders(): Promise<{
    success: boolean;
    data: NotificationProviderListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/providers");
    return res.data;
  },
  async getNotificationProvider(id: string): Promise<{
    success: boolean;
    data: NotificationProviderDetailView;
    message?: string;
  }> {
    const res = await apiClient.get(
      `/notifications/providers/${encodeURIComponent(id)}`,
    );
    return res.data;
  },
  async createNotificationProvider(payload: {
    name?: string;
    type: string;
    enabled: boolean;
    connection_config: Record<string, unknown>;
  }) {
    const res = await apiClient.post("/notifications/providers", payload);
    return res.data;
  },
  async updateNotificationProvider(
    id: string,
    payload: {
      name?: string;
      enabled?: boolean;
      connection_config?: Record<string, unknown>;
    },
  ) {
    const res = await apiClient.patch(
      `/notifications/providers/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data;
  },
  async deleteNotificationProvider(id: string) {
    const res = await apiClient.delete(
      `/notifications/providers/${encodeURIComponent(id)}`,
    );
    return res.data;
  },
  async testNotificationProvider(id: string) {
    const res = await apiClient.post(
      `/notifications/providers/${encodeURIComponent(id)}/test`,
    );
    return res.data;
  },
  async testNotificationProviderDraft(payload: {
    id?: string;
    name?: string;
    type: string;
    enabled: boolean;
    connection_config: Record<string, unknown>;
  }) {
    const res = await apiClient.post("/notifications/providers/test", payload);
    return res.data;
  },
  async getNotificationRules(): Promise<{
    success: boolean;
    data: NotificationRuleListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/rules");
    return res.data;
  },
  async createNotificationRule(payload: Record<string, unknown>) {
    const res = await apiClient.post("/notifications/rules", payload);
    return res.data;
  },
  async updateNotificationRule(id: string, payload: Record<string, unknown>) {
    const res = await apiClient.patch(
      `/notifications/rules/${encodeURIComponent(id)}`,
      payload,
    );
    return res.data;
  },
  async deleteNotificationRule(id: string) {
    const res = await apiClient.delete(
      `/notifications/rules/${encodeURIComponent(id)}`,
    );
    return res.data;
  },
  async getNotificationTriggers(params: {
    page: number;
    limit: number;
    rule_id?: string;
    status?: NotificationTriggerStatus | "all";
  }): Promise<{
    success: boolean;
    data: NotificationTriggerListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/triggers", {
      params: {
        page: params.page,
        limit: params.limit,
        rule_id: params.rule_id || undefined,
        status:
          params.status && params.status !== "all" ? params.status : undefined,
      },
    });
    return res.data;
  },
  async getNotificationDeliveries(params: {
    page: number;
    limit: number;
    rule_id?: string;
    provider_id?: string;
    trigger_id?: string;
    status?: NotificationDeliveryStatus | "all";
  }): Promise<{
    success: boolean;
    data: NotificationDeliveryListPayload;
    message?: string;
  }> {
    const res = await apiClient.get("/notifications/deliveries", {
      params: {
        page: params.page,
        limit: params.limit,
        rule_id: params.rule_id || undefined,
        provider_id: params.provider_id || undefined,
        trigger_id: params.trigger_id || undefined,
        status:
          params.status && params.status !== "all" ? params.status : undefined,
      },
    });
    return res.data;
  },
  async clearNotificationDeliveries(params: {
    rule_id?: string;
    provider_id?: string;
    trigger_id?: string;
    status?: NotificationDeliveryStatus | "all";
  }) {
    const res = await apiClient.delete("/notifications/deliveries", {
      data: {
        rule_id: params.rule_id || undefined,
        provider_id: params.provider_id || undefined,
        trigger_id: params.trigger_id || undefined,
        status:
          params.status && params.status !== "all" ? params.status : undefined,
      },
    });
    return res.data as {
      success: boolean;
      data: {
        deleted_count: number;
      };
      message?: string;
    };
  },
};
