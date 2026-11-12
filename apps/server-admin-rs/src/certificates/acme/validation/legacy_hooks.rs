//! Restore the bundled hooks modified in place by the 2.5.8 ACME client.
use super::*;

pub(super) async fn restore_if_patched(home: PathBuf, provider: &str) -> anyhow::Result<()> {
    let (patched, original) = match provider {
        "dns_cf" => (
            "8b4ee02359b3323d3f90c23f8a1cd1dc33e0c39827117c3a28faf3f0aed3429f",
            "9628ee8238cb3f9cfa1b1a985c0e9593436a3e4f8a9d65a6f775b981be9e76c8",
        ),
        "dns_ali" => (
            "ec842607020fcea72eff7dd09cfe015791b482f57f3df19aa8a957447500dfaa",
            "6cfcca209d54747dc7b48fea9aa4a2c04a6017c4464f8f23a077e3eaa187c427",
        ),
        "dns_dp" => (
            "203713ad983fc8ceefd27e1173708d686213aa782391e5d95d76faa78b06a6e4",
            "a4b5b70c81235a033c6892359b09000097b88a0792b7029d14653a5494d282de",
        ),
        "dns_tencent" => (
            "3f16996debcdcffabd70efcb61fe58fb72f311efc89cb50419cd3c858d403d5a",
            "bedbc48380d54e28d929c271495533beb77e254a50fefc258565246641163159",
        ),
        _ => return Ok(()),
    };
    let entry = format!("acmesh/dnsapi/{provider}.sh");
    let directory = home.join("dnsapi");
    let path = directory.join(format!("{provider}.sh"));
    tokio::task::spawn_blocking(move || {
        let source = match std::fs::read(&path) {
            Ok(source) => source,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if hex::encode(Sha256::digest(&source)) != patched {
            return Ok(());
        }
        let bundle = resolve_bundled_acme_zip_path()
            .ok_or_else(|| anyhow::anyhow!("missing bundled acme.sh for restoring {entry}"))?;
        let mut archive = ZipArchive::new(File::open(bundle)?)?;
        let mut restored = tempfile::NamedTempFile::new_in(directory)?;
        std::io::copy(&mut archive.by_name(&entry)?, &mut restored)?;
        anyhow::ensure!(
            hex::encode(Sha256::digest(std::fs::read(restored.path())?)) == original,
            "unexpected bundled ACME hook revision for {entry}"
        );
        restored
            .as_file()
            .set_permissions(std::fs::metadata(&path)?.permissions())?;
        restored.persist(path)?;
        Ok(())
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn patch_as_in_2_5_8(source: &str, provider: &str) -> String {
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

    #[tokio::test]
    async fn restores_only_hooks_patched_by_2_5_8() {
        let home = tempfile::tempdir().unwrap();
        let dnsapi = home.path().join("dnsapi");
        std::fs::create_dir(&dnsapi).unwrap();
        let mut archive =
            ZipArchive::new(File::open(resolve_bundled_acme_zip_path().unwrap()).unwrap()).unwrap();
        for provider in ["dns_cf", "dns_ali", "dns_dp", "dns_tencent"] {
            let path = dnsapi.join(format!("{provider}.sh"));
            let mut original = String::new();
            archive
                .by_name(&format!("acmesh/dnsapi/{provider}.sh"))
                .unwrap()
                .read_to_string(&mut original)
                .unwrap();
            let patched = patch_as_in_2_5_8(&original, provider);
            let custom = format!("{patched}# custom hook");
            for (installed, expected) in [
                (&patched, &original),
                (&original, &original),
                (&custom, &custom),
            ] {
                std::fs::write(&path, installed).unwrap();
                restore_if_patched(home.path().into(), provider)
                    .await
                    .unwrap();
                assert_eq!(std::fs::read_to_string(&path).unwrap(), *expected);
            }
        }
    }
}
