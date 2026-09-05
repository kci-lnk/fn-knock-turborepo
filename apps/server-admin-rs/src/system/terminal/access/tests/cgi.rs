//! Exercise the shipped NAS CGI scripts against the actual terminal HTTP API.
use super::*;
use std::path::Path;
use tokio::process::Command;

async fn request(
    script: &Path,
    bin: &Path,
    port: u16,
    path: &str,
    method: &str,
    cookie: &str,
) -> String {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    let body = if method == "POST" {
        r#"{"password":"cgi-test-secret"}"#
    } else {
        ""
    };
    let mut child = Command::new("sh")
        .arg(script)
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("AUTHENTICATE_CGI", bin.join("authenticate"))
        .env("ADMIN_TARGET_HOST", "127.0.0.1")
        .env("ADMIN_TARGET_PORT", port.to_string())
        .env("REQUEST_URI", path)
        .env("REQUEST_METHOD", method)
        .env("CONTENT_LENGTH", body.len().to_string())
        .env("HTTP_COOKIE", cookie)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .await
        .unwrap();
    let output = child.wait_with_output().await.unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}
fn json(output: &str) -> serde_json::Value {
    serde_json::from_str(output.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[tokio::test]
async fn nas_cgi_verification_cookie_unlocks_real_business_routes_and_survives_reentry() {
    use std::os::unix::fs::PermissionsExt;
    let (directory, state) = test_state().await;
    change(&state, true, Some("cgi-test-secret"), false).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = crate::terminal::terminal_routes().with_state(state.clone());
    let shutdown = state.shutdown.clone();
    state.spawn_background("terminal-cgi-test", async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
    });
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bin = directory.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    for (name, body) in [
        ("authenticate", "#!/bin/sh\nprintf test-admin"),
        ("id", "#!/bin/sh\nprintf administrators"),
    ] {
        let path = bin.join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    for (script, prefix) in [
        (
            "apps/fn-knock/app/ui/index.cgi",
            "/cgi/ThirdParty/fn-knock/index.cgi/",
        ),
        (
            "apps/fn-knock-lite/app/ui/index.cgi",
            "/cgi/ThirdParty/fn-knock-lite/index.cgi/",
        ),
        (
            "apps/fn-knock-synology/package/ui/index.cgi",
            "/webman/3rdparty/fn-knock-synology/index.cgi/",
        ),
    ] {
        let script = root.join(script);
        let access_path = format!("{prefix}api/admin/terminal/access");
        let targets_path = format!("{prefix}api/admin/terminal/targets");
        let locked = request(
            &script,
            &bin,
            port,
            &targets_path,
            "GET",
            "nas-session=private",
        )
        .await;
        assert_eq!(json(&locked)["errorCode"], "access_password_required");
        let verified = request(
            &script,
            &bin,
            port,
            &format!("{access_path}/verify"),
            "POST",
            "nas-session=private",
        )
        .await;
        let cookie = verified
            .lines()
            .find_map(|line| line.strip_prefix("Set-Cookie: fn-knock-terminal-access="))
            .unwrap();
        assert!(cookie.contains(&format!("Path={prefix}; HttpOnly; SameSite=Strict")));
        assert!(!cookie.contains("Max-Age"));
        let cookie = format!(
            "fn-knock-terminal-access={}; nas-session=private",
            cookie.split(';').next().unwrap()
        );
        for _ in 0..2 {
            let status = request(&script, &bin, port, &access_path, "GET", &cookie).await;
            assert_eq!(json(&status)["data"]["authorized"], true);
            let targets = request(&script, &bin, port, &targets_path, "GET", &cookie).await;
            assert!(json(&targets)["data"].is_array());
        }
        // A different browser session cannot inherit this authorization.
        let fresh = request(&script, &bin, port, &access_path, "GET", "").await;
        assert_eq!(json(&fresh)["data"]["authorized"], false);
    }
    state.shutdown.cancel();
}
