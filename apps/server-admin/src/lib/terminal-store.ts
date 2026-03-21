import { redis } from "./redis";
import {
  DEFAULT_TERMINAL_ATTACHMENT_TTL_SECONDS,
  TerminalAttachmentRecord,
  TerminalSessionRecord,
  normalizeTerminalAttachmentRecord,
  normalizeTerminalSessionRecord,
} from "./terminal-shared";

class TerminalStore {
  private readonly sessionIndexKey = "fn_knock:terminal:session:index";
  private readonly sessionDataPrefix = "fn_knock:terminal:session:data:";
  private readonly sessionAttachmentPrefix =
    "fn_knock:terminal:session:attachments:";
  private readonly attachmentDataPrefix = "fn_knock:terminal:attachment:data:";

  private sessionDataKey(id: string) {
    return `${this.sessionDataPrefix}${id}`;
  }

  private sessionAttachmentsKey(sessionId: string) {
    return `${this.sessionAttachmentPrefix}${sessionId}`;
  }

  private attachmentDataKey(id: string) {
    return `${this.attachmentDataPrefix}${id}`;
  }

  private async pruneSessions(ids: string[]): Promise<void> {
    if (ids.length === 0) return;
    const pipeline = redis.multi();
    pipeline.zrem(this.sessionIndexKey, ...ids);
    ids.forEach((id) => {
      pipeline.del(this.sessionDataKey(id));
    });
    await pipeline.exec();
  }

  private async pruneSessionAttachments(
    sessionId: string,
    attachmentIds: string[],
  ): Promise<void> {
    if (attachmentIds.length === 0) return;
    const pipeline = redis.multi();
    pipeline.srem(this.sessionAttachmentsKey(sessionId), ...attachmentIds);
    attachmentIds.forEach((id) => {
      pipeline.del(this.attachmentDataKey(id));
    });
    await pipeline.exec();
  }

  async listSessions(): Promise<TerminalSessionRecord[]> {
    const ids = await redis.zrevrange(this.sessionIndexKey, 0, -1);
    if (ids.length === 0) return [];
    const raws = await redis.mget(ids.map((id) => this.sessionDataKey(id)));

    const sessions: TerminalSessionRecord[] = [];
    const staleIds: string[] = [];
    raws.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      if (!raw) {
        staleIds.push(id);
        return;
      }
      try {
        sessions.push(
          normalizeTerminalSessionRecord(
            JSON.parse(raw) as TerminalSessionRecord,
          ),
        );
      } catch (error) {
        console.error("[terminal] failed to parse session record:", error);
        staleIds.push(id);
      }
    });
    if (staleIds.length > 0) {
      await this.pruneSessions(staleIds);
    }
    return sessions;
  }

  async getSession(id: string): Promise<TerminalSessionRecord | null> {
    const raw = await redis.get(this.sessionDataKey(id));
    if (!raw) return null;
    try {
      return normalizeTerminalSessionRecord(
        JSON.parse(raw) as TerminalSessionRecord,
      );
    } catch (error) {
      console.error("[terminal] failed to parse session record:", error);
      await redis
        .multi()
        .del(this.sessionDataKey(id))
        .zrem(this.sessionIndexKey, id)
        .exec();
      return null;
    }
  }

  async saveSession(
    session: TerminalSessionRecord,
  ): Promise<TerminalSessionRecord> {
    const normalized = normalizeTerminalSessionRecord(session);
    const score = Date.parse(normalized.updated_at) || Date.now();
    await redis
      .multi()
      .set(this.sessionDataKey(normalized.id), JSON.stringify(normalized))
      .zadd(this.sessionIndexKey, score, normalized.id)
      .exec();
    return normalized;
  }

  async deleteSession(id: string): Promise<void> {
    const attachmentIds = await redis.smembers(this.sessionAttachmentsKey(id));
    const pipeline = redis.multi();
    pipeline.del(this.sessionDataKey(id));
    pipeline.zrem(this.sessionIndexKey, id);
    pipeline.del(this.sessionAttachmentsKey(id));
    attachmentIds.forEach((attachmentId) => {
      pipeline.del(this.attachmentDataKey(attachmentId));
    });
    await pipeline.exec();
  }

  async listAttachmentIdsForSession(sessionId: string): Promise<string[]> {
    const ids = await redis.smembers(this.sessionAttachmentsKey(sessionId));
    if (ids.length === 0) return [];

    const raws = await redis.mget(ids.map((id) => this.attachmentDataKey(id)));
    const liveIds: string[] = [];
    const staleIds: string[] = [];

    raws.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      if (!raw) {
        staleIds.push(id);
        return;
      }

      try {
        normalizeTerminalAttachmentRecord(
          JSON.parse(raw) as TerminalAttachmentRecord,
        );
        liveIds.push(id);
      } catch (error) {
        console.error("[terminal] failed to parse attachment record:", error);
        staleIds.push(id);
      }
    });

    if (staleIds.length > 0) {
      await this.pruneSessionAttachments(sessionId, staleIds);
    }

    return liveIds;
  }

  async getAttachment(id: string): Promise<TerminalAttachmentRecord | null> {
    const raw = await redis.get(this.attachmentDataKey(id));
    if (!raw) return null;
    try {
      return normalizeTerminalAttachmentRecord(
        JSON.parse(raw) as TerminalAttachmentRecord,
      );
    } catch (error) {
      console.error("[terminal] failed to parse attachment record:", error);
      await redis.del(this.attachmentDataKey(id));
      return null;
    }
  }

  async saveAttachment(
    attachment: TerminalAttachmentRecord,
    ttlSeconds = DEFAULT_TERMINAL_ATTACHMENT_TTL_SECONDS,
  ): Promise<TerminalAttachmentRecord> {
    const normalized = normalizeTerminalAttachmentRecord(attachment);
    await redis
      .multi()
      .set(
        this.attachmentDataKey(normalized.id),
        JSON.stringify(normalized),
        "EX",
        Math.max(30, ttlSeconds),
      )
      .sadd(this.sessionAttachmentsKey(normalized.session_id), normalized.id)
      .expire(
        this.sessionAttachmentsKey(normalized.session_id),
        Math.max(60, ttlSeconds),
      )
      .exec();
    return normalized;
  }

  async refreshAttachment(
    id: string,
    ttlSeconds = DEFAULT_TERMINAL_ATTACHMENT_TTL_SECONDS,
  ): Promise<TerminalAttachmentRecord | null> {
    const attachment = await this.getAttachment(id);
    if (!attachment) return null;
    const next = normalizeTerminalAttachmentRecord({
      ...attachment,
      updated_at: new Date().toISOString(),
      expires_at: new Date(
        Date.now() + Math.max(30, ttlSeconds) * 1000,
      ).toISOString(),
    });
    await this.saveAttachment(next, ttlSeconds);
    return next;
  }

  async deleteAttachment(id: string): Promise<void> {
    const attachment = await this.getAttachment(id);
    if (!attachment) {
      await redis.del(this.attachmentDataKey(id));
      return;
    }
    await redis
      .multi()
      .del(this.attachmentDataKey(id))
      .srem(this.sessionAttachmentsKey(attachment.session_id), id)
      .exec();
  }
}

export const terminalStore = new TerminalStore();
