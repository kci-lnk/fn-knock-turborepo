export const POST_LOGIN_LOGOUT_DELAY_MS = 10_000;

const POST_LOGIN_LOGOUT_DELAY_KEY = 'fn-knock:auth:delay-logout-once';

export function markPendingLogoutDelay() {
    if (typeof window === 'undefined') {
        return;
    }

    window.sessionStorage.setItem(POST_LOGIN_LOGOUT_DELAY_KEY, '1');
}

export function consumePendingLogoutDelay() {
    if (typeof window === 'undefined') {
        return false;
    }

    const shouldDelay = window.sessionStorage.getItem(POST_LOGIN_LOGOUT_DELAY_KEY) === '1';
    if (shouldDelay) {
        window.sessionStorage.removeItem(POST_LOGIN_LOGOUT_DELAY_KEY);
    }

    return shouldDelay;
}
