use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::engine::{SchemaRegistry, VersionSheet};
use crate::utils::parse_filename;

#[derive(Debug, Clone, Default)]
pub struct WorkspaceIndex {
    pub containers: HashMap<String, FileRecord>,
    pub models: HashMap<String, FileRecord>,
    pub slices: HashMap<String, FileRecord>,
    pub views: HashMap<String, FileRecord>,
    pub raw_files: HashMap<String, FileRecord>,
}

impl WorkspaceIndex {
    pub fn list_containers(&self) -> Vec<String> { sorted_keys(&self.containers) }
    pub fn list_models(&self) -> Vec<String> { sorted_keys(&self.models) }
    pub fn list_slices(&self) -> Vec<String> { sorted_keys(&self.slices) }
    pub fn list_views(&self) -> Vec<String> { sorted_keys(&self.views) }

    pub fn remove_path(&mut self, path: &Path) {
        self.containers.retain(|_, record| record.path.as_path() != path);
        self.models.retain(|_, record| record.path.as_path() != path);
        self.slices.retain(|_, record| record.path.as_path() != path);
        self.views.retain(|_, record| record.path.as_path() != path);
        self.raw_files.retain(|_, record| record.path.as_path() != path);
    }

    pub fn upsert_path(&mut self, path: &Path) {
        self.remove_path(path);
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            return;
        };
        let Some(mut record) = parse_filename(file_name) else {
            return;
        };
        record.path = path.to_path_buf();
        match record.modifier.as_str() {
            "container" => { self.containers.insert(record.name.clone(), record); }
            "model" => { self.models.insert(record.name.clone(), record); }
            "slice" => { self.slices.insert(record.name.clone(), record); }
            "view" => { self.views.insert(record.name.clone(), record); }
            _ => {
                self.raw_files
                    .insert(format!("{}:{}", record.modifier, record.name), record);
            }
        }
    }

    pub fn files_with_modifier(&self, modifier: &str) -> Vec<FileRecord> {
        let mut records = self
            .raw_files
            .values()
            .filter(|record| record.modifier == modifier)
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.path.cmp(&right.path));
        records
    }
}

fn sorted_keys(map: &HashMap<String, FileRecord>) -> Vec<String> {
    let mut keys = map.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys
}

#[derive(Debug, Clone, Default)]
pub struct FileRecord {
    pub name: String,
    pub version: String,
    pub modifier: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub index: WorkspaceIndex,
    pub schema_registry: SchemaRegistry,
    pub version_sheet: VersionSheet,
}

/// Optional shared vocabulary directories. Local workspace schemas always win.
/// A project manifest can declare additional schema directories; paths must
/// stay within the workspace to avoid implicit reading of arbitrary locations.
fn load_schema_registry(root: &Path) -> SchemaRegistry {
    let schema_dir = root.join("schema");
    let mut registry = SchemaRegistry::load_from_dir(&schema_dir);
    let manifest = root.join("oneo.project.json");
    if let Ok(raw) = std::fs::read_to_string(manifest) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            if value.get("formatVersion").and_then(|v| v.as_u64()) == Some(1) {
                if let Some(dirs) = value.get("schemaDirectories").and_then(|v| v.as_array()) {
                    for dir in dirs.iter().filter_map(|v| v.as_str()) {
                        let path = Path::new(dir);
                        // Relative, bounded paths only; don't follow external symlinks.
                        if path.is_absolute() || path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                            continue;
                        }
                        let candidate = root.join(path);
                        if let (Ok(base), Ok(real)) = (root.canonicalize(), candidate.canonicalize()) {
                            if real.starts_with(base) && real.is_dir() {
                                registry.merge_versioned_dir(&real);
                                registry.merge_vocabulary_dir(&real);
                            }
                        }
                    }
                }
            }
        }
    }
    // Original vocabulary branch compatibility: an explicit env var (new name
    // preferred) or the NodePlan checkout's `crates/schemas` fallback.
    let explicit = std::env::var_os("ONEO_SCHEMAS")
        .or_else(|| std::env::var_os("NODEPLAN_1O_SCHEMAS"));
    let vocab = explicit.map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| {
            let p = root.join("crates/schemas");
            p.is_dir().then_some(p)
        });
    if let Some(vocab) = vocab {
        registry.merge_vocabulary_dir(vocab);
    }
    // core-1o always carries the shipped vocabulary, so applications, the
    // standalone server and the VS Code extension agree even without external files.
    registry.merge_embedded_vocabulary();
    registry
}

impl Workspace {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let schema_dir = root.join("schema");
        let schema_registry = load_schema_registry(&root);
        Self {
            root,
            index: WorkspaceIndex::default(),
            schema_registry,
            version_sheet: VersionSheet::load(schema_dir.join("1o.version.json")),
        }
    }

    pub fn reindex(&mut self) {
        self.index = WorkspaceIndex::default();
        for entry in WalkDir::new(&self.root).into_iter().flatten() {
            let path = entry.path();
            if path.is_file() {
                self.index.upsert_path(path);
            }
        }
    }

    /// Bounded file-event refresh: update only the named 1o path instead of
    /// rescanning the whole workspace on every edit.
    pub fn refresh_path(&mut self, path: &Path) {
        self.index.upsert_path(path);
    }

    pub fn remove_path(&mut self, path: &Path) {
        self.index.remove_path(path);
    }

    pub fn reload_schema_state(&mut self) {
        let schema_dir = self.root.join("schema");
        self.schema_registry = load_schema_registry(&self.root);
        self.version_sheet = VersionSheet::load(schema_dir.join("1o.version.json"));
    }

    pub fn schema_for_file(&self, modifier: &str, file_version: &str) -> Option<serde_json::Value> {
        let schema_version = self
            .version_sheet
            .schema_version_for(file_version, modifier);
        self.schema_registry
            .get(modifier, &schema_version)
            .cloned()
            .or_else(|| crate::engine::builtin_schema_for_modifier(modifier))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn workspace_schema_lookup_uses_schema_directory_and_version_sheet() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("schema/service")).unwrap();
        fs::write(
            temp.path().join("schema/1o.version.json"),
            r#"{"versions":{"v1":{"service":"v1"}}}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("schema/service/v1.schema.json"),
            r#"{"type":"object","properties":{"service":{"type":"object"}}}"#,
        )
        .unwrap();
        let workspace = Workspace::new(temp.path());
        assert!(workspace.schema_for_file("service", "v1").is_some());
    }

    #[test]
    fn targeted_refresh_tracks_custom_modifier_without_full_reindex() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("v1.local.1o.nodeplan");
        fs::write(&path, "{ nodeplan { workspace_id \"w\" } }").unwrap();
        let mut workspace = Workspace::new(temp.path());
        workspace.refresh_path(&path);
        assert_eq!(workspace.index.files_with_modifier("nodeplan").len(), 1);
        workspace.remove_path(&path);
        assert!(workspace.index.files_with_modifier("nodeplan").is_empty());
    }

    #[test]
    fn builtin_walkthrough_schema_is_available_without_workspace_schema_directory() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path());
        assert!(workspace.schema_for_file("walkthrough", "v1").is_some());
    }

}

#[cfg(test)]
mod integration_manifest_tests {
    use super::*;
    use std::fs;
    #[test]
    fn declared_vocabulary_is_visible_and_workspace_schema_wins() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("schema/page");
        let b = root.path().join("implementation/schemas/ui/page");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("v1.schema.json"), r#"{"title":"local"}"#).unwrap();
        fs::write(b.join("page.1o.schema.json"), r#"{"title":"external"}"#).unwrap();
        fs::write(root.path().join("oneo.project.json"), r#"{"formatVersion":1,"schemaDirectories":["implementation/schemas"]}"#).unwrap();
        let workspace = Workspace::new(root.path());
        assert_eq!(workspace.schema_registry.get("page", "v1").unwrap()["title"], "local");
    }
    #[test]
    fn declares_new_domain_without_server_source_changes() {
        let root = tempfile::tempdir().unwrap();
        let b = root.path().join("implementation/schemas/custom");
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("v1.schema.json"), r#"{"title":"custom"}"#).unwrap();
        fs::write(root.path().join("oneo.project.json"), r#"{"formatVersion":1,"schemaDirectories":["implementation/schemas"]}"#).unwrap();
        let workspace = Workspace::new(root.path());
        assert_eq!(workspace.schema_registry.get("custom", "v1").unwrap()["title"], "custom");
    }
    #[test]
    fn new_file_modifier_uses_project_schema_without_new_extension_registration() {
        let root = tempfile::tempdir().unwrap();
        let schema_dir = root.path().join("schema/invoice");
        fs::create_dir_all(&schema_dir).unwrap();
        fs::write(
            schema_dir.join("v1.schema.json"),
            r#"{"type":"object","required":["invoice"],"properties":{"invoice":{"type":"object","required":["recipient"],"properties":{"recipient":{"type":"string"}}}}}"#,
        ).unwrap();
        let record = crate::utils::parse_filename("v1.demo.1o.invoice").unwrap();
        assert_eq!(record.modifier, "invoice");
        let workspace = Workspace::new(root.path());
        assert!(workspace.schema_for_file(&record.modifier, &record.version).is_some());
        let valid = crate::engine::diagnostics_for_text_workspace(
            &workspace, &record.modifier, &record.version,
            r#"{ invoice { recipient "Example" } }"#,
        );
        assert!(!valid.iter().any(|d| matches!(d.severity, crate::engine::EngineSeverity::Error)), "{valid:?}");
        let invalid = crate::engine::diagnostics_for_text_workspace(
            &workspace, &record.modifier, &record.version,
            r#"{ invoice { recipient 42 } }"#,
        );
        assert!(invalid.iter().any(|d| matches!(d.severity, crate::engine::EngineSeverity::Error)), "{invalid:?}");
    }
    #[test]
    fn refuses_relative_path_escape_from_manifest() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("oneo.project.json"), r#"{"formatVersion":1,"schemaDirectories":["../elsewhere"]}"#).unwrap();
        let workspace = Workspace::new(root.path());
        assert!(workspace.schema_registry.get("elsewhere", "v1").is_none());
    }
}
