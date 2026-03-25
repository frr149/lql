use crate::client::{Client, LinearMeta};
use crate::config::Config;

pub fn run(config: &Config) -> Result<(), String> {
    let mut all_ok = true;

    // 1. Config
    println!("✓ Config loaded from {}", crate::config::config_path().display());

    // 2. Auth
    let client = match Client::new(&config.auth.api_key_ref) {
        Ok(c) => {
            println!("✓ API key loaded from 1Password");
            c
        }
        Err(e) => {
            println!("✗ API key not found. Ferris is sad. 🦀💧");
            println!("  {e}");
            return Ok(()); // No podemos continuar sin auth
        }
    };

    // 3. Conexión a Linear
    match client.query_no_vars(crate::queries::VIEWER_QUERY) {
        Ok(data) => {
            let name = data
                .get("viewer")
                .and_then(|v| v.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");
            let email = data
                .get("viewer")
                .and_then(|v| v.get("email"))
                .and_then(|e| e.as_str())
                .unwrap_or("unknown");
            println!("✓ Connected to Linear as {name} ({email})");
        }
        Err(e) => {
            println!("✗ Could not connect to Linear API");
            println!("  {e}");
            all_ok = false;
        }
    }

    // 4. Metadata (teams, labels)
    match LinearMeta::fetch(&client) {
        Ok(meta) => {
            let team_keys: Vec<&str> = meta.teams.iter().map(|t| t.key.as_str()).collect();
            println!("✓ Teams: {}", team_keys.join(", "));
            println!("✓ Labels: {} available", meta.labels.len());

            // Verificar context-map
            let cwd = std::env::current_dir().unwrap_or_default();
            match config.resolve_context(&cwd) {
                Some(ctx) => {
                    println!("✓ Context: {} → team={}", cwd.display(), ctx.team);
                    if let Some(p) = &ctx.project {
                        print!(" project={p}");
                    }
                    if let Some(l) = &ctx.label {
                        print!(" label={l}");
                    }
                    println!();
                }
                None => {
                    println!("ℹ No context-map match for {}", cwd.display());
                }
            }

            // Verificar teams del context-map existen
            for (path, entry) in &config.context_map {
                if meta.find_team(&entry.team).is_err() {
                    println!("✗ Context-map {path}: team \"{}\" not found in Linear", entry.team);
                    all_ok = false;
                }
                if let Some(ref label) = entry.label {
                    if meta.find_label(label).is_err() {
                        println!("✗ Context-map {path}: label \"{label}\" not found in Linear");
                        all_ok = false;
                    }
                }
            }
        }
        Err(e) => {
            println!("✗ Could not fetch metadata: {e}");
            all_ok = false;
        }
    }

    if all_ok {
        println!("\n✓ All checks passed. Ferris approves. 🦀");
    }

    Ok(())
}
