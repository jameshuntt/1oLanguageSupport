use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct VersionSheet {
    pub versions: HashMap<String, HashMap<String, String>>,
}

impl VersionSheet {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if !path.exists() {
            eprintln!("[1o-lsp] version sheet not found at {}", path.display());
            return Self::default();
        }
        match fs::read_to_string(path) {
            Ok(raw) => match serde_json::from_str::<Self>(&raw) {
                Ok(sheet) => sheet,
                Err(error) => {
                    eprintln!(
                        "[1o-lsp] malformed version sheet {}: {error}",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(error) => {
                eprintln!("[1o-lsp] failed to read version sheet {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub fn schema_version_for(&self, file_version: &str, modifier: &str) -> String {
        self.versions
            .get(file_version)
            .and_then(|per_modifier| per_modifier.get(modifier))
            .cloned()
            .unwrap_or_else(|| file_version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn version_sheet_resolves_modifier_schema_version() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("1o.version.json");
        fs::write(
            &path,
            r#"{"versions":{"v1":{"service":"v2026"}}}"#,
        )
        .unwrap();
        let sheet = VersionSheet::load(&path);
        assert_eq!(sheet.schema_version_for("v1", "service"), "v2026");
        assert_eq!(sheet.schema_version_for("v1", "model"), "v1");
    }
}
