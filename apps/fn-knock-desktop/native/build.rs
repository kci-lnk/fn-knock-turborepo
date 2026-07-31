use std::{fs, path::Path};

fn validate_windows_bundle_identity() {
    if !std::env::var("TARGET").is_ok_and(|target| target.contains("windows")) {
        return;
    }
    let path = Path::new("../bundle/windows/runtime/bundle.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "Windows runtime bundle identity is missing at {}: {error}; run npm run fn-knock:windows:prepare from the repository root",
            path.display()
        )
    });
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("invalid Windows runtime bundle identity: {error}"));
    for field in ["commit", "gateway_commit"] {
        let value = document[field].as_str().unwrap_or_default();
        assert!(
            value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Windows runtime bundle identity has an invalid or missing {field}; run npm run fn-knock:windows:prepare from the repository root"
        );
    }
    assert_eq!(
        document["control_api_version"].as_u64(),
        Some(5),
        "Windows runtime bundle identity has an invalid or missing control_api_version; run npm run fn-knock:windows:prepare from the repository root"
    );
}

fn main() {
    validate_windows_bundle_identity();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let execution_level = if std::env::var("PROFILE").as_deref() == Ok("release") {
            "requireAdministrator"
        } else {
            "asInvoker"
        };
        let manifest = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*" />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security><requestedPrivileges>
      <requestedExecutionLevel level="{execution_level}" uiAccess="false" />
    </requestedPrivileges></security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}" />
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#
        );
        let mut resources = winres::WindowsResource::new();
        resources.set_manifest(&manifest);
        resources.set_icon("assets/icon.ico");
        resources
            .set("ProductName", "Knock 敲门")
            .set("CompanyName", "KCI-LNK Corporation")
            .set("LegalCopyright", "Copyright © KCI-LNK Corporation")
            .set("FileDescription", "Knock 敲门 Windows 管理程序")
            .set("OriginalFilename", "fn-knock.exe");
        resources
            .compile()
            .expect("failed to compile Windows resources");
    }
}
