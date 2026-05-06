use crate::cli::{LabelsAction, LabelsCreateOpts, LabelsDeleteOpts, LabelsOpts};
use crate::client::{Client, GraphQLClient, LinearMeta};
use crate::config::Config;

pub fn run(config: &Config, opts: &LabelsOpts) -> Result<(), String> {
    let client = Client::new(&config.auth.api_key_ref)?;

    match &opts.action {
        None => list(&client, opts.json, opts.team.as_deref()),
        Some(LabelsAction::List(list_opts)) => {
            let json = list_opts.json || opts.json;
            let team = list_opts.team.as_deref().or(opts.team.as_deref());
            list(&client, json, team)
        }
        Some(LabelsAction::Create(create_opts)) => create(&client, create_opts),
        Some(LabelsAction::Delete(delete_opts)) => delete(&client, delete_opts),
    }
}

fn list(client: &dyn GraphQLClient, json: bool, _team: Option<&str>) -> Result<(), String> {
    let meta = LinearMeta::fetch(client)?;
    let labels = &meta.labels;

    if json {
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

fn create(client: &dyn GraphQLClient, opts: &LabelsCreateOpts) -> Result<(), String> {
    let meta = LinearMeta::fetch(client)?;

    // Comprobar si ya existe (case-insensitive)
    if meta.find_label(&opts.name).is_ok() {
        return Err(format!("Label \"{}\" already exists.", opts.name));
    }

    let mut input = serde_json::json!({
        "name": opts.name,
    });

    if let Some(color) = &opts.color {
        input["color"] = serde_json::json!(color);
    }

    if let Some(team_key) = &opts.team {
        let team = meta.find_team(team_key)?;
        input["teamId"] = serde_json::json!(team.id);
    }

    let data = client.query(crate::queries::LABEL_CREATE_MUTATION, serde_json::json!({"input": input}))?;

    let success = data
        .get("issueLabelCreate")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if !success {
        return Err("Linear API rejected label creation.".to_string());
    }

    let label = &data["issueLabelCreate"]["issueLabel"];
    let name = label.get("name").and_then(|n| n.as_str()).unwrap_or("?");
    let id = label.get("id").and_then(|i| i.as_str()).unwrap_or("?");

    if opts.json {
        println!("{}", serde_json::json!({"name": name, "id": id}));
    } else {
        eprintln!("✓ Label \"{name}\" created.");
    }

    Ok(())
}

fn delete(client: &dyn GraphQLClient, opts: &LabelsDeleteOpts) -> Result<(), String> {
    let meta = LinearMeta::fetch(client)?;

    let label = meta.find_label(&opts.name)?;
    let label_id = label.id.clone();
    let label_name = label.name.clone();

    let data = client.query(
        crate::queries::LABEL_DELETE_MUTATION,
        serde_json::json!({"id": label_id}),
    )?;

    let success = data
        .get("issueLabelDelete")
        .and_then(|c| c.get("success"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    if !success {
        return Err(format!("Linear API rejected deletion of label \"{label_name}\"."));
    }

    eprintln!("✓ Label \"{label_name}\" deleted.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::GraphQLClient;
    use serde_json::Value;
    use std::cell::RefCell;

    /// Mock que devuelve meta fixture para la primera query y respuesta custom para la segunda
    struct MockClient {
        calls: RefCell<Vec<String>>,
        meta_data: Value,
        mutation_response: Result<Value, String>,
    }

    impl MockClient {
        fn new(mutation_response: Result<Value, String>) -> Self {
            let path = format!("{}/tests/fixtures/meta.json", env!("CARGO_MANIFEST_DIR"));
            let content = std::fs::read_to_string(&path).unwrap();
            let fixture: Value = serde_json::from_str(&content).unwrap();
            Self {
                calls: RefCell::new(Vec::new()),
                meta_data: fixture["data"].clone(),
                mutation_response,
            }
        }
    }

    impl GraphQLClient for MockClient {
        fn query(&self, query: &str, _variables: Value) -> Result<Value, String> {
            self.calls.borrow_mut().push(query.to_string());
            // Primera llamada = meta query
            if self.calls.borrow().len() == 1 {
                Ok(self.meta_data.clone())
            } else {
                self.mutation_response.clone()
            }
        }
    }

    // --- Create tests ---

    #[test]
    fn test_create_success() {
        let client = MockClient::new(Ok(serde_json::json!({
            "issueLabelCreate": {
                "success": true,
                "issueLabel": {
                    "id": "new-id-123",
                    "name": "new-label",
                    "color": "#ff0000"
                }
            }
        })));

        let opts = LabelsCreateOpts {
            name: "new-label".to_string(),
            color: Some("#ff0000".to_string()),
            team: None,
            json: false,
        };

        let result = create(&client, &opts);
        assert!(result.is_ok(), "Create should succeed: {result:?}");
        assert_eq!(client.calls.borrow().len(), 2); // meta + create
    }

    #[test]
    fn test_create_duplicate_rejected() {
        let client = MockClient::new(Ok(serde_json::json!({})));

        let opts = LabelsCreateOpts {
            name: "reactor".to_string(), // ya existe en el fixture
            color: None,
            team: None,
            json: false,
        };

        let result = create(&client, &opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
        // Solo meta query, nunca llama a la mutación
        assert_eq!(client.calls.borrow().len(), 1);
    }

    #[test]
    fn test_create_duplicate_case_insensitive() {
        let client = MockClient::new(Ok(serde_json::json!({})));

        let opts = LabelsCreateOpts {
            name: "REACTOR".to_string(),
            color: None,
            team: None,
            json: false,
        };

        let result = create(&client, &opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    #[test]
    fn test_create_api_rejects() {
        let client = MockClient::new(Ok(serde_json::json!({
            "issueLabelCreate": {
                "success": false,
                "issueLabel": null
            }
        })));

        let opts = LabelsCreateOpts {
            name: "brand-new".to_string(),
            color: None,
            team: None,
            json: false,
        };

        let result = create(&client, &opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rejected"));
    }

    #[test]
    fn test_create_with_team() {
        let client = MockClient::new(Ok(serde_json::json!({
            "issueLabelCreate": {
                "success": true,
                "issueLabel": {
                    "id": "new-id-456",
                    "name": "team-label",
                    "color": null
                }
            }
        })));

        let opts = LabelsCreateOpts {
            name: "team-label".to_string(),
            color: None,
            team: Some("PROD".to_string()),
            json: false,
        };

        let result = create(&client, &opts);
        assert!(result.is_ok(), "Create with team should succeed: {result:?}");
    }

    #[test]
    fn test_create_with_invalid_team() {
        let client = MockClient::new(Ok(serde_json::json!({})));

        let opts = LabelsCreateOpts {
            name: "some-label".to_string(),
            color: None,
            team: Some("NONEXISTENT".to_string()),
            json: false,
        };

        let result = create(&client, &opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    // --- Delete tests ---

    #[test]
    fn test_delete_success() {
        let client = MockClient::new(Ok(serde_json::json!({
            "issueLabelDelete": {
                "success": true
            }
        })));

        let opts = LabelsDeleteOpts {
            name: "reactor".to_string(), // existe en fixture
        };

        let result = delete(&client, &opts);
        assert!(result.is_ok(), "Delete should succeed: {result:?}");
        assert_eq!(client.calls.borrow().len(), 2); // meta + delete
    }

    #[test]
    fn test_delete_nonexistent_label() {
        let client = MockClient::new(Ok(serde_json::json!({})));

        let opts = LabelsDeleteOpts {
            name: "does-not-exist".to_string(),
        };

        let result = delete(&client, &opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
        // Solo meta query
        assert_eq!(client.calls.borrow().len(), 1);
    }

    #[test]
    fn test_delete_api_rejects() {
        let client = MockClient::new(Ok(serde_json::json!({
            "issueLabelDelete": {
                "success": false
            }
        })));

        let opts = LabelsDeleteOpts {
            name: "reactor".to_string(),
        };

        let result = delete(&client, &opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rejected"));
    }
}
