export type CaptchaProvider = 'pow' | 'turnstile';

export type CaptchaWidgetMode = 'normal';

export type TurnstileCaptchaConfig = {
    site_key: string;
    secret_key: string;
};

export type PowCaptchaConfig = {
    base_max_number: number;
    uncommon_location: {
        enabled: boolean;
        max_number: number;
    };
};

export type CaptchaSettings = {
    provider: CaptchaProvider;
    widget_mode: CaptchaWidgetMode;
    pow: PowCaptchaConfig;
    turnstile: TurnstileCaptchaConfig;
};

export type CaptchaPublicSettings = {
    provider: CaptchaProvider;
    widget_mode: CaptchaWidgetMode;
    available: boolean;
    unavailable_reason: string | null;
    pow: Record<string, never>;
    turnstile: {
        site_key: string;
    };
};

export type CaptchaSubmission =
    | {
        provider: 'pow';
        proof: string;
    }
    | {
        provider: 'turnstile';
        token: string;
    };
