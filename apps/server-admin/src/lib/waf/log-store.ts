import { redis } from "../redis";
import type { WAFEvent } from "../go-backend";

const dateKey = (date: string) => `fn_knock:waf:logs:${date}`;
const eventKey = (traceId: string) => `fn_knock:waf:log:${traceId}`;
const statsKey = (date: string) => `fn_knock:waf:stats:${date}`;
const DATES_INDEX_KEY = "fn_knock:waf:logs:dates";
const DATES_INDEX_MIGRATED_KEY = "fn_knock:waf:logs:dates:migrated";

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;
const INITIALIZATION_RULE_FILENAME = "REQUEST-901-INITIALIZATION.conf";
const UNFILTERED_QUERY_SCAN_CHUNK_SIZE = 500;
const FILTERED_QUERY_SCAN_CHUNK_SIZE = 500;
const STALE_ID_REMOVE_CHUNK_SIZE = 500;
const RANGE_QUERY_SCAN_CHUNK_SIZE = 500;
const DELETE_DATE_CHUNK_SIZE = 500;

export interface WAFLogQuery {
  date?: string;
  trace_id?: string;
  search?: string;
  host?: string;
  client_ip?: string;
  rule_id?: string | number;
  route_type?: string;
  mode?: string;
  cursor?: string;
  limit?: string | number;
}

export interface WAFLogQueryResult {
  date: string;
  available_dates: string[];
  cursor: string;
  next_cursor: string;
  has_more: boolean;
  limit: number;
  total: number;
  items: WAFEvent[];
}

export interface WAFLogDeleteResult {
  date: string;
  deleted: boolean;
  available_dates: string[];
}

export interface WAFLogRangeSeriesQuery {
  fromMs: number;
  toMs: number;
  bucketCount: number;
  actions?: string[];
}

export interface WAFLogRangeSeriesResult {
  total: number;
  series: Array<[number, number]>;
}

const pad2 = (value: number): string => String(value).padStart(2, "0");

const localDateFromMs = (timestamp: number): string => {
  const date = new Date(timestamp);
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(
    date.getDate(),
  )}`;
};

const today = () => localDateFromMs(Date.now());

const sortDatesDescending = (dates: Iterable<string>): string[] =>
  [...new Set(dates)].sort((a, b) => b.localeCompare(a));

const normalizeDate = (value?: string | null): string => {
  const raw = String(value ?? "").trim();
  if (!raw) return today();
  if (!DATE_RE.test(raw)) {
    throw new Error("invalid date, expected YYYY-MM-DD");
  }
  return raw;
};

const normalizeLimit = (value: unknown): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) return 50;
  return Math.min(200, parsed);
};

const normalizeCursor = (value: unknown): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed < 0) return 0;
  return parsed;
};

const scoreForEvent = (event: WAFEvent): number => {
  const parsed = Date.parse(event.time);
  return Number.isFinite(parsed) ? parsed : Date.now();
};

const dateForEvent = (event: WAFEvent): string =>
  localDateFromMs(scoreForEvent(event));

const scoreForDate = (date: string): number => {
  const parsed = Date.parse(`${date}T00:00:00`);
  return Number.isFinite(parsed) ? parsed : 0;
};

const datesForRange = (fromMs: number, toMs: number): string[] => {
  const dates: string[] = [];
  const start = new Date(fromMs);
  start.setHours(0, 0, 0, 0);
  const end = new Date(toMs);
  end.setHours(0, 0, 0, 0);
  for (
    const cursor = new Date(start);
    cursor <= end;
    cursor.setDate(cursor.getDate() + 1)
  ) {
    dates.push(localDateFromMs(cursor.getTime()));
  }
  return dates;
};

const buildBucketSeries = (
  fromMs: number,
  toMs: number,
  bucketCount: number,
): Array<[number, number]> => {
  const normalizedBucketCount = Math.max(1, Math.floor(bucketCount));
  const span = Math.max(1, toMs - fromMs);
  const step = Math.max(1, Math.ceil(span / normalizedBucketCount));
  return Array.from(
    { length: normalizedBucketCount },
    (_, index) => [fromMs + index * step, 0] as [number, number],
  );
};

const bucketStepForSeries = (series: Array<[number, number]>): number =>
  series.length > 1 && series[0] && series[1]
    ? Math.max(1, series[1][0] - series[0][0])
    : 1;

const bucketIndexForTimestamp = (
  timestamp: number,
  fromMs: number,
  step: number,
  bucketCount: number,
): number =>
  Math.min(bucketCount - 1, Math.max(0, Math.floor((timestamp - fromMs) / step)));

const parseEvent = (raw: string | null): WAFEvent | null => {
  if (!raw) return null;
  try {
    const event = JSON.parse(raw) as WAFEvent;
    return sanitizeEvent(event);
  } catch {
    return null;
  }
};

const ruleBasename = (value: unknown): string => {
  const normalized = String(value ?? "").replace(/\\/g, "/");
  return normalized.split("/").pop() || "";
};

const isInitializationRule = (rule: { file?: string }): boolean =>
  ruleBasename(rule.file).toLowerCase() ===
  INITIALIZATION_RULE_FILENAME.toLowerCase();

const isBlockingAction = (action: unknown): boolean => {
  const normalized = String(action || "").toLowerCase();
  return normalized === "block" || normalized === "deny";
};

const sanitizeEvent = (event: WAFEvent): WAFEvent | null => {
  if (!event?.trace_id) return null;

  const rules = Array.isArray(event.rules)
    ? event.rules.filter((rule) => !isInitializationRule(rule))
    : undefined;
  const initializationRuleIds = new Set(
    (event.rules || [])
      .filter(isInitializationRule)
      .map((rule) => rule.id)
      .filter((id) => Number.isFinite(id)),
  );
  const ruleIds = Array.isArray(event.rule_ids)
    ? event.rule_ids.filter((id) => !initializationRuleIds.has(id))
    : undefined;
  const interruption =
    event.interruption?.rule_id &&
    initializationRuleIds.has(event.interruption.rule_id)
      ? undefined
      : event.interruption;
  const hasRuleSignal = Boolean(rules?.length || ruleIds?.length);
  const hasBlockingSignal =
    isBlockingAction(event.action) || Boolean(interruption);

  if (!hasRuleSignal && !hasBlockingSignal) return null;

  return {
    ...event,
    ...(rules ? { rules } : {}),
    ...(ruleIds ? { rule_ids: ruleIds } : {}),
    interruption,
  };
};

const includesToken = (value: unknown, token: string): boolean =>
  String(value ?? "")
    .toLowerCase()
    .includes(token);

const eventMatches = (event: WAFEvent, query: WAFLogQuery): boolean => {
  const host = String(query.host ?? "")
    .trim()
    .toLowerCase();
  if (host && String(event.host ?? "").toLowerCase() !== host) return false;

  const clientIP = String(query.client_ip ?? "").trim();
  if (clientIP && event.client_ip !== clientIP) return false;

  const routeType = String(query.route_type ?? "").trim();
  if (routeType && event.route_type !== routeType) return false;

  const mode = String(query.mode ?? "").trim();
  if (mode && event.mode !== mode) return false;

  const rawRuleID = String(query.rule_id ?? "").trim();
  if (rawRuleID) {
    const ruleID = Number.parseInt(rawRuleID, 10);
    if (!Number.isFinite(ruleID) || !event.rule_ids?.includes(ruleID)) {
      return false;
    }
  }

  const search = String(query.search ?? "")
    .trim()
    .toLowerCase();
  if (search) {
    const haystack = [
      event.trace_id,
      event.host,
      event.path,
      event.request_uri,
      event.client_ip,
      event.route_key,
      event.upstream,
      event.bundle_id,
      ...(event.rule_ids ?? []),
    ];
    if (!haystack.some((value) => includesToken(value, search))) {
      return false;
    }
  }

  return true;
};

const hasLogFilters = (query: WAFLogQuery): boolean =>
  [
    query.search,
    query.host,
    query.client_ip,
    query.rule_id,
    query.route_type,
    query.mode,
  ].some((value) => String(value ?? "").trim());

export class WAFLogStore {
  async persistEvents(
    events: WAFEvent[],
    retentionDays: number,
  ): Promise<void> {
    const normalizedRetentionDays = Math.max(1, Math.min(365, retentionDays));
    const ttlSeconds = normalizedRetentionDays * 24 * 60 * 60;
    const pipeline = redis.pipeline();
    const touchedDates = new Set<string>();
    let operations = 0;

    for (const rawEvent of events) {
      const event = sanitizeEvent(rawEvent);
      if (!event) continue;
      const eventDate = dateForEvent(event);
      const score = scoreForEvent(event);
      touchedDates.add(eventDate);
      pipeline.set(
        eventKey(event.trace_id),
        JSON.stringify(event),
        "EX",
        ttlSeconds,
      );
      pipeline.zadd(dateKey(eventDate), score, event.trace_id);
      pipeline.expire(dateKey(eventDate), ttlSeconds);
      pipeline.hincrby(statsKey(eventDate), "events", 1);
      pipeline.hincrby(
        statsKey(eventDate),
        `action:${event.action || "log"}`,
        1,
      );
      pipeline.expire(statsKey(eventDate), ttlSeconds);
      operations += 6;
    }

    for (const date of touchedDates) {
      pipeline.zadd(DATES_INDEX_KEY, scoreForDate(date), date);
      operations += 1;
    }

    if (operations > 0) {
      await pipeline.exec();
    }
  }

  async listDates(): Promise<string[]> {
    const migrated = await redis.get(DATES_INDEX_MIGRATED_KEY);
    if (!migrated) {
      return this.scanDatesAndBackfillIndex();
    }
    return this.listDatesFromIndex();
  }

  private async listDatesFromIndex(): Promise<string[]> {
    const indexedDates = (await redis.zrevrange(DATES_INDEX_KEY, 0, -1)).filter(
      (date) => DATE_RE.test(date),
    );
    if (indexedDates.length === 0) {
      return [today()];
    }

    const counts = await Promise.all(
      indexedDates.map((date) => redis.zcard(dateKey(date))),
    );
    const keys = new Set<string>([today()]);
    const staleDates: string[] = [];
    indexedDates.forEach((date, index) => {
      if ((counts[index] ?? 0) > 0) {
        keys.add(date);
      } else {
        staleDates.push(date);
      }
    });

    if (staleDates.length > 0) {
      await redis.zrem(DATES_INDEX_KEY, ...staleDates);
    }

    return sortDatesDescending(keys);
  }

  private async scanDatesAndBackfillIndex(): Promise<string[]> {
    const dates = new Set<string>();
    let cursor = "0";
    do {
      const [nextCursor, batch] = await redis.scan(
        cursor,
        "MATCH",
        "fn_knock:waf:logs:*",
        "COUNT",
        100,
      );
      cursor = nextCursor;
      for (const key of batch) {
        const date = key.slice("fn_knock:waf:logs:".length);
        if (DATE_RE.test(date)) dates.add(date);
      }
    } while (cursor !== "0");

    const pipeline = redis.pipeline();
    for (const date of dates) {
      pipeline.zadd(DATES_INDEX_KEY, scoreForDate(date), date);
    }
    pipeline.set(DATES_INDEX_MIGRATED_KEY, "1");
    await pipeline.exec();

    return sortDatesDescending([today(), ...dates]);
  }

  async getEvent(traceId: string): Promise<WAFEvent | null> {
    const normalized = traceId.trim();
    if (!normalized) return null;
    return parseEvent(await redis.get(eventKey(normalized)));
  }

  async getRangeSeries(
    query: WAFLogRangeSeriesQuery,
  ): Promise<WAFLogRangeSeriesResult> {
    const fromMs = Math.max(0, query.fromMs);
    const toMs = Math.max(fromMs, query.toMs);
    const series = buildBucketSeries(fromMs, toMs, query.bucketCount);
    const step = bucketStepForSeries(series);
    const actions = new Set(
      (query.actions || [])
        .map((action) => action.trim().toLowerCase())
        .filter(Boolean),
    );
    let total = 0;

    if (actions.size === 0) {
      const pipeline = redis.pipeline();
      const requests: number[] = [];

      series.forEach(([bucketStart], bucketIndex) => {
        const bucketEnd =
          bucketIndex === series.length - 1
            ? toMs
            : Math.min(toMs, bucketStart + step);
        for (const date of datesForRange(bucketStart, bucketEnd)) {
          pipeline.zcount(
            dateKey(date),
            bucketStart,
            bucketIndex === series.length - 1 ? bucketEnd : `(${bucketEnd}`,
          );
          requests.push(bucketIndex);
        }
      });

      const results = (await pipeline.exec()) || [];
      results.forEach((result, index) => {
        const error = result?.[0];
        const value = Number(result?.[1] ?? 0);
        if (error || !Number.isFinite(value)) return;
        const bucketIndex = requests[index];
        if (bucketIndex === undefined) return;
        const bucket = series[bucketIndex];
        if (!bucket) return;
        bucket[1] += value;
        total += value;
      });

      return { total, series };
    }

    for (const date of datesForRange(fromMs, toMs)) {
      let offset = 0;
      while (true) {
        const pairs = await redis.zrangebyscore(
          dateKey(date),
          fromMs,
          toMs,
          "WITHSCORES",
          "LIMIT",
          offset,
          RANGE_QUERY_SCAN_CHUNK_SIZE,
        );
        if (pairs.length === 0) break;
        const returnedEntries = Math.floor(pairs.length / 2);

        const ids: string[] = [];
        for (let index = 0; index < pairs.length; index += 2) {
          const id = pairs[index];
          if (!id) continue;
          ids.push(id);
        }
        offset += returnedEntries;

        const raws = await redis.mget(ids.map(eventKey));
        raws.forEach((raw) => {
          const event = parseEvent(raw);
          if (!event) return;
          if (!actions.has(String(event.action || "").toLowerCase())) return;
          const timestamp = scoreForEvent(event);
          if (timestamp >= fromMs && timestamp <= toMs) {
            const bucketIndex = bucketIndexForTimestamp(
              timestamp,
              fromMs,
              step,
              series.length,
            );
            const bucket = series[bucketIndex];
            if (!bucket) return;
            bucket[1] += 1;
            total += 1;
          }
        });
      }
    }
    return { total, series };
  }

  async query(query: WAFLogQuery): Promise<WAFLogQueryResult> {
    const date = normalizeDate(query.date);
    const availableDates = await this.listDates();
    const limit = normalizeLimit(query.limit);
    const cursor = normalizeCursor(query.cursor);

    if (query.trace_id) {
      const event = await this.getEvent(String(query.trace_id));
      const items = event && eventMatches(event, query) ? [event] : [];
      return {
        date,
        available_dates: availableDates,
        cursor: String(cursor),
        next_cursor: "",
        has_more: false,
        limit,
        total: items.length,
        items,
      };
    }

    const page = hasLogFilters(query)
      ? await this.queryFiltered(date, query, cursor, limit)
      : await this.queryUnfiltered(date, cursor, limit);

    return {
      date,
      available_dates: availableDates,
      cursor: String(cursor),
      next_cursor: page.next_cursor,
      has_more: page.has_more,
      limit,
      total: page.total,
      items: page.items,
    };
  }

  private async queryUnfiltered(
    date: string,
    cursor: number,
    limit: number,
  ): Promise<
    Pick<WAFLogQueryResult, "items" | "next_cursor" | "has_more" | "total">
  > {
    const originalTotal = await redis.zcard(dateKey(date));
    const events: WAFEvent[] = [];
    const staleIds: string[] = [];
    let offset = cursor;

    while (events.length < limit + 1) {
      const ids = await redis.zrevrange(
        dateKey(date),
        offset,
        offset + UNFILTERED_QUERY_SCAN_CHUNK_SIZE - 1,
      );
      if (ids.length === 0) break;
      offset += ids.length;

      const batch = await this.getEventsByIds(ids);
      events.push(...batch.events);
      staleIds.push(...batch.staleIds);
    }

    await this.removeStaleIds(date, staleIds);

    if (events.length === 0) {
      return {
        next_cursor: "",
        has_more: false,
        total: Math.max(0, originalTotal - staleIds.length),
        items: [],
      };
    }

    const items = events.slice(0, limit);
    const hasMore = events.length > limit;
    const nextCursor = cursor + items.length;

    return {
      next_cursor: hasMore ? String(nextCursor) : "",
      has_more: hasMore,
      total: Math.max(0, originalTotal - staleIds.length),
      items,
    };
  }

  private async queryFiltered(
    date: string,
    query: WAFLogQuery,
    cursor: number,
    limit: number,
  ): Promise<
    Pick<WAFLogQueryResult, "items" | "next_cursor" | "has_more" | "total">
  > {
    let offset = 0;
    let matchedTotal = 0;
    const items: WAFEvent[] = [];
    const staleIds: string[] = [];

    while (true) {
      const ids = await redis.zrevrange(
        dateKey(date),
        offset,
        offset + FILTERED_QUERY_SCAN_CHUNK_SIZE - 1,
      );
      if (ids.length === 0) break;
      offset += ids.length;

      const batch = await this.getEventsByIds(ids);
      staleIds.push(...batch.staleIds);

      for (const event of batch.events) {
        if (!eventMatches(event, query)) continue;
        if (matchedTotal >= cursor && items.length < limit) {
          items.push(event);
        }
        matchedTotal += 1;
      }
    }

    await this.removeStaleIds(date, staleIds);

    const nextCursor = cursor + items.length;
    const hasMore = nextCursor < matchedTotal;

    return {
      next_cursor: hasMore ? String(nextCursor) : "",
      has_more: hasMore,
      total: matchedTotal,
      items,
    };
  }

  private async getEventsByIds(
    ids: string[],
  ): Promise<{ events: WAFEvent[]; staleIds: string[] }> {
    if (ids.length === 0) return { events: [], staleIds: [] };

    const raws = await redis.mget(ids.map(eventKey));
    const events: WAFEvent[] = [];
    const staleIds: string[] = [];

    raws.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      const event = parseEvent(raw);
      if (event) {
        events.push(event);
      } else {
        staleIds.push(id);
      }
    });

    return { events, staleIds };
  }

  private async removeStaleIds(date: string, ids: string[]): Promise<void> {
    const uniqueIds = [...new Set(ids)];
    for (
      let index = 0;
      index < uniqueIds.length;
      index += STALE_ID_REMOVE_CHUNK_SIZE
    ) {
      const chunk = uniqueIds.slice(index, index + STALE_ID_REMOVE_CHUNK_SIZE);
      if (chunk.length > 0) {
        await redis.zrem(dateKey(date), ...chunk);
      }
    }
  }

  async deleteDate(rawDate: string): Promise<WAFLogDeleteResult> {
    const date = normalizeDate(rawDate);
    let deletedCount = 0;

    while (true) {
      const ids = await redis.zrange(dateKey(date), 0, DELETE_DATE_CHUNK_SIZE - 1);
      if (ids.length === 0) break;

      const pipeline = redis.pipeline();
      for (const id of ids) {
        pipeline.del(eventKey(id));
      }
      pipeline.zrem(dateKey(date), ...ids);
      await pipeline.exec();
      deletedCount += ids.length;
    }

    await redis
      .pipeline()
      .del(dateKey(date))
      .del(statsKey(date))
      .zrem(DATES_INDEX_KEY, date)
      .exec();

    return {
      date,
      deleted: deletedCount > 0,
      available_dates: await this.listDates(),
    };
  }
}

export const wafLogStore = new WAFLogStore();
