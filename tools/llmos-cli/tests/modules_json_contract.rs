use std::process::Command;

#[test]
fn modules_json_output_matches_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_llmos-cli"))
        .arg("modules")
        .arg("--output")
        .arg("json")
        .arg("--timeout-secs")
        .arg("1")
        .output()
        .expect("failed to run llmos-cli binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "llmos modules command failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );

    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("modules --output json should emit valid JSON");
    let modules = value
        .as_array()
        .expect("modules output should be a JSON array");
    assert_eq!(modules.len(), 3, "expected exactly 3 modules in output");

    for module in modules {
        let object = module
            .as_object()
            .expect("each module entry should be a JSON object");
        assert!(
            object.get("id").and_then(|v| v.as_str()).is_some(),
            "module entry missing string field 'id': {}",
            module
        );
        assert!(
            object.get("version").and_then(|v| v.as_str()).is_some(),
            "module entry missing string field 'version': {}",
            module
        );
        assert!(
            object.get("status").and_then(|v| v.as_str()).is_some(),
            "module entry missing string field 'status': {}",
            module
        );
    }
}
