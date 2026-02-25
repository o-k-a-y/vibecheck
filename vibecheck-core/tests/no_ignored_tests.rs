use std::path::Path;

#[test]
fn no_ignored_tests_in_codebase() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dirs = [root.join("src"), root.join("tests")];
    let mut violations = Vec::new();

    for dir in &dirs {
        scan_dir(dir, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "#[ignore] found in test code — failing tests should fail, not hide:\n{}",
        violations.join("\n")
    );
}

fn scan_dir(dir: &Path, violations: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, violations);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && path.file_name().and_then(|n| n.to_str()) != Some("no_ignored_tests.rs")
        {
            check_file(&path, violations);
        }
    }
}

fn check_file(path: &Path, violations: &mut Vec<String>) {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    for (i, line) in contents.lines().enumerate() {
        if line.trim() == "#[ignore]" || line.trim().starts_with("#[ignore]") {
            violations.push(format!(
                "  {}:{}: {}",
                path.display(),
                i + 1,
                line.trim()
            ));
        }
    }
}
