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
  SystemEventSource,
  SystemEventType,
} from "../../types";
import type {
  components as ApiContractComponents,
  operations as ApiContractOperations,
} from "@fn-knock/api-contract";
import { apiClient } from "./client";

type GetEventsOperation = ApiContractOperations["get_api_admin_events"];
type GetEventsQuery = NonNullable<GetEventsOperation["parameters"]["query"]>;
type GetEventsResponse =
  GetEventsOperation["responses"][200]["content"]["application/json"];
type DeleteEventsOperation = ApiContractOperations["delete_api_admin_events"];
type DeleteEventsBody =
  DeleteEventsOperation["requestBody"]["content"]["application/json"];
type DeleteEventsResponse =
  DeleteEventsOperation["responses"][200]["content"]["application/json"];
type ClearEventsResponse =
  ApiContractOperations["delete_api_admin_events_clear"]["responses"][200]["content"]["application/json"];
type NotificationSchemas = ApiContractComponents["schemas"];
type NotificationProviderCreateBody =
  NotificationSchemas["NotificationProviderCreateBodyData"];
type NotificationProviderUpdateBody =
  NotificationSchemas["NotificationProviderUpdateBodyData"];
type NotificationProviderTestBody =
  NotificationSchemas["NotificationProviderTestBodyData"];
type NotificationRuleCreateBody =
  NotificationSchemas["NotificationRuleCreateBodyData"];
type NotificationRuleUpdateBody =
  NotificationSchemas["NotificationRuleUpdateBodyData"];
type NotificationTriggerQuery = NonNullable<
  ApiContractOperations["get_api_admin_notifications_triggers"]["parameters"]["query"]
>;
type NotificationDeliveryQuery = NonNullable<
  ApiContractOperations["get_api_admin_notifications_deliveries"]["parameters"]["query"]
>;
type NotificationDeliveryClearBody =
  NotificationSchemas["NotificationDeliveryClearBodyData"];

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
  async getEvents(
    params: {
      page: number;
      limit: string;
      search: string;
      type?: SystemEventType | "all";
      level?: SystemEventLevel | "all";
      source?: SystemEventSource | "all";
      traceId?: string;
    },
    signal?: AbortSignal,
  ): Promise<GetEventsResponse> {
    const query = {
      page: params.page,
      limit: params.limit,
      search: params.search,
      type: params.type && params.type !== "all" ? params.type : undefined,
      level: params.level && params.level !== "all" ? params.level : undefined,
      source:
        params.source && params.source !== "all" ? params.source : undefined,
      trace_id: params.traceId?.trim() || undefined,
    } satisfies GetEventsQuery;
    const res = await apiClient.get("/events", {
      params: query,
      signal,
    });
    return res.data;
  },
  async deleteEvents(ids: string[]): Promise<DeleteEventsResponse> {
    const body = { ids } satisfies DeleteEventsBody;
    const res = await apiClient.delete("/events", { data: body });
    return res.data;
  },
  async clearEvents(): Promise<ClearEventsResponse> {
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
  async createNotificationProvider(payload: NotificationProviderCreateBody) {
    const res = await apiClient.post("/notifications/providers", payload);
    return res.data;
  },
  async updateNotificationProvider(
    id: string,
    payload: NotificationProviderUpdateBody,
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
  async testNotificationProviderDraft(payload: NotificationProviderTestBody) {
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
  async createNotificationRule(payload: NotificationRuleCreateBody) {
    const res = await apiClient.post("/notifications/rules", payload);
    return res.data;
  },
  async updateNotificationRule(
    id: string,
    payload: NotificationRuleUpdateBody,
  ) {
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
    const query = {
      page: params.page,
      limit: params.limit,
      rule_id: params.rule_id || undefined,
      status:
        params.status && params.status !== "all" ? params.status : undefined,
    } satisfies NotificationTriggerQuery;
    const res = await apiClient.get("/notifications/triggers", {
      params: query,
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
    const query = {
      page: params.page,
      limit: params.limit,
      rule_id: params.rule_id || undefined,
      provider_id: params.provider_id || undefined,
      trigger_id: params.trigger_id || undefined,
      status:
        params.status && params.status !== "all" ? params.status : undefined,
    } satisfies NotificationDeliveryQuery;
    const res = await apiClient.get("/notifications/deliveries", {
      params: query,
    });
    return res.data;
  },
  async clearNotificationDeliveries(params: {
    rule_id?: string;
    provider_id?: string;
    trigger_id?: string;
    status?: NotificationDeliveryStatus | "all";
  }) {
    const body = {
      rule_id: params.rule_id || undefined,
      provider_id: params.provider_id || undefined,
      trigger_id: params.trigger_id || undefined,
      status:
        params.status && params.status !== "all" ? params.status : undefined,
    } satisfies NotificationDeliveryClearBody;
    const res = await apiClient.delete("/notifications/deliveries", {
      data: body,
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
