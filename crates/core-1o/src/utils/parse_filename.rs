// qweres/infra/language-server-1o/src/utils/parse_file_name.rs

use std::path::PathBuf;

use crate::engine::FileRecord;


pub fn parse_filename(filename: &str) -> Option<FileRecord> {
    // Canonical format:
    //   v1.name.1o.modifier
    //
    // `name` may itself be dotted. This lets domain-qualified declarations such as
    // `v1.archive.overview.1o.walkthrough` retain both their semantic identifier and 1o modifier
    // without forcing a second registry-friendly alias.
    let parts: Vec<&str> = filename.split('.').collect();
    if parts.len() < 4 {
        return None;
    }

    let version = parts.first()?.to_string();
    let modifier = parts.last()?.to_string();
    if parts.get(parts.len().checked_sub(2)?)? != &"1o" {
        return None;
    }
    let name_parts = &parts[1..parts.len() - 2];
    if name_parts.is_empty() || name_parts.iter().any(|value| value.is_empty()) {
        return None;
    }
    let name = name_parts.join(".");

    Some(FileRecord {
        name,
        version,
        modifier,
        path: PathBuf::new(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v1_name_1o_modifier_convention() {
        let record = parse_filename("v1.type_serverd.1o.service").unwrap();
        assert_eq!(record.version, "v1");
        assert_eq!(record.name, "type_serverd");
        assert_eq!(record.modifier, "service");
    }

    #[test]
    fn parses_dotted_semantic_name_without_losing_modifier() {
        let record = parse_filename("v1.archive.file_ingest_authority_recovery.1o.walkthrough").unwrap();
        assert_eq!(record.version, "v1");
        assert_eq!(record.name, "archive.file_ingest_authority_recovery");
        assert_eq!(record.modifier, "walkthrough");
    }

    #[test]
    fn rejects_non_1o_filename() {
        assert!(parse_filename("type_serverd.service").is_none());
        assert!(parse_filename("v1.archive.overview.walkthrough").is_none());
    }

    #[test]
    fn parses_dotted_ui_composition_artifact_roles() {
        for (name, modifier) in [
            ("v1.organization.access_designer.1o.page", "page"),
            ("v1.access.summary.1o.section", "section"),
            ("v1.organization.access_hub.1o.overlay_zone", "overlay_zone"),
            ("v1.action_button.1o.component", "component"),
            ("v1.archive_manager.controls.1o.resource", "resource"),
            ("v1.archive_manager.1o.theme", "theme"),
        ] {
            let record = parse_filename(name).unwrap();
            assert_eq!(record.modifier, modifier);
            assert!(!record.name.is_empty());
        }
    }

}
