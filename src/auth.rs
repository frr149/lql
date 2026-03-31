use std::process::Command as ProcessCommand;

/// Trait para ejecutar comandos externos — permite mocking en tests
pub trait CommandRunner {
    fn run_op_read(&self, reference: &str) -> Result<std::process::Output, std::io::Error>;
}

/// Implementación real que ejecuta `op read`
pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run_op_read(&self, reference: &str) -> Result<std::process::Output, std::io::Error> {
        ProcessCommand::new("op").args(["read", reference]).output()
    }
}

/// Lee la API key de 1Password usando op read (con wrapper cache)
pub fn get_api_key(reference: &str) -> Result<String, String> {
    get_api_key_with(&RealCommandRunner, reference)
}

/// Versión testeable: acepta un runner inyectable
pub fn get_api_key_with(runner: &dyn CommandRunner, reference: &str) -> Result<String, String> {
    // Fast path: env var avoids op read entirely
    if let Ok(key) = std::env::var("LINEAR_API_KEY") {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // Existing op read path
    let output = runner.run_op_read(reference).map_err(|e| {
        format!(
            "Could not run 'op read'. Is 1Password CLI installed?\n  Error: {e}\n  Run: op read \"{reference}\"\n  If this fails, check: op signin"
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Could not read API key from 1Password.\n  Run: op read \"{reference}\"\n  If this fails, check: op signin\n  Detail: {stderr}"
        ));
    }

    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if key.is_empty() {
        return Err(format!(
            "API key from 1Password is empty.\n  Run: op read \"{reference}\""
        ));
    }

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::os::unix::process::ExitStatusExt;

    struct MockRunner {
        result: Result<std::process::Output, std::io::ErrorKind>,
    }

    impl CommandRunner for MockRunner {
        fn run_op_read(&self, _reference: &str) -> Result<std::process::Output, std::io::Error> {
            match &self.result {
                Ok(output) => Ok(output.clone()),
                Err(kind) => Err(std::io::Error::new(*kind, "mock error")),
            }
        }
    }

    fn mock_output(exit_code: i32, stdout: &str, stderr: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::from_raw(exit_code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    // ERR-46: op read no encontrado (binary not on PATH)
    #[test]
    fn test_op_not_found() {
        let runner = MockRunner {
            result: Err(std::io::ErrorKind::NotFound),
        };
        let err = get_api_key_with(&runner, "op://test/ref").unwrap_err();
        assert!(err.contains("Could not run 'op read'"), "{err}");
        assert!(err.contains("1Password CLI installed"), "{err}");
    }

    // ERR-47: op read falla (usuario cancela prompt, timeout, etc.)
    #[test]
    fn test_op_read_failed() {
        let runner = MockRunner {
            result: Ok(mock_output(1, "", "authorization prompt dismissed")),
        };
        let err = get_api_key_with(&runner, "op://test/ref").unwrap_err();
        assert!(err.contains("Could not read API key"), "{err}");
        assert!(err.contains("authorization prompt dismissed"), "{err}");
    }

    // op read devuelve key vacía
    #[test]
    fn test_op_read_empty_key() {
        let runner = MockRunner {
            result: Ok(mock_output(0, "", "")),
        };
        let err = get_api_key_with(&runner, "op://test/ref").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    // op read exitoso
    #[test]
    fn test_op_read_success() {
        let runner = MockRunner {
            result: Ok(mock_output(0, "lin_api_abc123\n", "")),
        };
        let key = get_api_key_with(&runner, "op://test/ref").unwrap();
        assert_eq!(key, "lin_api_abc123");
    }

    // op read exitoso con whitespace
    #[test]
    fn test_op_read_trims_whitespace() {
        let runner = MockRunner {
            result: Ok(mock_output(0, "  lin_api_abc123  \n", "")),
        };
        let key = get_api_key_with(&runner, "op://test/ref").unwrap();
        assert_eq!(key, "lin_api_abc123");
    }

    // LINEAR_API_KEY env var takes precedence
    #[test]
    #[serial]
    fn test_env_var_takes_precedence() {
        // SAFETY: test-only, isolated env var manipulation
        unsafe {
            std::env::set_var("LINEAR_API_KEY", "lin_test_from_env");
        }
        let runner = MockRunner {
            result: Err(std::io::ErrorKind::NotFound),
        };
        let key = get_api_key_with(&runner, "op://test/ref").unwrap();
        assert_eq!(key, "lin_test_from_env");
        unsafe {
            std::env::remove_var("LINEAR_API_KEY");
        }
    }

    // Empty env var falls through to op read
    #[test]
    #[serial]
    fn test_empty_env_var_falls_through_to_op() {
        // SAFETY: test-only, isolated env var manipulation
        unsafe {
            std::env::set_var("LINEAR_API_KEY", "");
        }
        let runner = MockRunner {
            result: Ok(mock_output(0, "lin_from_op\n", "")),
        };
        let key = get_api_key_with(&runner, "op://test/ref").unwrap();
        assert_eq!(key, "lin_from_op");
        unsafe {
            std::env::remove_var("LINEAR_API_KEY");
        }
    }

    // Whitespace-only env var falls through to op read
    #[test]
    #[serial]
    fn test_whitespace_env_var_falls_through_to_op() {
        // SAFETY: test-only, isolated env var manipulation
        unsafe {
            std::env::set_var("LINEAR_API_KEY", "   ");
        }
        let runner = MockRunner {
            result: Ok(mock_output(0, "lin_from_op\n", "")),
        };
        let key = get_api_key_with(&runner, "op://test/ref").unwrap();
        assert_eq!(key, "lin_from_op");
        unsafe {
            std::env::remove_var("LINEAR_API_KEY");
        }
    }
}
