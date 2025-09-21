use assert_cmd::Command;

#[test]
fn runs_trainer_with_args() {
    Command::cargo_bin("trainer").unwrap()
        .args(["--steps", "3", "--log", "debug"])
        .assert()
        .success();
}
