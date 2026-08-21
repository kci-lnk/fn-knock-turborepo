use super::*;

impl Store {
    pub(super) async fn verify_auth_session_shadow(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<()> {
        let matched = self
            .typed
            .typed_mobility
            .verify_and_repair_session_authority(session_id)
            .await?;
        self.observe_typed_mobility_shadow_comparison(matched);
        Ok(())
    }

    pub(super) fn observe_typed_mobility_shadow_comparison(&self, matched: bool) {
        if matched {
            if self.typed_mobility_shadow.mark_healthy() {
                tracing::info!("typed mobility aggregate comparison recovered");
            }
            return;
        }
        self.typed_mobility_shadow.mark_mismatch();
        tracing::warn!(
            "typed mobility shadow differed from the compatibility aggregate and was repaired"
        );
    }

    pub(super) async fn verify_passkey_runtime_shadow_key(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        let matched = self
            .typed
            .typed_passkey_runtime
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if self.typed_passkey_runtime_shadow.mark_healthy() {
                tracing::info!("typed passkey runtime shadow comparison recovered");
            }
            return Ok(());
        }
        self.typed_passkey_runtime_shadow.mark_mismatch();
        tracing::warn!(
            "typed passkey runtime shadow differed from the compatibility capability and was repaired"
        );
        Ok(())
    }

    pub async fn get_auth_login_mode(
        &self,
    ) -> crate::storage::StorageResult<crate::auth::mode::AuthLoginMode> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get("fn_knock:auth:login_mode:v1").await?;
        Ok(normalize_auth_login_mode(raw.as_deref()))
    }

    pub async fn set_auth_login_mode(
        &self,
        mode: crate::auth::mode::AuthLoginMode,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set("fn_knock:auth:login_mode:v1", mode.as_str()).await
    }

    pub async fn get_auth_accounts(&self) -> crate::storage::StorageResult<Vec<AuthAccount>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get("fn_knock:auth:accounts:v1").await?;
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        let value = serde_json::from_str::<Value>(&raw)?;
        Ok(normalize_auth_accounts_value(&value))
    }

    pub async fn set_auth_accounts(
        &self,
        accounts: &[AuthAccount],
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let normalized = normalize_auth_accounts(accounts);
        conn.set(
            "fn_knock:auth:accounts:v1",
            serde_json::to_string(&normalized)?,
        )
        .await
    }

    pub async fn get_auth_account(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<AuthAccount>> {
        Ok(self
            .get_auth_accounts()
            .await?
            .into_iter()
            .find(|account| account.id == id))
    }

    pub async fn get_auth_account_by_username(
        &self,
        normalized_username: &str,
    ) -> crate::storage::StorageResult<Option<AuthAccount>> {
        Ok(self.get_auth_accounts().await?.into_iter().find(|account| {
            normalize_auth_username(&account.username)
                == normalize_auth_username(normalized_username)
        }))
    }

    pub async fn save_auth_account(
        &self,
        account: AuthAccount,
    ) -> crate::storage::StorageResult<AuthAccount> {
        let mut accounts = self.get_auth_accounts().await?;
        let normalized = normalize_auth_account(account);
        let mut found = false;
        for existing in &mut accounts {
            if existing.id == normalized.id {
                *existing = normalized.clone();
                found = true;
                break;
            }
        }
        if !found {
            accounts.push(normalized.clone());
        }
        self.set_auth_accounts(&accounts).await?;
        Ok(normalized)
    }

    pub async fn get_auth_password_credential(
        &self,
        account_id: &str,
    ) -> crate::storage::StorageResult<Option<AuthPasswordCredential>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(auth_password_credential_key(account_id)).await?;
        raw.map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn set_auth_password_credential(
        &self,
        credential: &AuthPasswordCredential,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set(
            auth_password_credential_key(&credential.account_id),
            serde_json::to_string(credential)?,
        )
        .await
    }

    pub async fn delete_auth_password_credential(
        &self,
        account_id: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.del(auth_password_credential_key(account_id)).await
    }

    pub async fn get_totps(&self) -> crate::storage::StorageResult<Vec<TotpCredential>> {
        let raw: Option<String> = {
            let mut conn = self.conn();
            conn.get("fn_knock:totps").await?
        };
        let Some(raw) = raw else {
            let old_secret: Option<String> = {
                let mut conn = self.conn();
                conn.get("fn_knock:totp_secret").await?
            };
            let Some(old_secret) = old_secret.filter(|value| !value.is_empty()) else {
                return Ok(Vec::new());
            };
            let legacy = TotpCredential {
                id: "legacy-totp-id".to_string(),
                secret: old_secret,
                comment: "默认凭据".to_string(),
                created_at: now_iso(),
                access_scopes: Value::Array(Vec::new()),
                subdomain_access: normalize_totp_subdomain_access(Value::Null),
            };
            self.set_totps(std::slice::from_ref(&legacy)).await?;
            {
                let mut conn = self.conn();
                let _: () = conn.del("fn_knock:totp_secret").await?;
            }
            let mut passkeys = self.get_passkeys().await?;
            let mut passkeys_modified = false;
            for passkey in &mut passkeys {
                let missing_totp_id = passkey
                    .get("totpId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none();
                if missing_totp_id && let Some(object) = passkey.as_object_mut() {
                    object.insert("totpId".to_string(), Value::String(legacy.id.clone()));
                    passkeys_modified = true;
                }
            }
            if passkeys_modified {
                self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
                    .await?;
            }
            let normalized = normalize_totp_credentials(std::slice::from_ref(&legacy));
            return Ok(normalized);
        };
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        let value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
        Ok(normalize_totp_credentials_value(&value))
    }

    pub async fn set_totps(&self, totps: &[TotpCredential]) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let normalized = normalize_totp_credentials(totps);
        conn.set("fn_knock:totps", serde_json::to_string(&normalized)?)
            .await
    }

    pub async fn add_totp(&self, credential: TotpCredential) -> crate::storage::StorageResult<()> {
        let mut totps = self.get_totps().await?;
        if let Some(credential) =
            normalize_totp_credential_value(&serde_json::to_value(credential)?)
        {
            totps.push(credential);
        }
        self.set_totps(&totps).await
    }

    pub async fn update_totp_comment(
        &self,
        id: &str,
        comment: String,
    ) -> crate::storage::StorageResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.comment = comment.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn update_totp_access_scopes(
        &self,
        id: &str,
        access_scopes: Value,
    ) -> crate::storage::StorageResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let normalized = normalize_totp_access_scopes(access_scopes);
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.access_scopes = normalized.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn update_totp_subdomain_access(
        &self,
        id: &str,
        subdomain_access: Value,
    ) -> crate::storage::StorageResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let normalized = normalize_totp_subdomain_access(subdomain_access);
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.subdomain_access = normalized.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn delete_totp(&self, id: &str) -> crate::storage::StorageResult<bool> {
        let mut totps = self.get_totps().await?;
        let original_len = totps.len();
        totps.retain(|credential| credential.id != id);
        if totps.len() == original_len {
            return Ok(false);
        }
        self.set_totps(&totps).await?;
        let mut passkeys = self.get_passkeys().await?;
        let passkeys_original_len = passkeys.len();
        passkeys.retain(|passkey| passkey.get("totpId").and_then(Value::as_str) != Some(id));
        if passkeys.len() != passkeys_original_len {
            self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
                .await?;
        }
        Ok(true)
    }
}
