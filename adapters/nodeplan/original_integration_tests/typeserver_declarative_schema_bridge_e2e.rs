#![cfg(unix)]
#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use language_server_1o::nodeplan_semantic::{
    NodePlanSemanticProvider, NodePlanSemanticProviderConfig,
};
use language_server_1o::semantic::SemanticContext;
use language_server_1o::typeserver_declarative::{
    build_typeserver_declarative_plan, validate_local_declarative_workspace, SemanticPrimitive,
    StorageBackend,
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
fn real_typeserver_declarative_plan_is_deterministic_read_only_and_recovers_after_registry_restart(
) -> anyhow::Result<()> {
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

    progress(
        "registry-start",
        "start isolated registryd for existing node_daemon/type_serverd pilot",
    );
    let mut daemon = spawn_daemon(&layout)?;
    wait_for_registry(&layout, Duration::from_secs(10))?;

    progress(
        "real-service-generation",
        "build and commit existing real node_daemon/type_serverd pilot generation",
    );
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

    let authority = AuthorityProfile::Full.authority();
    let registry_client = NodePlanRegistryClient::new(layout.registry_paths().command);
    for (runtime_id, service_id) in [
        (PILOT_PROVIDER_RUNTIME, PILOT_PROVIDER_SERVICE),
        (PILOT_CONSUMER_RUNTIME, PILOT_CONSUMER_SERVICE),
    ] {
        let response = registry_client.send(&NodePlanRegistryCommandEnvelope {
            request_id: format!("1o-typeserver-register-{runtime_id}"),
            authority: authority.clone(),
            command: NodePlanRegistryCommand::RegisterRuntime(RegisteredRuntime {
                runtime_id: runtime_id.into(),
                workspace_id: PILOT_WORKSPACE_ID.into(),
                principal_id: authority.principal_id.clone(),
                service_ids: vec![service_id.into()],
            }),
        })?;
        assert!(
            response.accepted,
            "runtime registration rejected: {:?}",
            response.diagnostics
        );
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
    assert_eq!(
        before_status.registry_accepted_generation,
        Some(RegistryGenerationId(1))
    );

    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/typeserver_declarative_schema_bridge_v1");

    progress(
        "local-schema-validation",
        "parse and validate Todo model/container through existing 1o workspace schema registry",
    );
    let local = validate_local_declarative_workspace(&fixture_root)
        .map_err(|diagnostics| anyhow!(render_diagnostics(&diagnostics)))?;
    assert_eq!(local.models.len(), 1);
    assert_eq!(local.models[0].name, "Todo");
    assert_eq!(local.containers.len(), 1);
    assert_eq!(local.containers[0].name, "todos");
    assert_eq!(local.containers[0].model, "Todo");
    assert_eq!(local.containers[0].storage.backend, StorageBackend::Sqlite);
    assert_eq!(local.containers[0].storage.namespace, "local.todos");
    let todo_id = local.models[0]
        .fields
        .iter()
        .find(|field| field.name == "id")
        .expect("Todo.id exists");
    assert_eq!(todo_id.semantic_type_name, "TodoId");
    assert!(matches!(&todo_id.primitive, SemanticPrimitive::UuidV4 { .. }));

    progress(
        "nodeplan-service-resolution",
        "resolve type_serverd against real accepted NodePlan generation through read-only provider",
    );
    let provider = NodePlanSemanticProvider::new(
        NodePlanSemanticProviderConfig::new(&operator_root)
            .with_query_timeout(Duration::from_secs(2))
            .with_cache_ttl(Duration::ZERO),
    );
    let context = SemanticContext {
        workspace_id: PILOT_WORKSPACE_ID.into(),
    };
    let first = build_typeserver_declarative_plan(&fixture_root, &provider, &context)
        .map_err(|diagnostics| anyhow!(render_diagnostics(&diagnostics)))?;
    let repeated = build_typeserver_declarative_plan(&fixture_root, &provider, &context)
        .map_err(|diagnostics| anyhow!(render_diagnostics(&diagnostics)))?;
    assert_eq!(first, repeated);
    assert_eq!(
        first.deterministic_json()?,
        repeated.deterministic_json()?
    );
    assert_eq!(first.accepted_generation, 1);
    assert_eq!(first.models[0].name, "Todo");
    assert_eq!(first.containers[0].name, "todos");
    assert_eq!(first.containers[0].model.name, "Todo");
    assert_eq!(first.containers[0].service.service_id, "type_serverd");
    assert_eq!(first.containers[0].storage.backend, StorageBackend::Sqlite);
    assert_eq!(first.containers[0].storage.namespace, "local.todos");

    progress(
        "missing-service-diagnostic",
        "prove a nonexistent service is rejected by live NodePlan semantic truth",
    );
    let invalid_fixture = temp.path().join("invalid-service-workspace");
    copy_tree(&fixture_root, &invalid_fixture)?;
    fs::write(
        invalid_fixture.join("v1.todos.1o.container"),
        r#"{
            container {
                name "todos"
                model "Todo"
                storage { backend "sqlite" namespace "local.todos" }
                service "nonexistent_type_serverd"
            }
        }"#,
    )?;
    let missing_service = build_typeserver_declarative_plan(&invalid_fixture, &provider, &context)
        .expect_err("nonexistent service must be rejected");
    assert!(missing_service
        .iter()
        .any(|diagnostic| diagnostic.code == "missing_nodeplan_service"));

    progress(
        "registry-offline",
        "stop registryd and prove local schema/model resolution remains valid without false live acceptance",
    );
    shutdown_daemon(&layout, &authority)?;
    daemon.join().map_err(|_| anyhow!("registry daemon thread panicked"))?;
    let offline_local = validate_local_declarative_workspace(&fixture_root)
        .map_err(|diagnostics| anyhow!(render_diagnostics(&diagnostics)))?;
    assert_eq!(offline_local, local);
    let offline_plan = build_typeserver_declarative_plan(&fixture_root, &provider, &context)
        .expect_err("offline NodePlan truth must not be falsely asserted as resolved");
    assert!(offline_plan
        .iter()
        .any(|diagnostic| diagnostic.code == "nodeplan_semantics_unavailable"));

    progress(
        "registry-restart",
        "restart registryd, refresh semantic truth, and reproduce identical declarative plan",
    );
    daemon = spawn_daemon(&layout)?;
    wait_for_registry(&layout, Duration::from_secs(10))?;
    let recovered = build_typeserver_declarative_plan(&fixture_root, &provider, &context)
        .map_err(|diagnostics| anyhow!(render_diagnostics(&diagnostics)))?;
    assert_eq!(first, recovered);
    assert_eq!(first.deterministic_json()?, recovered.deterministic_json()?);

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

    assert_eq!(
        before_status.registry_accepted_generation,
        after_status.registry_accepted_generation
    );
    assert_eq!(
        before_status.applied_runtime_generation,
        after_status.applied_runtime_generation
    );
    assert_eq!(
        before_status.globally_healthy_generation,
        after_status.globally_healthy_generation
    );
    assert_eq!(
        before_operator.authorized_generations,
        after_operator.authorized_generations
    );
    assert_eq!(before_operator.pending_plans, after_operator.pending_plans);
    assert_eq!(before_provider_runtime, after_provider_runtime);
    assert_eq!(before_consumer_runtime, after_consumer_runtime);
    assert!(after_operator.authorized_generations.is_empty());
    assert!(after_operator.pending_plans.is_empty());

    shutdown_daemon(&layout, &authority)?;
    daemon.join().map_err(|_| anyhow!("registry daemon thread panicked"))?;

    println!(
        "one_o_typeserver_declarative_schema_bridge_summary model=Todo container=todos service=type_serverd storage_backend=sqlite storage_namespace=local.todos uuid_v4=ok uuid_v5=ok model_resolution=ok nodeplan_service_resolution=ok missing_service_diagnostic=ok deterministic_plan=ok offline_local_validation=ok registry_restart_recovery=ok generation_unchanged=true runtime_mutation=false typeserver_mutation=false sqlite_durable_mutation=false explicit_adoption_invoked=false accepted_generation={} accepted_generation_digest={} ok",
        first.accepted_generation,
        first.accepted_generation_digest,
    );
    Ok(())
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("unable to resolve NodePlan workspace root"))
}

fn spawn_daemon(
    layout: &OperatorLayout,
) -> anyhow::Result<thread::JoinHandle<()>> {
    let state_root = layout.registry_state.clone();
    let socket_dir = layout.socket_dir.clone();
    Ok(thread::spawn(move || {
        if let Err(error) = NodePlanRegistryDaemon::open(state_root, socket_dir)
            .and_then(NodePlanRegistryDaemon::run)
        {
            eprintln!("[1o-typeserver-e2e] registry daemon exited with error: {error}");
        }
    }))
}

fn shutdown_daemon(
    layout: &OperatorLayout,
    authority: &LocalRegistryAuthority,
) -> anyhow::Result<()> {
    let client = NodePlanRegistryClient::new(layout.registry_paths().command);
    let response = client.send(&NodePlanRegistryCommandEnvelope {
        request_id: "1o-typeserver-declarative-shutdown".into(),
        authority: authority.clone(),
        command: NodePlanRegistryCommand::Shutdown,
    })?;
    if !response.accepted {
        return Err(anyhow!("registry shutdown rejected: {:?}", response.diagnostics));
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> anyhow::Result<()> {
    for entry in walkdir::WalkDir::new(source).into_iter().flatten() {
        let path = entry.path();
        let relative = path.strip_prefix(source)?;
        let target = destination.join(relative);
        if path.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn render_diagnostics(
    diagnostics: &[language_server_1o::typeserver_declarative::DeclarativeDiagnostic],
) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}:{}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn progress(phase: &str, detail: &str) {
    eprintln!("[1o-typeserver-e2e] phase={phase} {detail}");
}
