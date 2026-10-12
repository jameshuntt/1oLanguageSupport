#![forbid(unsafe_code)]

use std::path::PathBuf;

use language_server_1o::engine::{diagnostics_for_text_workspace, Workspace};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/connection_traits_remote_runtime_lan_harness_v1")
}

#[test]
fn connection_profile_schema_lints_transport_trust_and_policy_values() {
    let root = fixture_root();
    let workspace = Workspace::new(&root);
    let valid = std::fs::read_to_string(root.join("v1.remote_lan.1o.connection")).unwrap();
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &valid);
    assert!(diagnostics.is_empty(), "valid connection profile diagnostics: {diagnostics:#?}");

    let invalid_adapter = valid.replace("adapter \"rustls_tcp\"", "adapter \"magic_lan\"");
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &invalid_adapter);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("allowed enum values")));

    let invalid_trust = valid.replace("mode \"pinned_certificate\"", "mode \"trust_on_first_use\"");
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &invalid_trust);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("allowed enum values")));

    let invalid_frame_type = valid.replace("max_frame_bytes 1048576", "max_frame_bytes \"unbounded\"");
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &invalid_frame_type);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("Expected integer")));

    let unknown_key = valid.replace("acceptance_profile \"second_physical_machine_lan\"", "acceptance_profile \"second_physical_machine_lan\"\n        magical_socket true");
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &unknown_key);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("Unknown key 'magical_socket'")));
}

#[test]
fn thunderbolt_connection_profile_lints_adapter_and_link_shape() {
    let root = fixture_root();
    let workspace = Workspace::new(&root);
    let valid = std::fs::read_to_string(root.join("v1.remote_thunderbolt.1o.connection")).unwrap();
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &valid);
    assert!(diagnostics.is_empty(), "valid Thunderbolt profile diagnostics: {diagnostics:#?}");

    let invalid_adapter = valid.replace(
        "adapter \"thunderbolt_bridge_rustls_tcp\"",
        "adapter \"thunderbolt_magic\"",
    );
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &invalid_adapter);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("allowed enum values")));

    let invalid_local_ip_type = valid.replace("local_ip \"10.77.0.1\"", "local_ip 107701");
    let diagnostics = diagnostics_for_text_workspace(&workspace, "connection", "v1", &invalid_local_ip_type);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("Expected string")));
}
