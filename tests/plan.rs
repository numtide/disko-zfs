use std::{path::Path, process::Command};

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn none_properties_equal_zero_in_zfs_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_disko-zfs"))
        .args([
            "--file",
            &fixture("none-properties-zfs-list.json"),
            "plan",
            "--spec",
            &fixture("none-properties-spec.json"),
        ])
        .output()
        .expect("disko-zfs plan should run");

    assert!(
        output.status.success(),
        "plan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "# Additive Commands\n# !! Destructive Commands !!\n"
    );
}
