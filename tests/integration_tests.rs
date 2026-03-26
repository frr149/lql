/// Tests de integración contra la API real de Linear.
///
/// Estos tests están marcados con #[ignore] por defecto.
/// Ejecutar con: cargo test -- --ignored
///
/// Requieren:
/// - API key de Linear en 1Password (op read "op://Private/Linear/api-key")
/// - Conexión a internet
/// - Issues reales en Linear (PROD, TOOL teams)
///
/// NOTA: Estos tests LEEN de Linear, nunca escriben. Son seguros de ejecutar.

use std::process::Command;

/// Helper: ejecuta lql con args y devuelve (exit_code, stdout, stderr)
fn run_lql(args: &[&str]) -> (i32, String, String) {
    let binary = format!("{}/target/debug/lql", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(&binary)
        .args(args)
        .output()
        .expect("Failed to execute lql binary");

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (code, stdout, stderr)
}

// --- ERR-53: issue no encontrada ---

#[test]
#[ignore]
fn integration_view_nonexistent_issue() {
    let (code, _stdout, stderr) = run_lql(&["view", "PROD-99999"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// --- ERR-64: search sin resultados ---

#[test]
#[ignore]
fn integration_search_no_results() {
    let (code, stdout, _stderr) = run_lql(&["search", "xyznonexistent123456", "--team", "PROD"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("0 issues"), "stdout: {stdout}");
}

// --- ERR-61: search encuentra por título ---

#[test]
#[ignore]
fn integration_search_finds_issues() {
    let (code, stdout, _stderr) = run_lql(&["search", "lql", "--team", "TOOL"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("TOOL-"), "stdout: {stdout}");
}

// --- ERR-74: dos list simultáneos no interfieren ---

#[test]
#[ignore]
fn integration_concurrent_list() {
    let binary = format!("{}/target/debug/lql", env!("CARGO_MANIFEST_DIR"));

    let child1 = Command::new(&binary)
        .args(["list", "--team", "PROD", "--limit", "3"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn lql 1");

    let child2 = Command::new(&binary)
        .args(["list", "--team", "TOOL", "--limit", "3"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn lql 2");

    let output1 = child1.wait_with_output().unwrap();
    let output2 = child2.wait_with_output().unwrap();

    let stderr1 = String::from_utf8_lossy(&output1.stderr);
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(output1.status.success(), "lql 1 failed: {stderr1}");
    assert!(output2.status.success(), "lql 2 failed: {stderr2}");
}

// --- Middleware: flags erróneos ---

#[test]
#[ignore]
fn integration_filter_flag_rejected() {
    let (code, _stdout, stderr) = run_lql(&["list", "--filter", "backlog"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("--filter no existe"), "stderr: {stderr}");
}

// --- Doctor funciona ---

#[test]
#[ignore]
fn integration_doctor() {
    let (code, stdout, _stderr) = run_lql(&["doctor"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("✓") || stdout.contains("teams"), "stdout: {stdout}");
}

// --- Labels funciona ---

#[test]
#[ignore]
fn integration_labels() {
    let binary = format!("{}/target/debug/lql", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(&binary)
        .args(["labels"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to run lql labels");
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("tokamak") || stdout.contains("lql"), "stdout: {stdout}");
}

// --- List con --json produce JSONL válido ---

#[test]
#[ignore]
fn integration_list_json() {
    let (code, stdout, _stderr) = run_lql(&["list", "--team", "TOOL", "--limit", "3", "--json"]);
    assert_eq!(code, 0);
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        assert!(parsed.is_ok(), "Invalid JSONL line: {line}");
    }
}

// --- Context desde cwd ---

#[test]
#[ignore]
fn integration_context() {
    let binary = format!("{}/target/debug/lql", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(&binary)
        .args(["context"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to execute lql context");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("TOOL"), "stdout: {stdout}");
}
