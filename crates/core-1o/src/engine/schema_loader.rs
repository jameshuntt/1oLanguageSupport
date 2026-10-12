use serde_json::Value;
use std::fs;

pub fn load_schema(path: &str) -> Result<Value, String> {
    let data = fs::read_to_string(path)
        .map_err(|error| format!("schema file read failed for {path}: {error}"))?;
    serde_json::from_str(&data)
        .map_err(|error| format!("invalid schema JSON for {path}: {error}"))
}
