use std::process::Command as ProcessCommand;

/// Lee la API key de 1Password usando op read (con wrapper cache)
pub fn get_api_key(reference: &str) -> Result<String, String> {
    let output = ProcessCommand::new("op")
        .args(["read", reference])
        .output()
        .map_err(|e| {
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
