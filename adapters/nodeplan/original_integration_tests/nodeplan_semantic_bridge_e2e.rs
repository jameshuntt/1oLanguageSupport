#![cfg(unix)]
#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use language_server_1o::lsp::{Backend, Client};
use language_server_1o::nodeplan_semantic::{
    NodePlanSemanticProvider, NodePlanSemanticProviderConfig,
};
use language_server_1o::semantic::SemanticBridge;
use lsp_types::{
    CompletionParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
};
use nodeplan_registry_core::{RegisteredRuntime, RegistryGenerationId, WorkspaceBuildState};
use nodeplan_registry_operator::{
    read_runtime_state, read_status, wait_for_registry, AuthorityProfile, OperatorLayout,
    OperatorWorkspace,
};
use nodeplan_registry_real_service_pilot::{
    PILOT_CONSUMER_RUNTIME, PILOT_CONSUMER_SERVICE, PILOT_PROVIDER_RUNTIME,
    PILOT_PROVIDER_SERVICE, PILOT_WORKSPACE_ID,
};
use nodeplan_registry_workspace_watch::{
    canonicalize_registration, register_watched_workspace_over_uds, RegisteredWorkspaceWatcher,
    WorkspacePipelineConfig, WorkspaceSavePipeline, COMPILED_ARTIFACT_PROFILE,
};
use nodeplan_registryd::{
    LocalRegistryAuthority, NodePlanRegistryClient, NodePlanRegistryCommand,
    NodePlanRegistryCommandEnvelope, NodePlanRegistryDaemon,
};
use tempfile::TempDir;

#[test]
fn real_nodeplan_semantic_bridge_is_read_only_and_recovers_after_registry_restart(
) -> Result<(), Box<dyn std::error::Error>> {
    let project = workspace_root()?;
    let temp = TempDir::new()?;
    let operator_root = temp.path().join("operator");
    let layout = OperatorLayout::resolve(&operator_root)?;
    let mut operator_state = layout.initialize()?;
    operator_state.workspaces.insert(
        PILOT_WORKSPACE_ID.into(),
        OperatorWorkspace {
            workspace_id: PILOT_WORKSPACE_ID.into(),
            canonical_root: fs::canonicalize(&project)?,
            cargo_manifest: fs::canonicalize(project.join("Cargo.toml"))?,
            watch_enabled: false,
        },
    );
    layout.write_state(&operator_state)?;

    progress("registry-start", "start isolated registryd for existing real-service pilot");
    let mut daemon = spawn_daemon(&layout)?;
    wait_for_registry(&layout, Duration::from_secs(10))?;

    progress("real-service-generation", "build and commit existing node_daemon/type_serverd pilot generation");
    let registration = canonicalize_registration(
        PILOT_WORKSPACE_ID,
        fs::canonicalize(&project)?,
        fs::canonicalize(project.join("Cargo.toml"))?,
        COMPILED_ARTIFACT_PROFILE,
    )?;
    let config = WorkspacePipelineConfig::new(
        &layout.registry_paths().command,
        &layout.registry_paths().updates,
        &layout.watcher_state,
    );
    register_watched_workspace_over_uds(&config, registration)?;
    let pipeline = WorkspaceSavePipeline::open_registered(config, PILOT_WORKSPACE_ID)?;
    let mut watcher = RegisteredWorkspaceWatcher::open(
        pipeline,
        Duration::from_millis(120),
        Duration::from_millis(20),
    )?;
    assert!(watcher.queue_initial_if_needed()?);
    let baseline = watcher.poll_until_outcome(Duration::from_secs(900))?;
    assert_eq!(baseline.state, WorkspaceBuildState::Accepted);
    assert_eq!(baseline.generation, Some(RegistryGenerationId(1)));

    // Runtime registrations are owned by the same canonical local operator principal
    // used by the read-only operator inspection boundary below. Registering them as
    // LocalRegistryAuthority::test_authority() would create a different owner and
    // correctly trigger RuntimeOwnershipMismatch on operator reads.
    let authority = AuthorityProfile::Full.authority();
    let registry_client = NodePlanRegistryClient::new(layout.registry_paths().command);
    for (runtime_id, service_id) in [
        (PILOT_PROVIDER_RUNTIME, PILOT_PROVIDER_SERVICE),
        (PILOT_CONSUMER_RUNTIME, PILOT_CONSUMER_SERVICE),
    ] {
        let response = registry_client.send(&NodePlanRegistryCommandEnvelope {
            request_id: format!("1o-semantic-register-{runtime_id}"),
            authority: authority.clone(),
            command: NodePlanRegistryCommand::RegisterRuntime(RegisteredRuntime {
                runtime_id: runtime_id.into(),
                workspace_id: PILOT_WORKSPACE_ID.into(),
                principal_id: authority.principal_id.clone(),
                service_ids: vec![service_id.into()],
            }),
        })?;
        assert!(response.accepted, "runtime registration rejected: {:?}", response.diagnostics);
    }

    let before_status = read_status(&layout, Some(&PILOT_WORKSPACE_ID.into()))?;
    let before_operator = layout.read_state()?.unwrap_or_default();
    let before_provider_runtime = read_runtime_state(
        &layout,
        &PILOT_WORKSPACE_ID.into(),
        &PILOT_PROVIDER_RUNTIME.into(),
    )?;
    let before_consumer_runtime = read_runtime_state(
        &layout,
        &PILOT_WORKSPACE_ID.into(),
        &PILOT_CONSUMER_RUNTIME.into(),
    )?;
    assert_eq!(before_status.registry_accepted_generation, Some(RegistryGenerationId(1)));

    progress("semantic-provider", "query accepted generation through read-only operator boundary");
    let provider = Arc::new(NodePlanSemanticProvider::new(
        NodePlanSemanticProviderConfig::new(&operator_root)
            .with_query_timeout(Duration::from_secs(2))
            .with_cache_ttl(Duration::ZERO),
    ));
    let snapshot = provider.snapshot(PILOT_WORKSPACE_ID)?;
    assert_eq!(snapshot.accepted_generation, 1);
    assert!(snapshot.services.binary_search(&"type_serverd".to_string()).is_ok());
    assert!(snapshot.runtimes.binary_search(&PILOT_CONSUMER_RUNTIME.to_string()).is_ok());

    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/nodeplan_semantic_bridge_v1");
    let (sender, _receiver) = crossbeam_channel::unbounded();
    let backend = Backend::new_with_semantic_bridge(
        Client::new(sender),
        &fixture_root,
        SemanticBridge::with_provider(provider.clone()),
    );
    let service_path = fixture_root.join("v1.type_serverd.1o.service");
    let service_uri = path_uri(&service_path)?;
    let service_text = fs::read_to_string(&service_path)?;
    backend.set_open_document(&service_uri, service_text.clone(), 1);

    progress("completion", "prove schema-directed service/runtime completion from real NodePlan truth");
    let service_items = backend.completion_items(completion_at_value(
        &service_uri,
        &service_text,
        "type_serverd",
    ))?;
    assert!(service_items.iter().any(|item| item.label == "type_serverd"));
    let runtime_items = backend.completion_items(completion_at_value(
        &service_uri,
        &service_text,
        PILOT_CONSUMER_RUNTIME,
    ))?;
    assert!(runtime_items
        .iter()
        .any(|item| item.label == PILOT_CONSUMER_RUNTIME));

    progress("semantic-diagnostics", "reject missing service while accepting real service/runtime");
    let fake_text = format!(
        "{{ service {{ name \"definitely-not-a-nodeplan-service\" runtime \"{}\" }} }}",
        PILOT_CONSUMER_RUNTIME
    );
    let fake_diagnostics = backend.document_diagnostics(&service_uri, &fake_text)?;
    assert!(fake_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unknown service")));
    let real_diagnostics = backend.document_diagnostics(&service_uri, &service_text)?;
    assert!(!real_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unknown service")
            || diagnostic.message.contains("unknown runtime")));

    progress("registry-offline", "stop registryd and preserve local syntax/schema validation");
    shutdown_daemon(&layout, &authority)?;
    daemon.join().map_err(|_| "registry daemon thread panicked")?;
    wait_for_absence(&layout.registry_paths().command, Duration::from_secs(5));
    let offline_diagnostics = backend.document_diagnostics(&service_uri, &service_text)?;
    assert!(offline_diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("NodePlan semantic validation unavailable")
    }));
    let locally_invalid = format!(
        "{{ service {{ name \"type_serverd\" runtime 123 }} }}"
    );
    let local_diagnostics = backend.document_diagnostics(&service_uri, &locally_invalid)?;
    assert!(local_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("Expected string")));

    progress("registry-restart", "restart registryd and refresh semantic snapshot");
    daemon = spawn_daemon(&layout)?;
    wait_for_registry(&layout, Duration::from_secs(10))?;
    let refreshed = provider.snapshot(PILOT_WORKSPACE_ID)?;
    assert_eq!(refreshed.accepted_generation, snapshot.accepted_generation);
    assert_eq!(
        refreshed.accepted_generation_digest,
        snapshot.accepted_generation_digest
    );
    let recovered_diagnostics = backend.document_diagnostics(&service_uri, &service_text)?;
    assert!(!recovered_diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("NodePlan semantic validation unavailable")
    }));

    let after_status = read_status(&layout, Some(&PILOT_WORKSPACE_ID.into()))?;
    let after_operator = layout.read_state()?.unwrap_or_default();
    let after_provider_runtime = read_runtime_state(
        &layout,
        &PILOT_WORKSPACE_ID.into(),
        &PILOT_PROVIDER_RUNTIME.into(),
    )?;
    let after_consumer_runtime = read_runtime_state(
        &layout,
        &PILOT_WORKSPACE_ID.into(),
        &PILOT_CONSUMER_RUNTIME.into(),
    )?;

    assert_eq!(before_status.registry_accepted_generation, after_status.registry_accepted_generation);
    assert_eq!(before_status.applied_runtime_generation, after_status.applied_runtime_generation);
    assert_eq!(before_status.globally_healthy_generation, after_status.globally_healthy_generation);
    assert_eq!(before_operator.authorized_generations, after_operator.authorized_generations);
    assert_eq!(before_operator.pending_plans, after_operator.pending_plans);
    assert_eq!(before_provider_runtime, after_provider_runtime);
    assert_eq!(before_consumer_runtime, after_consumer_runtime);
    assert!(after_operator.authorized_generations.is_empty());
    assert!(after_operator.pending_plans.is_empty());

    shutdown_daemon(&layout, &authority)?;
    daemon.join().map_err(|_| "registry daemon thread panicked")?;

    println!(
        "one_o_nodeplan_semantic_bridge_summary workspace_id={} accepted_generation={} accepted_generation_digest={} service_completion=type_serverd runtime_completion={} missing_service_diagnostic=ok offline_local_validation=ok registry_restart_recovery=ok generation_unchanged=true runtime_mutation=false explicit_adoption_invoked=false ok",
        PILOT_WORKSPACE_ID,
        snapshot.accepted_generation,
        snapshot.accepted_generation_digest,
        PILOT_CONSUMER_RUNTIME,
    );
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| "unable to resolve NodePlan workspace root".into())
}

fn spawn_daemon(
    layout: &OperatorLayout,
) -> Result<thread::JoinHandle<()>, Box<dyn std::error::Error>> {
    let state_root = layout.registry_state.clone();
    let socket_dir = layout.socket_dir.clone();
    Ok(thread::spawn(move || {
        if let Err(error) = NodePlanRegistryDaemon::open(state_root, socket_dir)
            .and_then(NodePlanRegistryDaemon::run)
        {
            eprintln!("[1o-nodeplan-e2e] registry daemon exited with error: {error}");
        }
    }))
}

fn shutdown_daemon(
    layout: &OperatorLayout,
    authority: &LocalRegistryAuthority,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = NodePlanRegistryClient::new(layout.registry_paths().command);
    let response = client.send(&NodePlanRegistryCommandEnvelope {
        request_id: "1o-semantic-shutdown".into(),
        authority: authority.clone(),
        command: NodePlanRegistryCommand::Shutdown,
    })?;
    if !response.accepted {
        return Err(format!("registry shutdown rejected: {:?}", response.diagnostics).into());
    }
    Ok(())
}

fn wait_for_socket(path: &Path, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!("timed out waiting for {}", path.display()).into())
}

fn wait_for_absence(path: &Path, timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn path_uri(path: &Path) -> Result<Uri, Box<dyn std::error::Error>> {
    Ok(Uri::from_str(&format!("file://{}", path.display()))?)
}

fn completion_at_value(uri: &Uri, text: &str, value: &str) -> CompletionParams {
    let offset = text.find(value).expect("fixture completion value exists") + 1;
    CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: position_for_offset(text, offset),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    }
}

fn position_for_offset(text: &str, offset: usize) -> Position {
    let prefix = &text[..offset.min(text.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail.len())
        .unwrap_or(prefix.len()) as u32;
    Position { line, character }
}

fn progress(phase: &str, detail: &str) {
    eprintln!("[1o-nodeplan-e2e] phase={phase} {detail}");
}
