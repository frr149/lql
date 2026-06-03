use crate::config::AuthConfig;
use std::ffi::OsStr;
use std::process::Command as ProcessCommand;

/// Runs an external command and returns its raw output.
///
/// Abstracted as a trait so the resolver can be tested with a mock that never
/// touches the host process table.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<std::process::Output, std::io::Error>;
}

pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<std::process::Output, std::io::Error> {
        let os_args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        ProcessCommand::new(program).args(&os_args).output()
    }
}

/// Resolves the Linear API key.
///
/// See [`AuthConfig`] for the resolution order. This convenience wrapper uses
/// the real process runner; tests should call [`resolve_api_key_with`].
pub fn get_api_key(auth: &AuthConfig) -> Result<String, String> {
    resolve_api_key_with(&RealCommandRunner, auth, |name| std::env::var(name).ok())
}

/// Human-readable description of which credential source will be used.
///
/// Computed with the same precedence as [`get_api_key`] but without invoking
/// the helper, so it's safe to call before the actual resolution. Intended for
/// `lql doctor` so the user knows whether the key came from env, command, or
/// the legacy `api_key_ref` sugar.
pub fn describe_source(auth: &AuthConfig) -> &'static str {
    describe_source_with(auth, |name| std::env::var(name).ok())
}

pub fn describe_source_with(
    auth: &AuthConfig,
    env: impl Fn(&str) -> Option<String>,
) -> &'static str {
    if env("LINEAR_API_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        "LINEAR_API_KEY env var"
    } else if auth.command.is_some() {
        "[auth].command"
    } else if auth.api_key_ref.is_some() {
        "[auth].api_key_ref (1Password)"
    } else {
        "no credential configured"
    }
}

/// Testable variant: callers inject the command runner and the env-var lookup
/// so unit tests don't depend on real process state.
pub fn resolve_api_key_with(
    runner: &dyn CommandRunner,
    auth: &AuthConfig,
    env: impl Fn(&str) -> Option<String>,
) -> Result<String, String> {
    // 1) Env var — universal escape hatch, ideal for CI.
    if let Some(raw) = env("LINEAR_API_KEY") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    // 2) Explicit credential helper command.
    if let Some(cmd) = &auth.command {
        return run_credential_command(runner, cmd);
    }

    // 3) Sugar for the 1Password CLI.
    if let Some(reference) = &auth.api_key_ref {
        return run_credential_command(
            runner,
            &["op".to_string(), "read".to_string(), reference.clone()],
        );
    }

    Err(no_credential_configured_message())
}

fn run_credential_command(runner: &dyn CommandRunner, cmd: &[String]) -> Result<String, String> {
    let Some((program, rest)) = cmd.split_first() else {
        return Err("[auth].command is empty — provide at least the program name, e.g. command = [\"op\", \"read\", \"op://Personal/Linear/api-key\"]".to_string());
    };
    let args: Vec<&str> = rest.iter().map(String::as_str).collect();

    let output = runner.run(program, &args).map_err(|e| {
        format!(
            "Could not run credential helper '{program}': {e}\n  Command: {}\n  Verify the program is installed and on PATH.",
            shell_quote(cmd)
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Credential helper failed.\n  Command: {}\n  Stderr: {}",
            shell_quote(cmd),
            stderr.trim()
        ));
    }

    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if key.is_empty() {
        return Err(format!(
            "Credential helper produced empty output.\n  Command: {}",
            shell_quote(cmd)
        ));
    }
    Ok(key)
}

fn shell_quote(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| {
            if p.chars()
                .any(|c| c.is_whitespace() || c == '"' || c == '\'')
            {
                format!("\"{}\"", p.replace('"', "\\\""))
            } else {
                p.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn no_credential_configured_message() -> String {
    "No Linear API key configured. Pick one of:\n\
       1. Set the LINEAR_API_KEY env var (recommended for CI):\n\
            export LINEAR_API_KEY=<your-key>\n\
       2. Configure a credential helper in ~/.config/lql/config.toml:\n\
            [auth]\n\
            command = [\"op\", \"read\", \"op://<your-vault>/Linear/api-key\"]\n\
            # or [\"pass\", \"show\", \"linear/api-key\"], [\"bw\", \"get\", \"password\", \"Linear\"], ...\n\
       Generate an API key at: https://linear.app/settings/api"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::unix::process::ExitStatusExt;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn env_with(name: &str, value: &str) -> impl Fn(&str) -> Option<String> {
        let name = name.to_string();
        let value = value.to_string();
        move |n| if n == name { Some(value.clone()) } else { None }
    }

    fn output(exit_code: i32, stdout: &str, stderr: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::from_raw(exit_code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    type RunFn = dyn FnMut(&str, &[&str]) -> Result<std::process::Output, std::io::Error>;

    /// Records every invocation so tests can assert on what got run.
    struct MockRunner {
        result: RefCell<Box<RunFn>>,
        calls: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl MockRunner {
        fn always_ok(stdout: &'static str) -> Self {
            Self {
                result: RefCell::new(Box::new(move |_, _| Ok(output(0, stdout, "")))),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn always(out: std::process::Output) -> Self {
            Self {
                result: RefCell::new(Box::new(move |_, _| Ok(out.clone()))),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn always_err(kind: std::io::ErrorKind) -> Self {
            Self {
                result: RefCell::new(Box::new(move |_, _| Err(std::io::Error::new(kind, "mock")))),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn never_called() -> Self {
            Self {
                result: RefCell::new(Box::new(|_, _| {
                    panic!("runner should not have been invoked")
                })),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for MockRunner {
        fn run(
            &self,
            program: &str,
            args: &[&str],
        ) -> Result<std::process::Output, std::io::Error> {
            self.calls.borrow_mut().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            (self.result.borrow_mut())(program, args)
        }
    }

    // 1) Env var wins regardless of what's in [auth].
    #[test]
    fn env_var_wins_over_command() {
        let runner = MockRunner::never_called();
        let auth = AuthConfig {
            command: Some(vec!["op".into(), "read".into(), "op://x/y/z".into()]),
            api_key_ref: None,
        };
        let key =
            resolve_api_key_with(&runner, &auth, env_with("LINEAR_API_KEY", "lin_env")).unwrap();
        assert_eq!(key, "lin_env");
        assert!(runner.calls.borrow().is_empty());
    }

    #[test]
    fn env_var_wins_over_api_key_ref() {
        let runner = MockRunner::never_called();
        let auth = AuthConfig {
            api_key_ref: Some("op://x/y/z".into()),
            command: None,
        };
        let key =
            resolve_api_key_with(&runner, &auth, env_with("LINEAR_API_KEY", "lin_env")).unwrap();
        assert_eq!(key, "lin_env");
    }

    // Whitespace-only env var falls through.
    #[test]
    fn whitespace_env_var_falls_through() {
        let runner = MockRunner::always_ok("lin_from_helper\n");
        let auth = AuthConfig {
            api_key_ref: Some("op://x/y/z".into()),
            command: None,
        };
        let key = resolve_api_key_with(&runner, &auth, env_with("LINEAR_API_KEY", "   ")).unwrap();
        assert_eq!(key, "lin_from_helper");
    }

    // 2) [auth].command runs the configured program.
    #[test]
    fn command_runs_and_trims_output() {
        let runner = MockRunner::always_ok("  lin_xyz  \n");
        let auth = AuthConfig {
            command: Some(vec!["pass".into(), "show".into(), "linear/api-key".into()]),
            api_key_ref: None,
        };
        let key = resolve_api_key_with(&runner, &auth, no_env).unwrap();
        assert_eq!(key, "lin_xyz");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "pass");
        assert_eq!(calls[0].1, vec!["show", "linear/api-key"]);
    }

    #[test]
    fn command_takes_precedence_over_api_key_ref() {
        let runner = MockRunner::always_ok("lin_from_command\n");
        let auth = AuthConfig {
            command: Some(vec!["custom".into()]),
            api_key_ref: Some("op://Personal/Linear/api-key".into()),
        };
        let key = resolve_api_key_with(&runner, &auth, no_env).unwrap();
        assert_eq!(key, "lin_from_command");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "custom");
    }

    #[test]
    fn command_empty_array_errors() {
        let runner = MockRunner::never_called();
        let auth = AuthConfig {
            command: Some(vec![]),
            api_key_ref: None,
        };
        let err = resolve_api_key_with(&runner, &auth, no_env).unwrap_err();
        assert!(err.contains("[auth].command is empty"), "{err}");
    }

    #[test]
    fn command_program_not_found() {
        let runner = MockRunner::always_err(std::io::ErrorKind::NotFound);
        let auth = AuthConfig {
            command: Some(vec!["nope-not-real".into(), "arg".into()]),
            api_key_ref: None,
        };
        let err = resolve_api_key_with(&runner, &auth, no_env).unwrap_err();
        assert!(
            err.contains("Could not run credential helper 'nope-not-real'"),
            "{err}"
        );
        assert!(err.contains("nope-not-real arg"), "{err}");
    }

    #[test]
    fn command_non_zero_exit() {
        let runner = MockRunner::always(output(1, "", "Item not found"));
        let auth = AuthConfig {
            command: Some(vec!["op".into(), "read".into(), "op://x/y/z".into()]),
            api_key_ref: None,
        };
        let err = resolve_api_key_with(&runner, &auth, no_env).unwrap_err();
        assert!(err.contains("Credential helper failed"), "{err}");
        assert!(err.contains("Item not found"), "{err}");
    }

    #[test]
    fn command_empty_stdout_errors() {
        let runner = MockRunner::always_ok("   \n");
        let auth = AuthConfig {
            command: Some(vec!["whatever".into()]),
            api_key_ref: None,
        };
        let err = resolve_api_key_with(&runner, &auth, no_env).unwrap_err();
        assert!(err.contains("empty output"), "{err}");
    }

    // 3) api_key_ref is sugar for `op read <ref>`.
    #[test]
    fn api_key_ref_translates_to_op_read() {
        let runner = MockRunner::always_ok("lin_from_op\n");
        let auth = AuthConfig {
            api_key_ref: Some("op://Personal/Linear/api-key".into()),
            command: None,
        };
        let key = resolve_api_key_with(&runner, &auth, no_env).unwrap();
        assert_eq!(key, "lin_from_op");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "op");
        assert_eq!(calls[0].1, vec!["read", "op://Personal/Linear/api-key"]);
    }

    // 4) Nothing configured → clear guidance.
    #[test]
    fn no_credentials_configured_gives_actionable_error() {
        let runner = MockRunner::never_called();
        let auth = AuthConfig::default();
        let err = resolve_api_key_with(&runner, &auth, no_env).unwrap_err();
        assert!(err.contains("No Linear API key configured"), "{err}");
        assert!(err.contains("LINEAR_API_KEY"), "{err}");
        assert!(err.contains("[auth]"), "{err}");
        assert!(err.contains("linear.app/settings/api"), "{err}");
    }

    // Every op:// reference in the user-facing message must use the
    // placeholder form, not a real vault name. Checking a positive property
    // ("uses a placeholder") avoids hardcoding any specific vault into a
    // public source file.
    #[test]
    fn no_credential_message_uses_only_placeholder_examples() {
        let err = no_credential_configured_message();
        for line in err.lines() {
            if line.contains("op://") {
                assert!(
                    line.contains("<your-vault>"),
                    "op:// reference without <your-vault> placeholder: {line}"
                );
            }
        }
    }

    #[test]
    fn describe_source_reflects_precedence() {
        let env_set = AuthConfig {
            command: Some(vec!["op".into()]),
            api_key_ref: Some("op://x/y/z".into()),
        };
        assert_eq!(
            describe_source_with(&env_set, env_with("LINEAR_API_KEY", "k")),
            "LINEAR_API_KEY env var"
        );
        assert_eq!(describe_source_with(&env_set, no_env), "[auth].command");
        let only_ref = AuthConfig {
            command: None,
            api_key_ref: Some("op://x/y/z".into()),
        };
        assert_eq!(
            describe_source_with(&only_ref, no_env),
            "[auth].api_key_ref (1Password)"
        );
        let empty = AuthConfig::default();
        assert_eq!(
            describe_source_with(&empty, no_env),
            "no credential configured"
        );
    }

    // Whitespace-trimming on stdout doesn't strip internal whitespace.
    #[test]
    fn helper_output_preserves_internal_chars() {
        let runner = MockRunner::always_ok("lin_with_inner_dashes-and-1234\n");
        let auth = AuthConfig {
            command: Some(vec!["echo".into()]),
            api_key_ref: None,
        };
        let key = resolve_api_key_with(&runner, &auth, no_env).unwrap();
        assert_eq!(key, "lin_with_inner_dashes-and-1234");
    }
}
