use super::*;

const CLIENT_IP: &str = "203.0.113.79";
const AUTH_HOST: &str = "auth.ss.com";
const ALLOWED_HOST: &str = "aa.ss.com";
const OTHER_HOST: &str = "bb.ss.com";
const TARGET_URL: &str = "https://aa.ss.com/dashboard?tab=files";
const PASSWORD: &str = "single-service-password123";

// These end-to-end handlers use the bounded production password hash pool.
// Keep this suite from overflowing its admission queue in parallel test runs.
static PASSWORD_SCOPE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct PasswordScopeFixture {
    _directory: tempfile::TempDir,
    state: AppState,
    app: Router,
    account_id: String,
}

impl PasswordScopeFixture {
    async fn cross_domain() -> Self {
        let fixture = Self::new("").await;
        let mut config = fixture.state.storage.store.get_config().await.unwrap();
        config["subdomain_mode"]["root_domain"] = json!("abc.com");
        config["subdomain_mode"]["auth_host"] = json!("auth.abc.com");
        config["subdomain_mode"]["public_auth_base_url"] = json!("https://auth.abc.com");
        fixture
            .state
            .storage
            .store
            .save_config(&config)
            .await
            .unwrap();
        assert!(
            fixture
                .state
                .storage
                .store
                .compare_and_set_host_mappings(
                    config
                        .get("host_mappings")
                        .and_then(Value::as_array)
                        .unwrap(),
                    &[
                        json!({"host": "ss.abc.com", "use_auth": true}),
                        json!({"host": "bb.def.com", "use_auth": true}),
                    ],
                )
                .await
                .unwrap()
                .is_some()
        );
        fixture
            .set_access(json!({"mode": "custom", "hosts": ["bb.def.com"]}))
            .await;
        fixture
    }

    async fn new(cookie_domain: &str) -> Self {
        let (directory, state) = auth_route_test_state("password-single-service").await;
        let mut config = state.storage.store.get_config().await.unwrap();
        config["subdomain_mode"]["cookie_domain"] = json!(cookie_domain);
        config["subdomain_mode"]["root_domain"] = json!("ss.com");
        config["subdomain_mode"]["auth_host"] = json!(AUTH_HOST);
        config["auth_credential_settings"] = json!({
            "post_login_ip_grant_mode": "follow_session"
        });
        state.storage.store.save_config(&config).await.unwrap();
        assert!(
            state
                .storage
                .store
                .compare_and_set_host_mappings(
                    config
                        .get("host_mappings")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                    &[
                        json!({"host": ALLOWED_HOST, "use_auth": true}),
                        json!({"host": OTHER_HOST, "use_auth": true}),
                    ],
                )
                .await
                .unwrap()
                .is_some()
        );
        state
            .storage
            .store
            .set_auth_login_mode(AuthLoginMode::Password)
            .await
            .unwrap();
        let account_routes: Router<AppState> = crate::admin::control::auth_account_routes().into();
        let app = Router::new()
            .merge(account_routes)
            .nest("/api/auth", auth_api_routes())
            .with_state(state.clone());
        let created = app
            .clone()
            .oneshot(
                Request::post("/api/admin/auth/accounts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"username": "single-service", "password": PASSWORD}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        let account_id = response_json(created).await["data"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(state.storage.store.get_totps().await.unwrap().is_empty());
        let fixture = Self {
            _directory: directory,
            state,
            app,
            account_id,
        };
        fixture
            .set_access(json!({"mode": "custom", "hosts": [ALLOWED_HOST]}))
            .await;
        fixture
    }

    async fn set_access(&self, access: Value) {
        // Seed the persisted permission state: the management PATCH also
        // refreshes the external Go control service, absent in this fixture.
        let mut account = self
            .state
            .storage
            .store
            .get_auth_account(&self.account_id)
            .await
            .unwrap()
            .unwrap();
        account.subdomain_access = crate::store::normalize_totp_subdomain_access(access);
        let restricted = account.subdomain_access["mode"] == "custom";
        self.state
            .storage
            .store
            .save_auth_account(account)
            .await
            .unwrap();
        if restricted {
            auth_mobility::clear_auto_ip_grants_for_auth_credential(&self.state, &self.account_id)
                .await
                .unwrap();
        }
    }

    async fn request(&self, endpoint: &str, host: &str, path: &str, cookie: &str) -> Response {
        self.request_with_method(Method::GET, endpoint, host, path, cookie)
            .await
    }

    async fn request_with_method(
        &self,
        method: Method,
        endpoint: &str,
        host: &str,
        path: &str,
        cookie: &str,
    ) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(endpoint)
                    .header("x-forwarded-host", host)
                    .header("x-forwarded-proto", "https")
                    .header("x-forwarded-path", path)
                    .header("x-forwarded-for", CLIENT_IP)
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn login(&self) -> (String, Value) {
        self.login_to(Some(TARGET_URL), AUTH_HOST).await
    }

    async fn login_to(&self, redirect: Option<&str>, host: &str) -> (String, Value) {
        self.login_as("single-service", redirect, host).await
    }

    async fn login_as(
        &self,
        username: &str,
        redirect: Option<&str>,
        host: &str,
    ) -> (String, Value) {
        // Exercise the public login handler, including a real, single-use PoW
        // challenge, rather than publishing a synthetic session fixture.
        let challenge = self
            .request("/api/auth/challenge", host, "/api/auth/challenge", "")
            .await;
        assert_eq!(challenge.status(), StatusCode::OK);
        let mut proof = response_json(challenge).await;
        let salt = proof["salt"].as_str().unwrap();
        let expected = proof["challenge"].as_str().unwrap();
        let number = (0..proof["maxnumber"].as_u64().unwrap())
            .find(|number| sha256_hex(format!("{salt}{number}").as_bytes()) == expected)
            .expect("solve login challenge");
        proof["number"] = json!(number);
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header("x-forwarded-host", host)
                    .header("x-forwarded-proto", "https")
                    .header("x-forwarded-for", CLIENT_IP)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "method": "password",
                            "username": username,
                            "password": PASSWORD,
                            "captcha": {
                                "provider": "pow",
                                "proof": BASE64_STANDARD.encode(proof.to_string())
                            },
                            "redirect_uri": redirect
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookies = response_set_cookies(&response);
        assert!(
            cookies[..cookies.len() - 1]
                .iter()
                .all(|cookie| cookie.contains("Max-Age=0"))
        );
        assert!(!cookies.last().unwrap().contains("Max-Age=0"));
        let payload = response_json(response).await;
        assert_eq!(payload["success"], true);
        (cookies.last().unwrap().clone(), payload["data"].clone())
    }
}

#[tokio::test]
async fn password_cross_domain_service_uses_host_only_session_and_keeps_permissions() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::cross_domain().await;
    let target = "https://bb.def.com/dashboard?tab=files";
    let anonymous = fixture
        .request("/api/auth/verify", "bb.def.com", "/dashboard?tab=files", "")
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let local = url::Url::parse(&format!(
        "https://bb.def.com{}",
        anonymous.headers()["X-Reauth-Redirect-Location"]
            .to_str()
            .unwrap()
    ))
    .unwrap();
    assert_eq!(local.path(), "/__auth__/login");
    assert_eq!(
        local
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1,
        target
    );
    let (set_cookie, login) = fixture.login_to(Some(target), "bb.def.com").await;
    assert!(!set_cookie.contains("Domain="));
    assert_eq!(login["grant_type"], "browser_session");
    assert_eq!(login["redirect_to"], target);
    let cookie = set_cookie.split(';').next().unwrap();
    assert_eq!(
        fixture
            .request("/api/auth/verify", "bb.def.com", "/dashboard", cookie)
            .await
            .status(),
        StatusCode::OK
    );
    // Even if a client manually sends that session to another host, the live
    // account ACL must deny it. A browser cannot share this host-only cookie.
    assert_eq!(
        fixture
            .request("/api/auth/verify", "ss.abc.com", "/dashboard", cookie)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fixture
            .request("/api/auth/verify", "bb.def.com", "/dashboard", "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let session_id = cookie.split_once('=').unwrap().1;
    assert!(
        fixture
            .state
            .storage
            .store
            .get_session(session_id)
            .await
            .unwrap()
            .unwrap()
            .post_login_ip_grant_record_id
            .is_none()
    );
}

#[tokio::test]
async fn password_cross_domain_shared_auth_moves_to_target_before_login() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::cross_domain().await;
    let target = "https://bb.def.com:8443/dashboard?tab=files";
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("redirect_uri", target)
        .finish();
    let endpoint = format!("/api/auth/bootstrap?{query}");
    let response = fixture
        .request(&endpoint, "auth.abc.com", "/api/auth/bootstrap", "")
        .await;
    let data = response_json(response).await;
    assert_eq!(data["data"]["auth"]["authenticated"], false);
    let redirect = url::Url::parse(data["data"]["redirect_to"].as_str().unwrap()).unwrap();
    assert_eq!(
        redirect.origin().ascii_serialization(),
        "https://bb.def.com:8443"
    );
    assert_eq!(redirect.path(), "/__auth__/login");
    assert_eq!(
        redirect
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1,
        target
    );
    let response = fixture
        .request(
            &endpoint,
            "bb.def.com:8443",
            "/__auth__/api/auth/bootstrap",
            "",
        )
        .await;
    assert!(
        response_json(response).await["data"]
            .get("redirect_to")
            .is_none()
    );
    let endpoint = "/api/auth/bootstrap?redirect_uri=https%3A%2F%2Funconfigured.def.com%2F";
    let response = fixture
        .request(endpoint, "auth.abc.com", "/api/auth/bootstrap", "")
        .await;
    assert!(
        response_json(response).await["data"]
            .get("redirect_to")
            .is_none()
    );
}

#[tokio::test]
async fn password_cross_domain_existing_shared_auth_session_does_not_stay_on_auth() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::cross_domain().await;
    let (set_cookie, login) = fixture.login_to(None, "auth.abc.com").await;
    assert!(set_cookie.contains("Domain=abc.com"));
    let local = url::Url::parse(login["redirect_to"].as_str().unwrap()).unwrap();
    assert_eq!(local.host_str(), Some("bb.def.com"));
    assert_eq!(local.path(), "/__auth__/login");
    let cookie = set_cookie.split(';').next().unwrap();
    let bootstrap = fixture
        .request(
            "/api/auth/bootstrap",
            "auth.abc.com",
            "/api/auth/bootstrap",
            cookie,
        )
        .await;
    let data = response_json(bootstrap).await;
    assert_eq!(data["data"]["auth"]["authenticated"], true);
    assert_eq!(data["data"]["redirect_to"], local.as_str());
}

#[tokio::test]
async fn password_single_service_login_shares_cookie_and_enforces_live_permissions() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    // The UI's default empty override must derive the shared cookie domain
    // from root_domain; explicit cookie-domain overrides are covered below.
    let fixture = PasswordScopeFixture::new("").await;
    let (set_cookie, login) = fixture.login().await;
    assert!(set_cookie.contains("Domain=ss.com"));
    assert!(set_cookie.contains("Path=/"));
    assert!(set_cookie.contains("HttpOnly"));
    assert_eq!(login["grant_type"], "browser_session");
    assert_eq!(login["redirect_to"], TARGET_URL);
    let cookie = set_cookie.split(';').next().unwrap();
    let session_id = cookie.split_once('=').unwrap().1;
    let session = fixture
        .state
        .storage
        .store
        .get_session(session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.credential_id, fixture.account_id);
    assert!(session.totp_id.is_empty());
    assert!(session.post_login_ip_grant_mode.is_none());
    assert!(session.post_login_ip_grant_record_id.is_none());

    let allowed = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", cookie)
        .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    assert_eq!(allowed.headers()[REAUTH_SUBDOMAIN_ACCESS_HEADER], "custom");
    assert_eq!(
        allowed.headers()[REAUTH_ALLOWED_SUBDOMAIN_HOSTS_HEADER],
        ALLOWED_HOST
    );
    let denied = fixture
        .request("/api/auth/verify", OTHER_HOST, "/dashboard", cookie)
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(denied.headers()[REAUTH_ACCESS_DENIED_HEADER], "scope");
    assert!(denied.headers().get(header::LOCATION).is_none());
    for (host, expected_denial) in [(ALLOWED_HOST, None), (OTHER_HOST, Some("scope"))] {
        let preflight = fixture
            .request_with_method(
                Method::HEAD,
                "/api/auth/preflight",
                host,
                "/dashboard",
                cookie,
            )
            .await;
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            preflight
                .headers()
                .get(REAUTH_ACCESS_DENIED_HEADER)
                .map(|value| value.to_str().unwrap()),
            expected_denial
        );
        assert!(preflight.headers().get(header::LOCATION).is_none());
    }
    let select = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/__select__", cookie)
        .await;
    assert_eq!(select.status(), StatusCode::FORBIDDEN);
    let anonymous = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", "")
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("redirect_uri", TARGET_URL)
        .finish();
    let bootstrap = fixture
        .request(
            &format!("/api/auth/bootstrap?{query}"),
            AUTH_HOST,
            "/api/auth/bootstrap",
            cookie,
        )
        .await;
    let bootstrap = response_json(bootstrap).await;
    assert_eq!(bootstrap["data"]["auth"]["authenticated"], true);
    assert_eq!(bootstrap["data"]["redirect_to"], TARGET_URL);

    fixture
        .set_access(json!({"mode": "custom", "hosts": [OTHER_HOST]}))
        .await;
    let revoked = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", cookie)
        .await;
    assert_eq!(revoked.status(), StatusCode::FORBIDDEN);
    let newly_allowed = fixture
        .request("/api/auth/verify", OTHER_HOST, "/dashboard", cookie)
        .await;
    assert_eq!(newly_allowed.status(), StatusCode::OK);
}

#[tokio::test]
async fn password_single_service_auth_only_cookie_routes_to_service_local_login() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    // Same parent domain is insufficient when an explicit override narrows
    // the cookie to auth. The service request below intentionally has no
    // cookie, matching what a browser sends to aa.ss.com in this case.
    let fixture = PasswordScopeFixture::new(AUTH_HOST).await;
    let (set_cookie, login) = fixture.login().await;
    assert!(set_cookie.contains("Domain=auth.ss.com"));
    assert_eq!(login["grant_type"], "browser_session");
    let local_login = login["redirect_to"].as_str().unwrap();
    assert!(local_login.starts_with("https://aa.ss.com/__auth__/login?"));
    let cookie = set_cookie.split(';').next().unwrap();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("redirect_uri", TARGET_URL)
        .finish();
    let bootstrap = fixture
        .request(
            &format!("/api/auth/bootstrap?{query}"),
            AUTH_HOST,
            "/api/auth/bootstrap",
            cookie,
        )
        .await;
    let bootstrap = response_json(bootstrap).await;
    assert_eq!(bootstrap["data"]["auth"]["authenticated"], true);
    assert_eq!(bootstrap["data"]["redirect_to"], local_login);
    let service = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", "")
        .await;
    assert_eq!(service.status(), StatusCode::UNAUTHORIZED);

    // Correct the cookie configuration while retaining the one-service scope.
    // No unrestricted permission or IP grant is needed to recover.
    let mut config = fixture.state.storage.store.get_config().await.unwrap();
    config["subdomain_mode"]["cookie_domain"] = json!("ss.com");
    fixture
        .state
        .storage
        .store
        .save_config(&config)
        .await
        .unwrap();
    let (shared_cookie, corrected_login) = fixture.login().await;
    assert!(shared_cookie.contains("Domain=ss.com"));
    assert_eq!(corrected_login["grant_type"], "browser_session");
    assert_eq!(corrected_login["redirect_to"], TARGET_URL);
    let cookie = shared_cookie.split(';').next().unwrap();
    let allowed = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", cookie)
        .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    let denied = fixture
        .request("/api/auth/verify", OTHER_HOST, "/dashboard", cookie)
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let anonymous = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", "")
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    // In an isolated state with the original narrow cookie domain, reproduce
    // why granting everything can hide the error: a fresh unrestricted login
    // publishes an IP grant, so the service works without a browser cookie.
    let unrestricted = PasswordScopeFixture::new(AUTH_HOST).await;
    unrestricted
        .set_access(json!({"mode": "all", "hosts": []}))
        .await;
    let (narrow_cookie, unrestricted_login) = unrestricted.login().await;
    assert!(narrow_cookie.contains("Domain=auth.ss.com"));
    assert_eq!(unrestricted_login["grant_type"], "login_ip_grant");
    assert_eq!(unrestricted_login["redirect_to"], TARGET_URL);
    let service = unrestricted
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", "")
        .await;
    assert_eq!(service.status(), StatusCode::OK);
}

#[tokio::test]
async fn password_single_service_stale_duplicate_cookie_does_not_broaden_access() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new("ss.com").await;
    let (set_cookie, _) = fixture.login().await;
    let cookie = set_cookie.split(';').next().unwrap();
    let stale_cookie = format!("{}=stale-session", cookies::SESSION_COOKIE_NAME);
    let fresh_last = format!("{stale_cookie}; {cookie}");
    let allowed = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", &fresh_last)
        .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    let denied = fixture
        .request("/api/auth/verify", OTHER_HOST, "/dashboard", &fresh_last)
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    // An invalid legacy cookie must no longer hide the only live session.
    let stale_last = format!("{cookie}; {stale_cookie}");
    let hidden = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", &stale_last)
        .await;
    assert_eq!(hidden.status(), StatusCode::OK);
    assert!(response_set_cookies(&hidden).is_empty());
    let denied = fixture
        .request("/api/auth/verify", OTHER_HOST, "/dashboard", &stale_last)
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let valid = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", cookie)
        .await;
    assert_eq!(valid.status(), StatusCode::OK);
}

#[tokio::test]
async fn password_single_service_logout_revokes_the_recovered_browser_session() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new("ss.com").await;
    let (fresh, _) = fixture.login().await;
    let fresh = fresh.split(';').next().unwrap();
    let session_id = fresh.split_once('=').unwrap().1;
    let combined = format!("{fresh}; {}=stale", cookies::SESSION_COOKIE_NAME);
    let logout = fixture
        .request(
            "/api/auth/logout",
            ALLOWED_HOST,
            "/__auth__/api/auth/logout",
            &combined,
        )
        .await;
    // This fixture has no Go control service to confirm gateway revocation;
    // server-side revocation must still destroy the recovered live session.
    assert!(matches!(
        logout.status(),
        StatusCode::FOUND | StatusCode::SERVICE_UNAVAILABLE
    ));
    assert!(
        fixture
            .state
            .storage
            .store
            .get_session(session_id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture
            .request("/api/auth/verify", ALLOWED_HOST, "/", fresh)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn password_single_service_direct_auth_login_redirects_to_aa_ss_com() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new("").await;
    let (set_cookie, login) = fixture.login_to(None, AUTH_HOST).await;
    assert_eq!(login["redirect_to"], "https://aa.ss.com/");
    let cookie = set_cookie.split(';').next().unwrap();
    let bootstrap = fixture
        .request(
            "/api/auth/bootstrap",
            AUTH_HOST,
            "/api/auth/bootstrap",
            cookie,
        )
        .await;
    assert_eq!(
        response_json(bootstrap).await["data"]["redirect_to"],
        "https://aa.ss.com/"
    );
    let service = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/", cookie)
        .await;
    assert_eq!(service.status(), StatusCode::OK);
    let denied = fixture
        .request("/api/auth/verify", OTHER_HOST, "/", cookie)
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn password_single_service_incompatible_cookie_uses_service_local_login() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new(AUTH_HOST).await;
    let anonymous = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard?tab=files", "")
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let location = anonymous.headers()["X-Reauth-Redirect-Location"]
        .to_str()
        .unwrap();
    let parsed = url::Url::parse(&format!("https://{ALLOWED_HOST}{location}")).unwrap();
    assert_eq!(parsed.path(), "/__auth__/login");
    assert_eq!(
        parsed
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1,
        TARGET_URL
    );
    let bootstrap = fixture
        .request(
            &format!("/api/auth/bootstrap?{}", parsed.query().unwrap()),
            ALLOWED_HOST,
            "/__auth__/login",
            "",
        )
        .await;
    assert!(
        response_json(bootstrap).await["data"]
            .get("redirect_to")
            .is_none()
    );
    let (set_cookie, login) = fixture.login_to(Some(TARGET_URL), ALLOWED_HOST).await;
    assert!(set_cookie.contains("Domain=ss.com"));
    assert_eq!(login["redirect_to"], TARGET_URL);
    let cookie = set_cookie.split(';').next().unwrap();
    assert_eq!(
        fixture
            .request("/api/auth/verify", ALLOWED_HOST, "/dashboard", cookie)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .request("/api/auth/verify", OTHER_HOST, "/dashboard", cookie)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn password_single_service_local_login_keeps_public_https_port() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new(AUTH_HOST).await;
    let mut config = fixture.state.storage.store.get_config().await.unwrap();
    config["subdomain_mode"]["public_https_port"] = json!(8443);
    fixture
        .state
        .storage
        .store
        .save_config(&config)
        .await
        .unwrap();
    let response = fixture
        .request("/api/auth/verify", ALLOWED_HOST, "/dashboard?tab=files", "")
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let location = response.headers()["X-Reauth-Redirect-Location"]
        .to_str()
        .unwrap();
    let parsed = url::Url::parse(&format!("https://{ALLOWED_HOST}:8443{location}")).unwrap();
    assert_eq!(
        parsed
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1,
        "https://aa.ss.com:8443/dashboard?tab=files"
    );
    let (_, direct_login) = fixture.login_to(None, ALLOWED_HOST).await;
    assert_eq!(direct_login["redirect_to"], "https://aa.ss.com:8443/");
}

#[tokio::test]
async fn password_multiple_services_do_not_choose_an_arbitrary_login_target() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new("ss.com").await;
    fixture
        .set_access(json!({"mode": "custom", "hosts": [ALLOWED_HOST, OTHER_HOST]}))
        .await;
    let (set_cookie, login) = fixture.login_to(None, AUTH_HOST).await;
    assert!(login.get("redirect_to").is_none());
    let cookie = set_cookie.split(';').next().unwrap();
    for host in [ALLOWED_HOST, OTHER_HOST] {
        assert_eq!(
            fixture
                .request("/api/auth/verify", host, "/", cookie)
                .await
                .status(),
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn password_single_service_expired_duplicate_cookie_recovers_live_session() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new("ss.com").await;
    let (fresh, _) = fixture.login().await;
    let (expired, _) = fixture.login().await;
    let fresh = fresh.split(';').next().unwrap();
    let expired = expired.split(';').next().unwrap();
    let expired_id = expired.split_once('=').unwrap().1;
    fixture
        .state
        .storage
        .store
        .update_session_value(
            expired_id,
            json!({"expires_at": "2000-01-01T00:00:00Z"})
                .as_object()
                .unwrap()
                .clone(),
        )
        .await
        .unwrap();
    let combined = format!("{fresh}; {expired}");
    assert_eq!(
        fixture
            .request("/api/auth/verify", ALLOWED_HOST, "/", &combined)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .request("/api/auth/verify", OTHER_HOST, "/", &combined)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn password_single_service_ambiguous_live_cookies_do_not_choose_by_permission() {
    let _serial = PASSWORD_SCOPE_TEST_LOCK.lock().await;
    let fixture = PasswordScopeFixture::new("ss.com").await;
    let created = fixture
        .app
        .clone()
        .oneshot(
            Request::post("/api/admin/auth/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"username": "other-service", "password": PASSWORD}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let account_id = response_json(created).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut account = fixture
        .state
        .storage
        .store
        .get_auth_account(&account_id)
        .await
        .unwrap()
        .unwrap();
    account.subdomain_access = crate::store::normalize_totp_subdomain_access(
        json!({"mode": "custom", "hosts": [OTHER_HOST]}),
    );
    fixture
        .state
        .storage
        .store
        .save_auth_account(account)
        .await
        .unwrap();
    let (first, _) = fixture.login().await;
    let (second, _) = fixture.login_as("other-service", None, AUTH_HOST).await;
    let first = first.split(';').next().unwrap();
    let second = second.split(';').next().unwrap();
    let ambiguous = format!("{first}; {second}; {}=stale", cookies::SESSION_COOKIE_NAME);
    assert_eq!(
        fixture
            .request("/api/auth/verify", ALLOWED_HOST, "/", &ambiguous)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let last_live = format!("{first}; {second}");
    assert_eq!(
        fixture
            .request("/api/auth/verify", ALLOWED_HOST, "/", &last_live)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fixture
            .request("/api/auth/verify", OTHER_HOST, "/", &last_live)
            .await
            .status(),
        StatusCode::OK
    );
    let first_last = format!("{second}; {first}");
    assert_eq!(
        fixture
            .request("/api/auth/verify", ALLOWED_HOST, "/", &first_last)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .request("/api/auth/verify", OTHER_HOST, "/", &first_last)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}
