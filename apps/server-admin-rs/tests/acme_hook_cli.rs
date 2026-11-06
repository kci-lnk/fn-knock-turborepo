#![cfg(unix)]
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn installed_server_binary_selects_only_an_exact_txt_without_starting_a_server() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_server-admin-rs"))
        .args([
            "acme-txt-record-id",
            "dns_cf",
            "example.test",
            "_acme-challenge.example.test",
            "Proof",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(br#"{"success":true,"result_info":{"total_count":3},"result":[
        {"id":"first","type":"TXT","name":"_acme-challenge.example.test","content":"proof"},
        {"id":"second","type":"TXT","name":"_acme-challenge.other.example.test","content":"Proof"},
        {"id":"correct","type":"TXT","name":"_acme-challenge.example.test","content":"\"Proof\""}]}"#).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"correct\n");
}
