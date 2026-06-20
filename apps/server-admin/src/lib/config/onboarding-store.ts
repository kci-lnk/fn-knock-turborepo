import type Redis from "ioredis";

import { DEFAULT_RUN_MODE_PROMPT_PREFERENCES } from "./defaults";
import type { RunModePromptPreferences, WelcomeGuideStatus } from "./types";

const RUN_MODE_PROMPT_PREFERENCES_KEY =
  "fn_knock:run-mode:prompt-preferences";
const WELCOME_GUIDE_STATUS_KEY = "fn_knock:welcome-guide:status";

export class OnboardingStore {
  constructor(private readonly redis: Redis) {}

  async getRunModePromptPreferences(): Promise<RunModePromptPreferences> {
    const raw = await this.redis.get(RUN_MODE_PROMPT_PREFERENCES_KEY);
    if (!raw) return DEFAULT_RUN_MODE_PROMPT_PREFERENCES;

    try {
      const parsed = JSON.parse(raw) as Partial<RunModePromptPreferences>;
      return {
        directToReverseProxy: parsed.directToReverseProxy === true,
        reverseProxyToDirect: parsed.reverseProxyToDirect === true,
        switchToSubdomain: parsed.switchToSubdomain === true,
        subdomainToReverseProxy: parsed.subdomainToReverseProxy === true,
      };
    } catch {
      return DEFAULT_RUN_MODE_PROMPT_PREFERENCES;
    }
  }

  async updateRunModePromptPreferences(
    patch: Partial<RunModePromptPreferences>,
  ): Promise<RunModePromptPreferences> {
    const next = {
      ...(await this.getRunModePromptPreferences()),
      ...patch,
    };
    await this.redis.set(
      RUN_MODE_PROMPT_PREFERENCES_KEY,
      JSON.stringify(next),
    );
    return next;
  }

  async getWelcomeGuideStatus(): Promise<WelcomeGuideStatus> {
    const raw = await this.redis.get(WELCOME_GUIDE_STATUS_KEY);
    if (!raw) {
      return {
        completed: false,
        completed_at: null,
      };
    }

    if (raw === "1" || raw === "true") {
      return {
        completed: true,
        completed_at: null,
      };
    }

    try {
      const parsed = JSON.parse(raw) as Partial<WelcomeGuideStatus>;
      return {
        completed: parsed.completed === true,
        completed_at:
          typeof parsed.completed_at === "string" && parsed.completed_at.trim()
            ? parsed.completed_at
            : null,
      };
    } catch {
      return {
        completed: false,
        completed_at: null,
      };
    }
  }

  async completeWelcomeGuide(): Promise<WelcomeGuideStatus> {
    const current = await this.getWelcomeGuideStatus();
    const next: WelcomeGuideStatus = {
      completed: true,
      completed_at: current.completed_at ?? new Date().toISOString(),
    };
    await this.redis.set(WELCOME_GUIDE_STATUS_KEY, JSON.stringify(next));
    return next;
  }
}
