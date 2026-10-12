//! Read-only NodePlan semantic provider for 1o.
//!
//! This module deliberately imports only NodePlan operator inspection functions.
//! It cannot submit artifacts, adopt generations, restart runtimes, or mutate
//! registry/operator state.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};
use nodeplan_registry_core::WorkspaceId;
use nodeplan_registry_operator::{
    read_generation, read_runtime_registrations, read_status, OperatorLayout,
};

use crate::semantic::{
    SemanticContext, SemanticProvider, SemanticProviderError, SemanticSymbol,
    SemanticValidation,
};

pub const NODEPLAN_SERVICE_REF: &str = "nodeplan.service";
pub const NODEPLAN_RUNTIME_REF: &str = "nodeplan.runtime";
pub const NODEPLAN_CONNECTION_REF: &str = "nodeplan.connection";
pub const NODEPLAN_HANDLER_REF: &str = "nodeplan.handler";
pub const NODEPLAN_FUNCTION_REF: &str = "nodeplan.function";
pub const NODEPLAN_CAPABILITY_REF: &str = "nodeplan.capability";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePlanSemanticSnapshot {
    pub workspace_id: String,
    pub accepted_generation: u64,
    pub accepted_generation_digest: String,
    pub services: Vec<String>,
    pub runtimes: Vec<String>,
    pub connections: Vec<String>,
    pub handlers: Vec<String>,
    pub functions: Vec<String>,
    pub capabilities: Vec<String>,
    /// Per-service function inventories (service id → sorted function names)
    /// — the precise surface behind scoped validation: a function paired with
    /// the WRONG service is refused even when the name exists elsewhere. The
    /// flat `functions` list stays for unscoped references and completion.
    pub functions_by_service: Vec<(String, Vec<String>)>,
}

impl NodePlanSemanticSnapshot {
    fn values_for(&self, semantic_ref: &str) -> Option<&[String]> {
        match semantic_ref {
            NODEPLAN_SERVICE_REF => Some(&self.services),
            NODEPLAN_RUNTIME_REF => Some(&self.runtimes),
            NODEPLAN_CONNECTION_REF => Some(&self.connections),
            NODEPLAN_HANDLER_REF => Some(&self.handlers),
            NODEPLAN_FUNCTION_REF => Some(&self.functions),
            NODEPLAN_CAPABILITY_REF => Some(&self.capabilities),
            _ => None,
        }
    }

    fn identity(&self) -> (&str, u64, &str) {
        (
            &self.workspace_id,
            self.accepted_generation,
            &self.accepted_generation_digest,
        )
    }

    /// The function inventory of one service, when the service is known.
    fn functions_of(&self, service: &str) -> Option<&[String]> {
        self.functions_by_service
            .binary_search_by(|(known, _)| known.as_str().cmp(service))
            .ok()
            .map(|index| self.functions_by_service[index].1.as_slice())
    }
}

#[derive(Debug, Clone)]
pub struct NodePlanSemanticProviderConfig {
    pub operator_root: PathBuf,
    pub query_timeout: Duration,
    pub cache_ttl: Duration,
}

impl NodePlanSemanticProviderConfig {
    pub fn new(operator_root: impl Into<PathBuf>) -> Self {
        Self {
            operator_root: operator_root.into(),
            query_timeout: Duration::from_millis(750),
            cache_ttl: Duration::from_millis(500),
        }
    }

    pub fn from_environment() -> Result<Self, SemanticProviderError> {
        let value = std::env::var_os("NODEPLAN_1O_OPERATOR_ROOT")
            .or_else(|| std::env::var_os("NODEPLAN_OPERATOR_ROOT"))
            .ok_or_else(|| {
                SemanticProviderError::InvalidConfiguration(
                    "set NODEPLAN_1O_OPERATOR_ROOT (or NODEPLAN_OPERATOR_ROOT) to the same operator root used by NodePlan tooling"
                        .into(),
                )
            })?;
        Ok(Self::new(PathBuf::from(value)))
    }

    pub fn with_query_timeout(mut self, timeout: Duration) -> Self {
        self.query_timeout = timeout;
        self
    }

    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SnapshotIdentity {
    workspace_id: String,
    accepted_generation: u64,
    accepted_generation_digest: String,
}

impl From<&NodePlanSemanticSnapshot> for SnapshotIdentity {
    fn from(snapshot: &NodePlanSemanticSnapshot) -> Self {
        Self {
            workspace_id: snapshot.workspace_id.clone(),
            accepted_generation: snapshot.accepted_generation,
            accepted_generation_digest: snapshot.accepted_generation_digest.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct CachedSnapshot {
    snapshot: NodePlanSemanticSnapshot,
    checked_at: Instant,
}

/// The cache identity is explicitly `(workspace_id, accepted_generation,
/// accepted_generation_digest)`. Only the latest identity per workspace is
/// retained, so a new accepted generation/digest invalidates the prior entry.
#[derive(Debug, Default)]
struct SnapshotCache {
    latest_by_workspace: HashMap<String, SnapshotIdentity>,
    entries: HashMap<SnapshotIdentity, CachedSnapshot>,
}

impl SnapshotCache {
    fn fresh(&self, workspace_id: &str, ttl: Duration) -> Option<NodePlanSemanticSnapshot> {
        let identity = self.latest_by_workspace.get(workspace_id)?;
        let cached = self.entries.get(identity)?;
        if cached.checked_at.elapsed() <= ttl {
            Some(cached.snapshot.clone())
        } else {
            None
        }
    }

    fn store(&mut self, snapshot: NodePlanSemanticSnapshot) {
        let identity = SnapshotIdentity::from(&snapshot);
        if let Some(prior) = self
            .latest_by_workspace
            .insert(snapshot.workspace_id.clone(), identity.clone())
        {
            if prior != identity {
                self.entries.remove(&prior);
            }
        }
        self.entries.insert(
            identity,
            CachedSnapshot {
                snapshot,
                checked_at: Instant::now(),
            },
        );
    }

    #[cfg(test)]
    fn identity(&self, workspace_id: &str) -> Option<(u64, String)> {
        self.latest_by_workspace.get(workspace_id).map(|identity| {
            (
                identity.accepted_generation,
                identity.accepted_generation_digest.clone(),
            )
        })
    }
}

struct QueryRequest {
    workspace_id: String,
    reply: Sender<Result<NodePlanSemanticSnapshot, SemanticProviderError>>,
}

pub struct NodePlanSemanticProvider {
    query_tx: Sender<QueryRequest>,
    query_timeout: Duration,
    cache_ttl: Duration,
    cache: Arc<Mutex<SnapshotCache>>,
}

impl NodePlanSemanticProvider {
    pub fn new(config: NodePlanSemanticProviderConfig) -> Self {
        let (query_tx, query_rx) = bounded::<QueryRequest>(1);
        let operator_root = config.operator_root.clone();
        thread::Builder::new()
            .name("1o-nodeplan-semantic-readonly".into())
            .spawn(move || query_worker(query_rx, operator_root))
            .expect("spawn bounded NodePlan semantic query worker");
        Self {
            query_tx,
            query_timeout: config.query_timeout,
            cache_ttl: config.cache_ttl,
            cache: Arc::new(Mutex::new(SnapshotCache::default())),
        }
    }

    pub fn from_environment() -> Result<Self, SemanticProviderError> {
        Ok(Self::new(NodePlanSemanticProviderConfig::from_environment()?))
    }

    pub fn snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<NodePlanSemanticSnapshot, SemanticProviderError> {
        if workspace_id.trim().is_empty() {
            return Err(SemanticProviderError::InvalidConfiguration(
                "NodePlan workspace_id is empty".into(),
            ));
        }
        if let Some(snapshot) = self
            .cache
            .lock()
            .expect("semantic cache lock")
            .fresh(workspace_id, self.cache_ttl)
        {
            return Ok(snapshot);
        }

        let (reply_tx, reply_rx) = bounded(1);
        self.query_tx
            .send_timeout(
                QueryRequest {
                    workspace_id: workspace_id.to_string(),
                    reply: reply_tx,
                },
                self.query_timeout,
            )
            .map_err(|_| {
                SemanticProviderError::Timeout(
                    "NodePlan semantic query worker did not accept a bounded request".into(),
                )
            })?;

        let snapshot = reply_rx.recv_timeout(self.query_timeout).map_err(|_| {
            SemanticProviderError::Timeout(
                "NodePlan semantic query exceeded the configured timeout".into(),
            )
        })??;

        self.cache
            .lock()
            .expect("semantic cache lock")
            .store(snapshot.clone());
        Ok(snapshot)
    }
}

impl SemanticProvider for NodePlanSemanticProvider {
    fn complete(
        &self,
        semantic_ref: &str,
        context: &SemanticContext,
    ) -> Result<Vec<SemanticSymbol>, SemanticProviderError> {
        let snapshot = self.snapshot(&context.workspace_id)?;
        let values = snapshot.values_for(semantic_ref).ok_or_else(|| {
            SemanticProviderError::UnsupportedSemanticRef(semantic_ref.to_string())
        })?;
        Ok(values
            .iter()
            .cloned()
            .map(|value| SemanticSymbol {
                value,
                detail: format!(
                    "NodePlan accepted generation {} ({})",
                    snapshot.accepted_generation, snapshot.accepted_generation_digest
                ),
            })
            .collect())
    }

    fn validate(
        &self,
        semantic_ref: &str,
        value: &str,
        context: &SemanticContext,
    ) -> Result<SemanticValidation, SemanticProviderError> {
        let snapshot = self.snapshot(&context.workspace_id)?;
        let values = snapshot.values_for(semantic_ref).ok_or_else(|| {
            SemanticProviderError::UnsupportedSemanticRef(semantic_ref.to_string())
        })?;
        let identity = (
            snapshot.accepted_generation,
            snapshot.accepted_generation_digest.clone(),
        );
        if values.binary_search_by(|known| known.as_str().cmp(value)).is_ok() {
            Ok(SemanticValidation::Exists {
                accepted_generation: identity.0,
                accepted_generation_digest: identity.1,
            })
        } else {
            Ok(SemanticValidation::Missing {
                accepted_generation: identity.0,
                accepted_generation_digest: identity.1,
            })
        }
    }

    /// Scoped validation: a `nodeplan.function` reference inside a block that
    /// names a `nodeplan.service` validates against THAT service's function
    /// inventory. An unknown scope service falls back to the flat check (the
    /// service reference itself already errors separately — one wrong name
    /// should not cascade into two diagnostics).
    fn validate_scoped(
        &self,
        semantic_ref: &str,
        value: &str,
        scope: Option<(&str, &str)>,
        context: &SemanticContext,
    ) -> Result<SemanticValidation, SemanticProviderError> {
        let Some((NODEPLAN_SERVICE_REF, service)) = scope else {
            return self.validate(semantic_ref, value, context);
        };
        if semantic_ref != NODEPLAN_FUNCTION_REF {
            return self.validate(semantic_ref, value, context);
        }
        let snapshot = self.snapshot(&context.workspace_id)?;
        let Some(functions) = snapshot.functions_of(service) else {
            return self.validate(semantic_ref, value, context);
        };
        let identity = (
            snapshot.accepted_generation,
            snapshot.accepted_generation_digest.clone(),
        );
        if functions.binary_search_by(|known| known.as_str().cmp(value)).is_ok() {
            Ok(SemanticValidation::Exists {
                accepted_generation: identity.0,
                accepted_generation_digest: identity.1,
            })
        } else {
            Ok(SemanticValidation::Missing {
                accepted_generation: identity.0,
                accepted_generation_digest: identity.1,
            })
        }
    }

    fn refresh_identity(
        &self,
        context: &SemanticContext,
    ) -> Result<Option<String>, SemanticProviderError> {
        let snapshot = self.snapshot(&context.workspace_id)?;
        Ok(Some(format!(
            "{}:{}",
            snapshot.accepted_generation, snapshot.accepted_generation_digest
        )))
    }
}

fn query_worker(query_rx: Receiver<QueryRequest>, operator_root: PathBuf) {
    while let Ok(request) = query_rx.recv() {
        let result = read_snapshot(&operator_root, &request.workspace_id);
        let _ = request.reply.send(result);
    }
}

fn read_snapshot(
    operator_root: &PathBuf,
    workspace_id: &str,
) -> Result<NodePlanSemanticSnapshot, SemanticProviderError> {
    let layout = OperatorLayout::resolve(operator_root).map_err(|error| {
        SemanticProviderError::Unavailable(format!("operator layout unavailable: {error}"))
    })?;
    let workspace_id_typed = WorkspaceId::from(workspace_id.to_string());
    let status = read_status(&layout, Some(&workspace_id_typed)).map_err(|error| {
        SemanticProviderError::Unavailable(format!("registry status unavailable: {error}"))
    })?;
    let accepted_generation = status.registry_accepted_generation.ok_or_else(|| {
        SemanticProviderError::Unavailable(format!(
            "workspace '{workspace_id}' has no accepted registry generation"
        ))
    })?;
    let generation = read_generation(&layout, &workspace_id_typed, accepted_generation)
        .map_err(|error| {
            SemanticProviderError::Unavailable(format!("accepted generation unavailable: {error}"))
        })?;
    let runtimes = read_runtime_registrations(&layout, &workspace_id_typed).map_err(|error| {
        SemanticProviderError::Unavailable(format!("runtime registrations unavailable: {error}"))
    })?;

    let mut services = BTreeSet::new();
    let mut handlers = BTreeSet::new();
    let mut functions = BTreeSet::new();
    let mut capabilities = BTreeSet::new();
    let mut functions_by_service: std::collections::BTreeMap<String, BTreeSet<String>> =
        std::collections::BTreeMap::new();
    for service in &generation.services {
        services.insert(service.service_id.to_string());
        handlers.extend(service.contract_handlers.iter().map(ToString::to_string));
        handlers.extend(
            service
                .generated_handlers
                .iter()
                .map(|handler| handler.handler_id.to_string()),
        );
        let own = functions_by_service
            .entry(service.service_id.to_string())
            .or_default();
        own.extend(service.contract_functions.iter().cloned());
        own.extend(
            service
                .generated_functions
                .iter()
                .map(|function| function.name.clone()),
        );
        functions.extend(own.iter().cloned());
        capabilities.extend(service.iam_requirements.iter().cloned());
    }

    let runtime_values = runtimes
        .into_iter()
        .map(|runtime| runtime.runtime_id.to_string())
        .collect::<BTreeSet<_>>();
    let connection_values = generation
        .resolved_connections
        .iter()
        .map(|connection| connection.connection_id.to_string())
        .collect::<BTreeSet<_>>();

    Ok(NodePlanSemanticSnapshot {
        workspace_id: workspace_id.to_string(),
        accepted_generation: generation.generation.0,
        accepted_generation_digest: generation.artifact_digest.to_string(),
        services: services.into_iter().collect(),
        runtimes: runtime_values.into_iter().collect(),
        connections: connection_values.into_iter().collect(),
        handlers: handlers.into_iter().collect(),
        functions: functions.into_iter().collect(),
        capabilities: capabilities.into_iter().collect(),
        functions_by_service: functions_by_service
            .into_iter()
            .map(|(service, functions)| (service, functions.into_iter().collect()))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(generation: u64, digest: &str) -> NodePlanSemanticSnapshot {
        NodePlanSemanticSnapshot {
            workspace_id: "w".into(),
            accepted_generation: generation,
            accepted_generation_digest: digest.into(),
            services: vec![],
            runtimes: vec![],
            connections: vec![],
            handlers: vec![],
            functions: vec![],
            capabilities: vec![],
            functions_by_service: vec![],
        }
    }

    #[test]
    fn cache_identity_is_replaced_when_generation_changes() {
        let mut cache = SnapshotCache::default();
        cache.store(snapshot(1, "digest-1"));
        assert_eq!(cache.identity("w"), Some((1, "digest-1".into())));
        cache.store(snapshot(2, "digest-2"));
        assert_eq!(cache.identity("w"), Some((2, "digest-2".into())));
    }

    #[test]
    fn snapshot_values_are_sorted_for_binary_search_validation() {
        let mut value = snapshot(1, "digest");
        value.services = vec!["a".into(), "b".into()];
        assert!(value
            .values_for(NODEPLAN_SERVICE_REF)
            .unwrap()
            .binary_search(&"b".to_string())
            .is_ok());
    }
}
