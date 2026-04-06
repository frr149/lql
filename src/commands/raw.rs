use crate::cli::RawOpts;
use crate::client::{Client, GraphQLClient};
use crate::config::Config;

pub fn run(config: &Config, opts: &RawOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;

    // Obtener query: inline > fichero
    let query = if let Some(ref q) = opts.query {
        q.clone()
    } else if let Some(ref path) = opts.file {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read query file {path}: {e}"))?
    } else {
        return Err(
            "No query provided. Use: lql raw 'query { ... }' or --file query.graphql".to_string(),
        );
    };

    // Construir variables
    let mut variables = serde_json::json!({});

    // --vars-file
    if let Some(ref path) = opts.vars_file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read vars file {path}: {e}"))?;
        variables = serde_json::from_str(&content)
            .map_err(|e| format!("Invalid JSON in vars file {path}: {e}"))?;
    }

    // --var key=value (se mergean sobre vars_file)
    if let Some(ref vars) = opts.vars {
        for var in vars {
            let (key, value) = var.split_once('=').ok_or_else(|| {
                format!("Invalid variable format \"{var}\". Use: --var key=value")
            })?;
            variables[key] = serde_json::json!(value);
        }
    }

    let data = client.query(&query, variables)?;

    // Output: JSON crudo formateado
    println!(
        "{}",
        serde_json::to_string_pretty(&data).unwrap_or_default()
    );

    Ok(())
}
