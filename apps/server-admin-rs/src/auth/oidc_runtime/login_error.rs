use super::*;

pub(crate) async fn consume_login_error_for_bootstrap(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
) -> Option<(String, String)> {
    let token = cookies::read_cookie(headers, cookies::OIDC_LOGIN_ERROR_COOKIE_NAME)?;
    let notice = oidc_consume_login_error_notice(state, &hash_oidc_token(&token))
        .await
        .ok()
        .flatten()?;
    let message = notice
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let domain = resolve_cookie_domain(config, headers);
    let prefix = crate::auth::resolve_auth_ui_base_prefix(headers, uri);
    let path = if prefix.is_empty() { "/" } else { prefix };
    let clear_cookie = cookies::oidc_login_error_clear_cookie(domain.as_deref(), path);
    Some((message, clear_cookie))
}

pub(super) async fn consume_callback_state_for_notice(
    state: &AppState,
    provider_id: &str,
    state_token: Option<&str>,
    flow_token: Option<&str>,
) -> Option<Value> {
    let state_token = state_token?;
    if !oidc_flow_token_valid(state_token, flow_token) {
        return None;
    }
    oidc_consume_state(state, &hash_oidc_token(state_token))
        .await
        .ok()
        .flatten()
        .filter(|value| value.get("provider_id").and_then(Value::as_str) == Some(provider_id))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn login_error_redirect_response(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    message: String,
    translator: &Translator,
    redirect_uri: Option<&str>,
    persist_notice: bool,
    flow_clear_cookie: Option<String>,
) -> Response {
    let mut cookies = Vec::new();
    if persist_notice {
        let token = create_public_token();
        let token_hash = hash_oidc_token(&token);
        let notice = json!({
            "token_hash": token_hash,
            "message": normalize_login_error_message(&message, translator),
            "created_at": time_utils::now_iso(),
            "expires_at": time_utils::iso_after_seconds(LOGIN_ERROR_TTL_SECONDS as i64)
        });
        if let Err(error) =
            oidc_save_login_error_notice(state, &notice, LOGIN_ERROR_TTL_SECONDS).await
        {
            tracing::warn!(%error, "failed to save OIDC login error notice");
        } else {
            let domain = resolve_cookie_domain(config, headers);
            let path = resolve_oidc_cookie_path(config, headers, uri.path());
            cookies.push(cookies::oidc_login_error_cookie(
                &token,
                LOGIN_ERROR_TTL_SECONDS as i64,
                domain.as_deref(),
                &path,
            ));
        }
    }
    if let Some(cookie) = flow_clear_cookie {
        cookies.push(cookie);
    }
    let location = build_login_redirect(config, headers, uri.path(), redirect_uri);
    redirect_response(&location, cookies)
}

pub(super) fn provider_error_message(error: &str, translator: &Translator) -> String {
    match error.trim().to_ascii_lowercase().as_str() {
        "access_denied" => oidc_text(translator, "providerErrors.accessDenied"),
        "temporarily_unavailable" => oidc_text(translator, "providerErrors.temporarilyUnavailable"),
        "server_error" => oidc_text(translator, "providerErrors.serverError"),
        "invalid_scope" => oidc_text(translator, "providerErrors.invalidScope"),
        "invalid_request" | "unauthorized_client" | "unsupported_response_type" => {
            oidc_text(translator, "providerErrors.rejected")
        }
        _ => oidc_text(translator, "providerErrors.incomplete"),
    }
}

pub(super) fn is_oidc_operation_aborted_error(error: &str) -> bool {
    let message = error.to_ascii_lowercase();
    message.contains("operation was aborted")
        || (message.contains("aborterror") && message.contains("aborted"))
}

pub(super) fn oidc_login_failed_retry_after_message(
    translator: &Translator,
    message: &str,
    retry_after: i64,
) -> String {
    oidc_text_params(
        translator,
        "loginFailedRetryAfter",
        &[
            ("message", message.to_string()),
            ("seconds", retry_after.max(1).to_string()),
        ],
    )
}

pub(super) fn redirect_response(location: &str, cookies: Vec<String>) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()));
    apply_no_store_headers(response.headers_mut());
    for cookie in cookies {
        append_set_cookie(response.headers_mut(), &cookie);
    }
    response
}

pub(super) fn bind_provider_selection_response(
    uri: &Uri,
    token: &str,
    invite: &Value,
    providers: &[Value],
    translator: &Translator,
    locale: &str,
) -> Response {
    let totp_name = invite
        .pointer("/totp/comment")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("TOTP");
    let actions = providers
        .iter()
        .filter_map(|provider| {
            let id = provider.get("id").and_then(Value::as_str)?;
            let name = provider.get("name").and_then(Value::as_str).unwrap_or(id);
            let query = format!(
                "token={}&provider_id={}",
                encode_query(token),
                encode_query(id)
            );
            Some(format!(
                r#"<a href="{}?{}">{}</a>"#,
                html_escape(uri.path()),
                query,
                html_escape(&oidc_text_params(
                    translator,
                    "bindWithProvider",
                    &[("provider", name.to_string())],
                ))
            ))
        })
        .collect::<String>();
    bind_html_response(
        StatusCode::OK,
        &oidc_text(translator, "selectProviderTitle"),
        &oidc_text_params(translator, "bindToTotp", &[("totp", totp_name.to_string())]),
        locale,
        Some(format!(r#"<div class="actions">{actions}</div>"#)),
    )
}

pub(super) fn bind_html_response(
    status: StatusCode,
    title: &str,
    body: &str,
    locale: &str,
    actions: Option<String>,
) -> Response {
    let html = format!(
        r#"<!doctype html>
<html lang="{locale}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <style>
      body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#f6f7f9;color:#111827;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
      main{{width:min(92vw,420px);box-sizing:border-box;border:1px solid #e5e7eb;border-radius:12px;background:#fff;padding:28px;box-shadow:0 18px 48px rgba(15,23,42,.08)}}
      h1{{margin:0 0 10px;font-size:22px;line-height:1.25}}
      p{{margin:0;color:#4b5563;line-height:1.7;font-size:14px}}
      .actions{{display:grid;gap:10px;margin-top:22px}}
      a{{display:flex;align-items:center;justify-content:center;height:40px;border-radius:8px;background:#111827;color:#fff;text-decoration:none;font-size:14px;font-weight:600}}
    </style>
  </head>
  <body>
    <main>
      <h1>{title}</h1>
      <p>{body}</p>
      {actions}
    </main>
  </body>
</html>"#,
        locale = html_escape(locale),
        title = html_escape(title),
        body = html_escape(body),
        actions = actions.unwrap_or_default(),
    );
    let mut response = (
        status,
        [
            ("content-type", "text/html; charset=utf-8"),
            ("x-content-type-options", "nosniff"),
            ("x-frame-options", "DENY"),
            ("referrer-policy", "no-referrer"),
        ],
        html,
    )
        .into_response();
    response.headers_mut().insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
    apply_no_store_headers(response.headers_mut());
    response
}
