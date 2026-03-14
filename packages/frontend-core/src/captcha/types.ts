export type CaptchaProvider = 'pow' | 'turnstile';

export type CaptchaWidgetMode = 'normal';

export type TurnstileCaptchaConfig = {
    site_key: string;
    secret_key: string;
};

export type CaptchaSettings = {
    provider: CaptchaProvider;
    widget_mode: CaptchaWidgetMode;
    pow: Record<string, never>;
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
