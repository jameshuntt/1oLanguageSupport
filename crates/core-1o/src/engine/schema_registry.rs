use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIssue {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SchemaRegistry {
    schemas: HashMap<(String, String), Value>,
    issues: Vec<SchemaIssue>,
}

impl SchemaRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load workspace schemas from `<workspace-root>/schema/<modifier>/<version>.schema.json`.
    /// Malformed or unreadable files are recorded as bounded issues; they never panic the LSP.
    pub fn load_from_dir(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        if !root.exists() {
            return Self::empty();
        }

        let mut registry = Self::empty();
        for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(parent) = path.parent() else { continue };
            if parent == root {
                // `schema/1o.version.json` is a version sheet, not a modifier schema.
                continue;
            }
            let Some(modifier) = parent.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let version = stem.strip_suffix(".schema").unwrap_or(stem);
            match fs::read_to_string(path) {
                Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                    Ok(schema) => {
                        registry.schemas.insert(
                            (modifier.to_string(), version.to_string()),
                            schema,
                        );
                    }
                    Err(error) => registry.issues.push(SchemaIssue {
                        path: path.display().to_string(),
                        message: format!("malformed schema JSON: {error}"),
                    }),
                },
                Err(error) => registry.issues.push(SchemaIssue {
                    path: path.display().to_string(),
                    message: format!("schema read failed: {error}"),
                }),
            }
        }
        registry
    }

    /// Merge the shared 1o VOCABULARY (the kernel's `crates/schemas` layout:
    /// `<family dirs>/<name>.1o.schema.json`, versioned v1 in the title) into
    /// this registry under version "v1". Project-local schemas keep priority:
    /// a (modifier, version) already present is never overwritten — the
    /// vocabulary fills gaps, it does not shadow a workspace's own files.
    /// One source of truth for shapes; N readers deriving from it.
    pub fn merge_vocabulary_dir(&mut self, root: impl AsRef<Path>) {
        let root = root.as_ref();
        if !root.exists() {
            return;
        }
        for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !path.is_file() || !name.ends_with(".1o.schema.json") {
                continue;
            }
            let modifier = name.trim_end_matches(".1o.schema.json").to_string();
            let key = (modifier, "v1".to_string());
            if self.schemas.contains_key(&key) {
                continue;
            }
            match fs::read_to_string(path) {
                Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                    Ok(schema) => {
                        self.schemas.insert(key, schema);
                    }
                    Err(error) => self.issues.push(SchemaIssue {
                        path: path.display().to_string(),
                        message: format!("malformed vocabulary schema JSON: {error}"),
                    }),
                },
                Err(error) => self.issues.push(SchemaIssue {
                    path: path.display().to_string(),
                    message: format!("vocabulary schema read failed: {error}"),
                }),
            }
        }
    }

    /// Merge versioned `schema/<modifier>/<version>.schema.json` entries.
    /// Earlier directories win, so the workspace's own schemas override packages.
    pub fn merge_versioned_dir(&mut self, root: impl AsRef<Path>) {
        let next = Self::load_from_dir(root);
        for (key, value) in next.schemas {
            self.schemas.entry(key).or_insert(value);
        }
        self.issues.extend(next.issues);
    }

    /// Fill any remaining v1 schema gaps from the vocabulary compiled into core-1o.
    /// Workspace and explicitly supplied schemas retain priority.
    pub fn merge_embedded_vocabulary(&mut self) {
        for &(modifier, raw) in crate::embedded_vocabulary::EMBEDDED_VOCABULARY {
            let key = (modifier.to_string(), "v1".to_string());
            if self.schemas.contains_key(&key) {
                continue;
            }
            match serde_json::from_str::<Value>(raw) {
                Ok(schema) => { self.schemas.insert(key, schema); }
                Err(error) => self.issues.push(SchemaIssue {
                    path: format!("embedded:{modifier}"),
                    message: format!("malformed embedded vocabulary schema JSON: {error}"),
                }),
            }
        }
    }

    pub fn get(&self, modifier: &str, version: &str) -> Option<&Value> {
        self.schemas
            .get(&(modifier.to_string(), version.to_string()))
    }

    pub fn issues(&self) -> &[SchemaIssue] {
        &self.issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn malformed_schema_is_recorded_without_panicking() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("service");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("v1.schema.json"), "{").unwrap();
        let registry = SchemaRegistry::load_from_dir(temp.path());
        assert!(registry.get("service", "v1").is_none());
        assert_eq!(registry.issues().len(), 1);
    }
}

#[cfg(test)]
mod vocabulary_tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn vocabulary_merges_under_v1_and_local_schemas_keep_priority() {
        let dir = tempfile::tempdir().unwrap();
        let vocabulary = dir.path().join("crates/schemas");
        write(
            &vocabulary.join("ui/page/page.1o.schema.json"),
            r#"{"title": "1o page v1", "type": "object", "required": ["page"]}"#,
        );
        write(
            &vocabulary.join("nodeplan/daemon_harness.1o.schema.json"),
            r#"{"title": "1o daemon harness declaration v1", "type": "object"}"#,
        );

        // A registry with a LOCAL page schema already present.
        let mut registry = SchemaRegistry::empty();
        registry.schemas.insert(
            ("page".to_string(), "v1".to_string()),
            serde_json::json!({"title": "local override"}),
        );
        registry.merge_vocabulary_dir(&vocabulary);

        // The gap was filled…
        assert!(registry.get("daemon_harness", "v1").is_some());
        // …and the local schema was NOT shadowed.
        assert_eq!(
            registry.get("page", "v1").unwrap()["title"],
            serde_json::json!("local override")
        );
        assert!(registry.issues().is_empty());
    }

    #[test]
    fn embedded_vocabulary_contains_program() {
        let mut registry = SchemaRegistry::empty();
        registry.merge_embedded_vocabulary();
        assert!(registry.get("program", "v1").is_some());
        assert!(registry.get("application_action", "v1").is_some());
    }

    #[test]
    fn missing_vocabulary_dir_changes_nothing() {
        let mut registry = SchemaRegistry::empty();
        registry.merge_vocabulary_dir("/nonexistent/vocabulary");
        assert!(registry.get("page", "v1").is_none());
        assert!(registry.issues().is_empty());
    }
}
