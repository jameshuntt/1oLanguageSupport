use serde_json::Value;

use crate::engine::{
    json_schema_validator::validate_ast, EngineDiagnostic, Lexer, Parser, Span, Workspace,
};

/// Legacy helper retained for callers outside the workspace-aware LSP path.
/// Built-in schema parsing is fallible and never panics on empty/malformed JSON.
pub fn validate_text(modifier: &str, text: &str) -> Result<(), Vec<EngineDiagnostic>> {
    let tokens = Lexer::new(text).tokenize();
    let ast = Parser::new(tokens)
        .parse()
        .map_err(|error| vec![EngineDiagnostic::from(error)])?;
    let Some(schema) = builtin_schema_for_modifier(modifier) else {
        return Ok(());
    };
    let mut diagnostics = Vec::new();
    validate_ast(&ast, &schema, &mut diagnostics);
    if diagnostics.is_empty() { Ok(()) } else { Err(diagnostics) }
}

pub fn builtin_schema_for_modifier(modifier: &str) -> Option<Value> {
    let raw = match modifier {
        "container" => include_str!("../schema_definitions/container/v1.schema.json"),
        "model" => include_str!("../schema_definitions/model/v1.schema.json"),
        "slice" => include_str!("../schema_definitions/slice/v1.schema.json"),
        "walkthrough" => include_str!("../schema_definitions/walkthrough/v1.schema.json"),
        "page" => include_str!("../schema_definitions/page/v1.schema.json"),
        "section" => include_str!("../schema_definitions/section/v1.schema.json"),
        "overlay_zone" => include_str!("../schema_definitions/overlay_zone/v1.schema.json"),
        "component" => include_str!("../schema_definitions/component/v1.schema.json"),
        "resource" => include_str!("../schema_definitions/resource/v1.schema.json"),
        "theme" => include_str!("../schema_definitions/theme/v1.schema.json"),
        _ => return None,
    };
    match serde_json::from_str(raw) {
        Ok(schema) => Some(schema),
        Err(error) => {
            eprintln!("[1o-lsp] built-in schema '{modifier}' is malformed: {error}");
            None
        }
    }
}

pub fn diagnostics_for_text_workspace(
    workspace: &Workspace,
    modifier: &str,
    file_version: &str,
    text: &str,
) -> Vec<EngineDiagnostic> {
    let tokens = Lexer::new(text).tokenize();
    let ast = match Parser::new(tokens).parse() {
        Ok(ast) => ast,
        Err(error) => return vec![EngineDiagnostic::from(error)],
    };

    let schema_version = workspace
        .version_sheet
        .schema_version_for(file_version, modifier);
    let schema = workspace
        .schema_registry
        .get(modifier, &schema_version)
        .cloned()
        .or_else(|| builtin_schema_for_modifier(modifier));
    let Some(schema) = schema else {
        let issue = workspace
            .schema_registry
            .issues()
            .first()
            .map(|issue| format!("; {}", issue.message))
            .unwrap_or_default();
        return vec![EngineDiagnostic::warning(
            format!(
                "Schema unavailable for modifier='{modifier}', file_version='{file_version}', schema_version='{schema_version}'{issue}"
            ),
            Span { start: 0, end: 0 },
        )];
    };

    let mut diagnostics = Vec::new();
    validate_ast(&ast, &schema, &mut diagnostics);
    diagnostics
}

pub fn validate_text_workspace(
    workspace: &Workspace,
    modifier: &str,
    file_version: &str,
    text: &str,
) -> Result<(), Vec<EngineDiagnostic>> {
    let diagnostics = diagnostics_for_text_workspace(workspace, modifier, file_version, text);
    if diagnostics.is_empty() { Ok(()) } else { Err(diagnostics) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn unknown_modifier_produces_warning_instead_of_panic() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path());
        let diagnostics = diagnostics_for_text_workspace(
            &workspace,
            "__missing_schema_test__",
            "v1",
            "{ __missing_schema_test__ { name \"x\" } }",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(matches!(diagnostics[0].severity, crate::engine::EngineSeverity::Warning));
    }

    #[test]
    fn workspace_schema_rejects_non_string_semantic_field_locally() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("schema/service")).unwrap();
        fs::write(temp.path().join("schema/1o.version.json"), r#"{"versions":{"v1":{"service":"v1"}}}"#).unwrap();
        fs::write(
            temp.path().join("schema/service/v1.schema.json"),
            r#"{"type":"object","properties":{"service":{"type":"object","properties":{"runtime":{"type":"string"}},"required":["runtime"]}},"required":["service"]}"#,
        ).unwrap();
        let workspace = Workspace::new(temp.path());
        let diagnostics = diagnostics_for_text_workspace(
            &workspace,
            "service",
            "v1",
            "{ service { runtime 123 } }",
        );
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("Expected string")));
    }

    #[test]
    fn walkthrough_catalog_files_validate_against_builtin_schema() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/walkthroughs").join("archive_manager");
        let mut files = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.file_name().and_then(|value| value.to_str()).map(|name| name.ends_with(".1o.walkthrough")).unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();
        assert!(!files.is_empty());
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap();
            validate_text("walkthrough", &source)
                .unwrap_or_else(|diagnostics| panic!("{} failed walkthrough schema validation: {diagnostics:?}", path.display()));
        }
    }

    #[test]
    fn walkthrough_schema_rejects_unknown_event_kind() {
        let source = r#"{
            walkthrough {
                id "archive.invalid"
                title "Invalid"
                description "Invalid event kind fixture"
                environment "IsolatedDemo"
                start_requirement "ExistingOrganization"
                fixture_requirement "None"
                estimated_presentation_ms 1
                destructive false
                safe_demo_only true
                event { kind "PixelClick" }
            }
        }"#;
        let diagnostics = validate_text("walkthrough", source).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("allowed enum values")));
    }


    #[test]
    fn ui_composition_files_validate_against_builtin_schemas() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/ui").join("archive_manager");
        let mut files = std::fs::read_dir(&root).unwrap().filter_map(Result::ok).map(|entry| entry.path()).collect::<Vec<_>>();
        files.sort();
        assert!(!files.is_empty());
        for path in files {
            if !path.is_file() { continue; }
            let name = path.file_name().and_then(|v| v.to_str()).unwrap();
            let record = crate::utils::parse_filename(name).unwrap_or_else(|| panic!("invalid 1o UI composition filename {name}"));
            assert!(matches!(record.modifier.as_str(), "page" | "section" | "overlay_zone" | "component" | "resource" | "theme"));
            let source = std::fs::read_to_string(&path).unwrap();
            validate_text(&record.modifier, &source).unwrap_or_else(|diagnostics| panic!("{} failed {} schema validation: {diagnostics:?}", path.display(), record.modifier));
        }
    }

    #[test]
    fn ui_component_schema_rejects_unknown_prop_mode() {
        let source = r#"{ component { id "x" prop { name "route" type "route" mode "magical" } } }"#;
        let diagnostics = validate_text("component", source).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("allowed enum values")));
    }

    #[test]
    fn overlay_zone_schema_rejects_unknown_placement() {
        let source = r#"{ overlay_zone { id "x" title "x" wide_placement "floating_magic" narrow_placement "bottom" breakpoint_width 900 } }"#;
        let diagnostics = validate_text("overlay_zone", source).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("allowed enum values")));
    }

    #[test]
    fn builtin_ui_schema_registry_covers_every_artifact_role() {
        for modifier in ["page", "section", "overlay_zone", "component", "resource", "theme"] {
            assert!(builtin_schema_for_modifier(modifier).is_some(), "missing builtin schema for {modifier}");
        }
    }

    #[test]
    fn ui_page_schema_rejects_missing_route() {
        let diagnostics = validate_text("page", r#"{ page { id "x" title "X" } }"#).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("route") || diagnostic.message.contains("required")));
    }

    #[test]
    fn ui_section_schema_rejects_unknown_property() {
        let diagnostics = validate_text("section", r#"{ section { id "x" title "X" magical_authority true } }"#).unwrap_err();
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn ui_resource_schema_rejects_non_string_content() {
        let diagnostics = validate_text("resource", r#"{ resource { id "x" value { key "spacing" content 12 } } }"#).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("string")));
    }

    #[test]
    fn ui_theme_schema_requires_token() {
        let diagnostics = validate_text("theme", r#"{ theme { id "x" } }"#).unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("token") || diagnostic.message.contains("required")));
    }

}
