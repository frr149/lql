use crate::cli::LabelsOpts;
use crate::client::{Client, LinearMeta};
use crate::config::Config;

pub fn run(config: &Config, opts: &LabelsOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;
    let meta = LinearMeta::fetch(&client)?;

    let labels = &meta.labels;

    if opts.json {
        for label in labels {
            println!(
                "{}",
                serde_json::json!({"name": label.name, "id": label.id})
            );
        }
    } else {
        for label in labels {
            println!("{}", label.name);
        }
        println!("── {} labels", labels.len());
    }

    Ok(())
}
