use super::*;

pub(super) async fn verify_captcha(
    state: &AppState,
    config: &Value,
    submission: &CaptchaSubmission,
    client_ip: &str,
    translator: &Translator,
) -> Result<(), String> {
    let settings = config
        .get("captcha")
        .cloned()
        .unwrap_or_else(|| json!({ "provider": "pow" }));
    let provider = settings
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("pow");
    let submitted_provider = captcha_submission_provider(submission);
    if provider != submitted_provider {
        return Err(captcha_text(translator, "providerConfigMismatch"));
    }

    match (provider, submission) {
        ("pow", CaptchaSubmission::Pow { proof }) => {
            verify_pow_captcha(state, proof, translator).await
        }
        ("turnstile", CaptchaSubmission::Turnstile { token }) => {
            verify_turnstile_captcha(state, &settings, token, client_ip, translator).await
        }
        _ => Err(captcha_text(translator, "providerUnavailable")),
    }
}

pub(super) fn captcha_submission_provider(submission: &CaptchaSubmission) -> &'static str {
    match submission {
        CaptchaSubmission::Pow { .. } => "pow",
        CaptchaSubmission::Turnstile { .. } => "turnstile",
    }
}

pub(super) async fn verify_pow_captcha(
    state: &AppState,
    proof: &str,
    translator: &Translator,
) -> Result<(), String> {
    let Some(key) = state
        .settings
        .altcha_hmac_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(captcha_text(translator, "powServerNotConfigured"));
    };
    let decoded = BASE64_STANDARD
        .decode(proof)
        .map_err(|_| auth_route_text(translator, "invalidCaptchaProof"))?;
    let data: PowProof = serde_json::from_slice(&decoded)
        .map_err(|_| auth_route_text(translator, "invalidCaptchaProof"))?;
    let validation = validate_pow_proof(data, key, time_utils::now_ms() / 1000, translator)?;
    match state
        .store
        .set_nonce_if_not_exists(&validation.nonce, 86_400)
        .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(auth_route_text(translator, "captchaChallengeAlreadyUsed")),
        Err(error) => {
            tracing::warn!(%error, "failed to store captcha nonce");
            Err(auth_route_text(translator, "captchaVerifyFailed"))
        }
    }
}

pub(super) fn validate_pow_proof(
    data: PowProof,
    key: &str,
    now_seconds: i64,
    translator: &Translator,
) -> Result<PowValidation, String> {
    if data.algorithm.as_deref() != Some("SHA-256") {
        return Err(auth_route_text(translator, "invalidCaptchaAlgorithm"));
    }

    let raw_challenge = data.challenge.unwrap_or_default();
    let challenge = raw_challenge.to_ascii_lowercase();
    let number = pow_number_text(data.number.as_ref());
    let salt = data.salt.unwrap_or_default();
    let signature = data.signature.unwrap_or_default().to_ascii_lowercase();
    let expected_challenge = sha256_hex(format!("{salt}{number}").as_bytes());
    if expected_challenge
        .as_bytes()
        .ct_eq(challenge.as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(auth_route_text(translator, "invalidCaptchaChallenge"));
    }

    let expected_signature = hmac_sha256_hex(key.as_bytes(), raw_challenge.as_bytes());
    if expected_signature
        .as_bytes()
        .ct_eq(signature.as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(auth_route_text(translator, "invalidCaptchaSignature"));
    }

    if let Some(expires) = parse_pow_expires(&salt)
        && now_seconds > expires
    {
        return Err(auth_route_text(translator, "captchaChallengeExpired"));
    }

    Ok(PowValidation {
        nonce: raw_challenge,
    })
}

pub(super) async fn verify_turnstile_captcha(
    state: &AppState,
    settings: &Value,
    token: &str,
    client_ip: &str,
    translator: &Translator,
) -> Result<(), String> {
    let secret_key = settings
        .pointer("/turnstile/secret_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if secret_key.is_empty() {
        return Err(captcha_text(translator, "turnstileSecretMissing"));
    }
    if token.trim().is_empty() {
        return Err(captcha_text(translator, "turnstileTokenRequired"));
    }

    let body = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        serializer.append_pair("secret", &secret_key);
        serializer.append_pair("response", token.trim());
        if !client_ip.is_empty() {
            serializer.append_pair("remoteip", client_ip);
        }
        serializer.finish()
    };
    let response = state
        .fallback_client
        .post(TURNSTILE_VERIFY_URL)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|_| captcha_text(translator, "turnstileServiceUnavailable"))?;
    if !response.status().is_success() {
        return Err(captcha_text(translator, "turnstileServiceUnavailable"));
    }
    let result = response
        .json::<Value>()
        .await
        .map_err(|_| auth_route_text(translator, "turnstileResponseInvalid"))?;
    if result.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else if let Some(reason) = turnstile_error_reason(&result) {
        Err(captcha_text_params(
            translator,
            "turnstileVerifyFailedWithReason",
            &[("reason", reason)],
        ))
    } else {
        Err(captcha_text(translator, "turnstileVerifyFailed"))
    }
}

pub(super) fn pow_secret_number_from_random(value: u32) -> u32 {
    value % POW_MAX_NUMBER
}

pub(super) fn pow_number_text(value: Option<&Value>) -> String {
    let Some(Value::Number(number)) = value else {
        return String::new();
    };
    if let Some(value) = number.as_i64() {
        return value.to_string();
    }
    if let Some(value) = number.as_u64() {
        return value.to_string();
    }
    let Some(value) = number.as_f64() else {
        return String::new();
    };
    if !value.is_finite() {
        return String::new();
    }
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

pub(super) fn turnstile_error_reason(result: &Value) -> Option<String> {
    let reason = result
        .get("error-codes")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    (!reason.is_empty()).then_some(reason)
}
