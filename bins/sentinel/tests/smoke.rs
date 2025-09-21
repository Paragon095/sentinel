use assert_cmd::Command;

#[test]
fn runs_sentinel() {
    Command::cargo_bin("sentinel").unwrap()
        .assert()
        .success();
}
