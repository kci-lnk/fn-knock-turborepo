//! Harden only the four known bundled hooks. The add operations and provider
//! signing code stay in acme.sh. The remove operations delegate selection of
//! an ID to our JSON parser before sending the provider's delete request.
use super::*;
use std::io::Read;

fn revisions(provider: &str) -> Option<(&'static str, &'static str)> {
    Some(match provider {
        "dns_cf" => (
            "9628ee8238cb3f9cfa1b1a985c0e9593436a3e4f8a9d65a6f775b981be9e76c8",
            "8b4ee02359b3323d3f90c23f8a1cd1dc33e0c39827117c3a28faf3f0aed3429f",
        ),
        "dns_ali" => (
            "6cfcca209d54747dc7b48fea9aa4a2c04a6017c4464f8f23a077e3eaa187c427",
            "ec842607020fcea72eff7dd09cfe015791b482f57f3df19aa8a957447500dfaa",
        ),
        "dns_dp" => (
            "a4b5b70c81235a033c6892359b09000097b88a0792b7029d14653a5494d282de",
            "203713ad983fc8ceefd27e1173708d686213aa782391e5d95d76faa78b06a6e4",
        ),
        "dns_tencent" => (
            "bedbc48380d54e28d929c271495533beb77e254a50fefc258565246641163159",
            "3f16996debcdcffabd70efcb61fe58fb72f311efc89cb50419cd3c858d403d5a",
        ),
        _ => return None,
    })
}
pub(super) async fn audited_hook(state: &AppState, provider: &str) -> bool {
    let Some((original, patched)) = revisions(provider) else {
        return false;
    };
    if crate::runtime_profile::deployment_target(state) == "windows" {
        return false;
    }
    let dir = acme_home_dir(state);
    let (Ok(client), Ok(hook)) = tokio::join!(
        tokio::fs::read(acme_executable_path(state)),
        tokio::fs::read(dir.join("dnsapi").join(format!("{provider}.sh")))
    ) else {
        return false;
    };
    let hash = hex::encode(Sha256::digest(hook));
    (hash == original || hash == patched)
        && hex::encode(Sha256::digest(client))
            == "c7d68b021cfd6380ea83a82962abde5b484779fee0b97d38681dfa1396bbc8d7"
}

fn patch(source: &str, provider: &str) -> String {
    let mut script = String::new();
    for line in source.lines() {
        let body = line.trim_start();
        if body.starts_with("record_id=") && body.contains("\"$response\"") {
            script.push_str(&line[..line.len() - body.len()]);
            script.push_str(&format!(r#"record_id=$(printf '%s' "$response" | "$FN_KNOCK_ACME_PARSER" acme-txt-record-id {provider} "$_domain" "$fulldomain" "$txtvalue") || return 1"#));
        } else {
            if provider == "dns_tencent" && body == "attempt=1" {
                script.push_str("  record_id=\"\"\n");
            }
            script.push_str(line);
            if provider == "dns_tencent"
                && body.contains("_tencent_rest \"DescribeRecordFilterList\"")
            {
                script.push_str(" || return 1");
            }
        }
        script.push('\n');
    }
    script
}

pub(super) async fn prepare(state: &AppState, provider: &str) -> anyhow::Result<bool> {
    let Some((original, patched)) = revisions(provider) else {
        return Ok(false);
    };
    let path = acme_home_dir(state)
        .join("dnsapi")
        .join(format!("{provider}.sh"));
    let provider = provider.to_owned();
    tokio::task::spawn_blocking(move || {
        let source = match std::fs::read(&path) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        let hash = hex::encode(Sha256::digest(&source));
        if hash == patched {
            return Ok(true);
        }
        if hash != original {
            return Ok(false);
        }
        let updated = patch(std::str::from_utf8(&source)?, &provider);
        anyhow::ensure!(
            hex::encode(Sha256::digest(updated.as_bytes())) == patched,
            "ACME hook patch verification failed"
        );
        let mut file = tempfile::NamedTempFile::new_in(
            path.parent()
                .ok_or_else(|| anyhow::anyhow!("missing hook directory"))?,
        )?;
        file.write_all(updated.as_bytes())?;
        file.as_file()
            .set_permissions(std::fs::metadata(&path)?.permissions())?;
        file.persist(path)?;
        Ok(true)
    })
    .await?
}

/// Internal acme.sh command. Its only output is one verified ID (or empty).
/// It has no DNS credentials or network operations in its interface.
pub fn cleanup_id_command(args: &[String]) -> anyhow::Result<()> {
    let [provider, zone, name, value] = args else {
        anyhow::bail!("invalid ACME record lookup")
    };
    const LIMIT: u64 = 4 * 1024 * 1024;
    let mut bytes = Vec::new();
    std::io::stdin().take(LIMIT + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!((bytes.len() as u64) <= LIMIT, "DNS response too large");
    let body = serde_json::from_slice(&bytes)?;
    if let Some(id) = crate::ddns::acme_txt::native_cleanup_id(provider, zone, name, value, &body)?
    {
        println!("{id}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_bundled_remover_is_patched_and_digest_verified() {
        let mut zip = ZipArchive::new(Cursor::new(
            include_bytes!("../../../resources/acmesh.zip").as_slice(),
        ))
        .unwrap();
        for provider in ["dns_cf", "dns_dp", "dns_ali", "dns_tencent"] {
            let mut original = String::new();
            zip.by_name(&format!("acmesh/dnsapi/{provider}.sh"))
                .unwrap()
                .read_to_string(&mut original)
                .unwrap();
            let hashes = revisions(provider).unwrap();
            assert_eq!(hex::encode(Sha256::digest(&original)), hashes.0);
            let script = patch(&original, provider);
            assert_eq!(hex::encode(Sha256::digest(&script)), hashes.1);
            assert!(
                !script
                    .lines()
                    .any(|line| line.trim_start().starts_with("record_id=")
                        && line.contains("grep "))
            );
        }
    }
}
