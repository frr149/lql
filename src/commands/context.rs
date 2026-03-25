use crate::config::Config;

pub fn run(config: &Config) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Could not get cwd: {e}"))?;

    match config.resolve_context(&cwd) {
        Some(ctx) => {
            println!("Context: {}", cwd.display());
            println!("  Team: {}", ctx.team);
            if let Some(ref project) = ctx.project {
                println!("  Project: {project}");
            }
            if let Some(ref label) = ctx.label {
                println!("  Label: {label}");
            }
            println!("  Source: {}", ctx.source);
        }
        None => {
            println!("Context: {}", cwd.display());
            println!("  No context-map match for this directory.");
            println!(
                "  Use --team to specify, or add an entry in {}",
                crate::config::config_path().display()
            );
        }
    }

    Ok(())
}
