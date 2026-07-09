use super::*;

pub(super) async fn build_standard_oidc_authorization_url(
    provider: &Value,
    callback_url: &str,
    state_token: &str,
    nonce: &str,
    code_verifier: &str,
    translator: &Translator,
) -> Result<String, String> {
    let discovery = resolve_discovery_with_translator(provider, translator).await?;
    let config = provider_config(provider, translator)?;
    let client_id = string_field(config, "client_id")
        .ok_or_else(|| oidc_text(translator, "clientIdMissing"))?;
    let mut url = Url::parse(
        discovery
            .get("authorization_endpoint")
            .and_then(Value::as_str)
            .ok_or_else(|| oidc_text(translator, "authorizationEndpointMissing"))?,
    )
    .map_err(|_| oidc_text(translator, "authorizationEndpointInvalid"))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("client_id", client_id);
        params.append_pair("response_type", "code");
        params.append_pair("redirect_uri", callback_url);
        params.append_pair(
            "scope",
            &scopes(config, &["openid", "profile", "email"]).join(" "),
        );
        params.append_pair("state", state_token);
        params.append_pair("nonce", nonce);
        params.append_pair("code_challenge", &create_pkce_challenge(code_verifier));
        params.append_pair("code_challenge_method", "S256");
        for (key, value) in extra_auth_params(config) {
            params.append_pair(&key, &value);
        }
    }
    Ok(url.to_string())
}

pub(super) fn build_oauth_profile_authorization_url(
    provider: &Value,
    callback_url: &str,
    state_token: &str,
    translator: &Translator,
) -> Result<String, String> {
    let config = provider_config(provider, translator)?;
    let client_id = string_field(config, "client_id")
        .ok_or_else(|| oidc_text(translator, "clientIdMissing"))?;
    let endpoint = string_field(config, "authorization_endpoint")
        .ok_or_else(|| oidc_text(translator, "authorizationEndpointMissing"))?;
    let mut url =
        Url::parse(endpoint).map_err(|_| oidc_text(translator, "authorizationEndpointInvalid"))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("client_id", client_id);
        params.append_pair("response_type", "code");
        params.append_pair("redirect_uri", callback_url);
        params.append_pair("scope", &scopes(config, &[]).join(" "));
        params.append_pair("state", state_token);
        for (key, value) in extra_auth_params(config) {
            params.append_pair(&key, &value);
        }
    }
    Ok(url.to_string())
}

pub(super) async fn resolve_standard_oidc_callback(
    state: &AppState,
    provider: &Value,
    code: &str,
    callback_url: &str,
    auth_state: &Value,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let discovery = resolve_discovery_with_translator(provider, translator).await?;
    let config = provider_config(provider, translator)?;
    let token_endpoint = discovery
        .get("token_endpoint")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "tokenEndpointMissing"))?;
    let client_id = string_field(config, "client_id").unwrap_or("");
    let client_secret = string_field(config, "client_secret").unwrap_or("");
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("client_id", client_id.to_string()),
        ("client_secret", client_secret.to_string()),
        ("code", code.to_string()),
        ("redirect_uri", callback_url.to_string()),
    ];
    if let Some(code_verifier) = auth_state.get("code_verifier").and_then(Value::as_str) {
        form.push(("code_verifier", code_verifier.to_string()));
    }
    let token_payload = exchange_form_token(state, token_endpoint, &form, None, translator).await?;
    verify_standard_oidc_profile(
        state,
        provider,
        &token_payload,
        &discovery,
        auth_state.get("nonce").and_then(Value::as_str),
        translator,
    )
    .await
}

pub(super) async fn resolve_oauth_profile_callback(
    state: &AppState,
    provider: &Value,
    code: &str,
    callback_url: &str,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let config = provider_config(provider, translator)?;
    let token_endpoint = string_field(config, "token_endpoint")
        .ok_or_else(|| oidc_text(translator, "tokenEndpointMissing"))?;
    let client_id = string_field(config, "client_id").unwrap_or("");
    let client_secret = string_field(config, "client_secret").unwrap_or("");
    let form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("client_id", client_id.to_string()),
        ("client_secret", client_secret.to_string()),
        ("code", code.to_string()),
        ("redirect_uri", callback_url.to_string()),
    ];
    let headers = (provider.get("type").and_then(Value::as_str) == Some("github"))
        .then_some(vec![("Accept", "application/json")]);
    let token_payload =
        exchange_form_token(state, token_endpoint, &form, headers, translator).await?;
    let access_token = string_field_from_value(&token_payload, "access_token")
        .ok_or_else(|| oidc_text(translator, "accessTokenMissing"))?;
    if provider.get("type").and_then(Value::as_str) == Some("github") {
        return fetch_github_profile(state, provider, access_token, translator).await;
    }
    Err(oidc_text(translator, "providerUnsupported"))
}

pub(super) async fn exchange_form_token(
    state: &AppState,
    endpoint: &str,
    fields: &[(&str, String)],
    extra_headers: Option<Vec<(&str, &str)>>,
    translator: &Translator,
) -> Result<Value, String> {
    let body = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in fields {
            serializer.append_pair(key, value);
        }
        serializer.finish()
    };
    let mut request = oidc_http_request(state.fallback_client.post(endpoint), "application/json")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body);
    for (key, value) in extra_headers.unwrap_or_default() {
        request = request.header(key, value);
    }
    let response = request.send().await.map_err(|error| {
        oidc_text_params(
            translator,
            "tokenRequestFailed",
            &[("detail", error.to_string())],
        )
    })?;
    parse_http_payload(response, translator).await
}

pub(super) fn oidc_http_request(
    request: reqwest::RequestBuilder,
    accept: &'static str,
) -> reqwest::RequestBuilder {
    request
        .header(header::ACCEPT, accept)
        .header(header::USER_AGENT, OIDC_HTTP_USER_AGENT)
}

pub(super) async fn parse_http_payload(
    response: reqwest::Response,
    translator: &Translator,
) -> Result<Value, String> {
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let text = response.text().await.map_err(|error| {
        oidc_text_params(
            translator,
            "readResponseFailed",
            &[("detail", error.to_string())],
        )
    })?;
    if !status.is_success() {
        return Err(oidc_text_params(
            translator,
            "httpResponseFailed",
            &[
                ("status", status.to_string()),
                ("detail", text.chars().take(160).collect::<String>()),
            ],
        ));
    }
    parse_json_or_form(&text, &content_type, translator)
}

pub(super) fn parse_json_or_form(
    text: &str,
    content_type: &str,
    translator: &Translator,
) -> Result<Value, String> {
    let trimmed = text.trim();
    if content_type.contains("json") || trimmed.starts_with('{') {
        serde_json::from_str(trimmed).map_err(|error| {
            oidc_text_params(
                translator,
                "jsonResponseInvalid",
                &[("detail", error.to_string())],
            )
        })
    } else {
        let object = url::form_urlencoded::parse(trimmed.as_bytes())
            .map(|(key, value)| (key.into_owned(), Value::String(value.into_owned())))
            .collect::<Map<_, _>>();
        Ok(Value::Object(object))
    }
}

pub(super) async fn verify_standard_oidc_profile(
    state: &AppState,
    provider: &Value,
    token_payload: &Value,
    discovery: &Value,
    expected_nonce: Option<&str>,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let id_token = token_payload
        .get("id_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| oidc_text(translator, "idTokenMissing"))?;
    let jwks_uri = discovery
        .get("jwks_uri")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "jwksUriMissing"))?;
    let jwks = oidc_http_request(state.fallback_client.get(jwks_uri), "application/json")
        .send()
        .await
        .map_err(|error| {
            oidc_text_params(
                translator,
                "jwksFetchFailed",
                &[("detail", error.to_string())],
            )
        })?
        .json::<JwkSet>()
        .await
        .map_err(|error| {
            oidc_text_params(translator, "jwksInvalid", &[("detail", error.to_string())])
        })?;
    let header = decode_header(id_token).map_err(|error| {
        oidc_text_params(
            translator,
            "tokenHeaderInvalid",
            &[("detail", error.to_string())],
        )
    })?;
    let jwk = select_jwk(&jwks, header.kid.as_deref())
        .ok_or_else(|| oidc_text(translator, "signingKeyUnavailable"))?;
    let decoding_key = DecodingKey::from_jwk(jwk).map_err(|error| {
        oidc_text_params(
            translator,
            "signingKeyInvalid",
            &[("detail", error.to_string())],
        )
    })?;
    let config = provider_config(provider, translator)?;
    let client_id = string_field(config, "client_id").unwrap_or("");
    let discovery_issuer = discovery
        .get("issuer")
        .and_then(Value::as_str)
        .unwrap_or("");
    let issuer_for_verify = (!discovery_issuer.contains("{tenantid}")).then_some(discovery_issuer);
    let mut validation = Validation::new(header.alg);
    validation.set_audience(&[client_id]);
    if let Some(issuer) = issuer_for_verify {
        validation.set_issuer(&[issuer]);
    }
    let token = decode::<Value>(id_token, &decoding_key, &validation).map_err(|error| {
        oidc_text_params(
            translator,
            "idTokenVerificationFailed",
            &[("detail", error.to_string())],
        )
    })?;
    let payload = token.claims;
    if let Some(expected_nonce) = expected_nonce
        && payload.get("nonce").and_then(Value::as_str) != Some(expected_nonce)
    {
        return Err(oidc_text(translator, "nonceCheckFailed"));
    }
    if issuer_for_verify.is_none() {
        let issuer = payload.get("iss").and_then(Value::as_str).unwrap_or("");
        if !issuer.starts_with("https://login.microsoftonline.com/") {
            return Err(oidc_text(translator, "issuerCheckFailed"));
        }
    }
    let subject = payload
        .get("sub")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| oidc_text(translator, "subjectEmpty"))?;
    let mut userinfo = Value::Object(Map::new());
    if let Some(endpoint) = discovery
        .get("userinfo_endpoint")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        && let Some(access_token) = token_payload.get("access_token").and_then(Value::as_str)
        && let Ok(response) =
            oidc_http_request(state.fallback_client.get(endpoint), "application/json")
                .bearer_auth(access_token)
                .send()
                .await
        && response.status().is_success()
        && let Ok(payload) = parse_http_payload(response, translator).await
    {
        userinfo = payload;
    }
    let pick = |key: &str| userinfo.get(key).or_else(|| payload.get(key));
    Ok(ExternalProfile {
        issuer: payload
            .get("iss")
            .and_then(Value::as_str)
            .unwrap_or(discovery_issuer)
            .to_string(),
        subject: subject.to_string(),
        display_name: optional_string(pick("name"))
            .or_else(|| optional_string(pick("preferred_username"))),
        email: optional_string(pick("email")),
        email_verified: Some(value_truthy(pick("email_verified"))),
        avatar_url: optional_string(pick("picture")),
    })
}

pub(super) fn select_jwk<'a>(jwks: &'a JwkSet, kid: Option<&str>) -> Option<&'a Jwk> {
    if let Some(kid) = kid
        && let Some(jwk) = jwks
            .keys
            .iter()
            .find(|jwk| jwk.common.key_id.as_deref() == Some(kid))
    {
        return Some(jwk);
    }
    jwks.keys.first()
}

pub(super) async fn fetch_github_profile(
    state: &AppState,
    provider: &Value,
    access_token: &str,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let config = provider_config(provider, translator)?;
    let user_endpoint =
        string_field(config, "userinfo_endpoint").unwrap_or("https://api.github.com/user");
    let user = github_api_request(&state.fallback_client, user_endpoint, access_token)
        .send()
        .await
        .map_err(|error| {
            oidc_text_params(
                translator,
                "githubProfileRequestFailed",
                &[("detail", error.to_string())],
            )
        })?;
    let user = parse_http_payload(user, translator).await?;
    let subject = optional_string(user.get("id"))
        .or_else(|| {
            user.get("id")
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
        })
        .ok_or_else(|| oidc_text(translator, "githubUserIdEmpty"))?;
    let mut email = optional_string(user.get("email"));
    let mut email_verified = false;
    if let Some(endpoint) = string_field(config, "emails_endpoint")
        && let Ok(response) = github_api_request(&state.fallback_client, endpoint, access_token)
            .send()
            .await
        && response.status().is_success()
        && let Ok(emails) = response.json::<Value>().await
        && let Some(items) = emails.as_array()
        && let Some(primary) = items
            .iter()
            .find(|item| item.get("primary").and_then(Value::as_bool) == Some(true))
            .or_else(|| items.first())
    {
        email = optional_string(primary.get("email")).or(email);
        email_verified = primary
            .get("verified")
            .and_then(Value::as_bool)
            .unwrap_or(email.is_some());
    }
    Ok(ExternalProfile {
        issuer: "github".to_string(),
        subject,
        display_name: optional_string(user.get("name"))
            .or_else(|| optional_string(user.get("login"))),
        email,
        email_verified: Some(email_verified),
        avatar_url: optional_string(user.get("avatar_url")),
    })
}

pub(super) fn github_api_request(
    client: &reqwest::Client,
    endpoint: &str,
    access_token: &str,
) -> reqwest::RequestBuilder {
    oidc_http_request(client.get(endpoint), "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .bearer_auth(access_token)
}
