use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use chv_controlplane_store::{
    AlertRepository, DesiredStateRepository, EventRepository, NodeRepository,
    ObservedStateRepository, OperationRepository, StorePool,
};

use crate::mutations::MutationService;

#[derive(Clone)]
pub struct AppState {
    pub pool: StorePool,
    pub node_repo: NodeRepository,
    pub operation_repo: OperationRepository,
    pub event_repo: EventRepository,
    pub alert_repo: AlertRepository,
    pub desired_state_repo: DesiredStateRepository,
    pub observed_state_repo: ObservedStateRepository,
    pub mutations: Arc<dyn MutationService>,
}

pub fn bff_router() -> Router<AppState> {
    Router::new()
        .route("/v1/overview", get(crate::handlers::overview::get_overview))
        .route("/v1/nodes", get(crate::handlers::nodes::list_nodes))
        .route("/v1/nodes/get", post(crate::handlers::nodes::get_node))
        .route("/v1/vms", get(crate::handlers::vms::list_vms))
        .route("/v1/vms/get", post(crate::handlers::vms::get_vm))
        .route("/v1/vms/mutate", post(crate::handlers::vms::mutate_vm))
        .route("/v1/tasks", get(crate::handlers::tasks::list_tasks))
}
