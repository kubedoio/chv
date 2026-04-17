use std::sync::Arc;

use axum::{
    routing::post,
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
    pub jwt_secret: String,
}

pub fn bff_router() -> Router<AppState> {
    Router::new()
        .route("/v1/overview", post(crate::handlers::overview::get_overview))
        .route("/v1/nodes", post(crate::handlers::nodes::list_nodes))
        .route("/v1/nodes/get", post(crate::handlers::nodes::get_node))
        .route("/v1/nodes/mutate", post(crate::handlers::nodes::mutate_node))
        .route("/v1/vms", post(crate::handlers::vms::list_vms))
        .route("/v1/vms/get", post(crate::handlers::vms::get_vm))
        .route("/v1/vms/mutate", post(crate::handlers::vms::mutate_vm))
        .route("/v1/vms/events", post(crate::handlers::events::list_events_for_vm))
        .route("/v1/tasks", post(crate::handlers::tasks::list_tasks))
        .route("/v1/tasks/stream", axum::routing::get(crate::handlers::tasks::stream_tasks))
        .route("/v1/clusters", post(crate::handlers::clusters::list_clusters))
        .route("/v1/networks", post(crate::handlers::networks::list_networks))
        .route("/v1/networks/get", post(crate::handlers::networks::get_network))
        .route("/v1/networks/mutate", post(crate::handlers::networks::mutate_network))
        .route("/v1/events", post(crate::handlers::events::list_events))
        .route("/v1/images", post(crate::handlers::images::list_images))
        .route("/v1/maintenance", post(crate::handlers::maintenance::get_maintenance))
        .route("/v1/settings", post(crate::handlers::settings::get_settings))
        .route("/v1/volumes", post(crate::handlers::volumes::list_volumes))
        .route("/v1/volumes/get", post(crate::handlers::volumes::get_volume))
        .route("/v1/volumes/mutate", post(crate::handlers::volumes::mutate_volume))
}
