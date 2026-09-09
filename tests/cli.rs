use assert_cmd::Command;
use std::fs;

#[test]
fn cli_build_with_relative_path_no_output() {
    let temp = std::env::temp_dir().join("rsml_test_cli_build");

    // Clean up from any previous run.
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp.join("src")).unwrap();

    fs::write(temp.join("src/test.rsml"), "").unwrap();

    // Replicates: `rsml-cli build src` (relative path, no --output).
    Command::cargo_bin("rsml-cli")
        .unwrap()
        .current_dir(&temp)
        .args(["build", "src"])
        .assert()
        .success();

    // With no --output, the model.json is written into the input dir.
    let model_json_path = temp.join("src/test.model.json");
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

#[test]
fn cli_build_imports_macros_from_transitive_derives() {
    let temp = std::env::temp_dir().join("rsml_test_cli_derived_macros");
    let input = temp.join("src");
    let theme = input.join("theme");
    let output = temp.join("out");

    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&theme).unwrap();

    fs::write(
        theme.join("macros.rsml"),
        r#"
@macro Fade -> Construct {
    BackgroundTransparency = 0.5;
}
"#,
    )
    .unwrap();
    fs::write(theme.join("base.rsml"), "@derive \"./macros\";\n").unwrap();
    fs::write(
        input.join("app.rsml"),
        r#"
@derive "./theme/base";

Frame {
    Fade!();
}
"#,
    )
    .unwrap();

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

    let content: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("app.model.json")).unwrap()).unwrap();

    assert!(
        content["children"][0]["properties"]["PropertiesSerialize"]["Attributes"]
            .get("BackgroundTransparency")
            .is_some(),
        "expected the macro from the transitive derive to be expanded: {content:#}"
    );

    let _ = fs::remove_dir_all(&temp);
}
