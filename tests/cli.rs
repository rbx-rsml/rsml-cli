use assert_cmd::Command;
use std::fs;

#[test]
fn cli_build_command() {
    let temp = std::env::temp_dir().join("rsml_test_cli_build");
    let input = temp.join("src");
    let output = temp.join("out");

    // Clean up from any previous run.
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&input).unwrap();
    fs::create_dir_all(&output).unwrap();

    fs::write(input.join("test.rsml"), "").unwrap();

    Command::cargo_bin("rsml-cli")
        .unwrap()
        .args([
            "build",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let model_json_path = output.join("test.model.json");
    assert!(
        model_json_path.exists(),
        "Expected {:?} to exist",
        model_json_path
    );

    let content: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&model_json_path).unwrap()).unwrap();
    assert_eq!(content["className"], "StyleSheet");

    // Clean up.
    let _ = fs::remove_dir_all(&temp);
}
