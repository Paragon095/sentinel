use assert_cmd::Command;

#[test]
fn runs_scanner_baseline() {
    Command::cargo_bin("scanner").unwrap()
        .arg("baseline")
        .assert()
        .success();
}
