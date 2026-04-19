use std::sync::Arc;

use axum::{routing::{delete, get, post}, Router};
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
        .route(
            "/v1/auth/login",
            post(crate::handlers::auth::login),
        )
        .route(
            "/v1/overview",
            post(crate::handlers::overview::get_overview),
        )
        .route(
            "/v1/metrics",
            post(crate::handlers::metrics::get_metrics),
        )
        .route("/v1/nodes", post(crate::handlers::nodes::list_nodes))
        .route("/v1/nodes/get", post(crate::handlers::nodes::get_node))
        .route(
            "/v1/nodes/mutate",
            post(crate::handlers::nodes::mutate_node),
        )
        .route(
            "/v1/nodes/enroll",
            post(crate::handlers::nodes::enroll_node),
        )
        .route("/v1/vms", post(crate::handlers::vms::list_vms))
        .route("/v1/vms/get", post(crate::handlers::vms::get_vm))
        .route("/v1/vms/create", post(crate::handlers::vms::create_vm))
        .route("/v1/vms/delete", post(crate::handlers::vms::delete_vm))
        .route("/v1/vms/mutate", post(crate::handlers::vms::mutate_vm))
        .route(
            "/v1/vms/console",
            post(crate::handlers::vms::get_vm_console),
        )
        .route(
            "/v1/vms/:vm_id/console-url",
            get(crate::handlers::vms::get_vm_console_url),
        )
        .route(
            "/v1/vms/events",
            post(crate::handlers::events::list_events_for_vm),
        )
        .route("/v1/tasks", post(crate::handlers::tasks::list_tasks))
        .route(
            "/v1/tasks/stream",
            axum::routing::get(crate::handlers::tasks::stream_tasks),
        )
        .route(
            "/v1/clusters",
            post(crate::handlers::clusters::list_clusters),
        )
        .route(
            "/v1/networks",
            post(crate::handlers::networks::list_networks),
        )
        .route(
            "/v1/networks/get",
            post(crate::handlers::networks::get_network),
        )
        .route(
            "/v1/networks/create",
            post(crate::handlers::networks::create_network),
        )
        .route(
            "/v1/networks/delete",
            post(crate::handlers::networks::delete_network),
        )
        .route(
            "/v1/networks/update",
            post(crate::handlers::networks::update_network),
        )
        .route(
            "/v1/networks/mutate",
            post(crate::handlers::networks::mutate_network),
        )
        .route("/v1/events", post(crate::handlers::events::list_events))
        .route("/v1/images", post(crate::handlers::images::list_images))
        .route(
            "/v1/images/import",
            post(crate::handlers::images::import_image),
        )
        .route(
            "/v1/images/delete",
            post(crate::handlers::images::delete_image),
        )
        .route(
            "/v1/maintenance",
            post(crate::handlers::maintenance::get_maintenance),
        )
        .route(
            "/v1/settings",
            post(crate::handlers::settings::get_settings),
        )
        .route("/v1/volumes", post(crate::handlers::volumes::list_volumes))
        .route(
            "/v1/volumes/get",
            post(crate::handlers::volumes::get_volume),
        )
        .route(
            "/v1/volumes/mutate",
            post(crate::handlers::volumes::mutate_volume),
        )
        .route("/v1/users", post(crate::handlers::users::list_users))
        .route(
            "/v1/users/create",
            post(crate::handlers::users::create_user),
        )
        .route(
            "/v1/users/update",
            post(crate::handlers::users::update_user),
        )
        .route(
            "/v1/users/delete",
            post(crate::handlers::users::delete_user),
        )
        // Storage Pools
        .route(
            "/v1/storage-pools",
            post(crate::handlers::storage::list_storage_pools),
        )
        .route(
            "/v1/storage-pools/create",
            post(crate::handlers::storage::create_storage_pool),
        )
        // VM Templates
        .route(
            "/v1/vm-templates",
            get(crate::handlers::templates::list_vm_templates)
                .post(crate::handlers::templates::create_vm_template),
        )
        .route(
            "/v1/vm-templates/:id",
            delete(crate::handlers::templates::delete_vm_template),
        )
        // Cloud-init Templates
        .route(
            "/v1/cloud-init-templates",
            get(crate::handlers::templates::list_cloud_init_templates)
                .post(crate::handlers::templates::create_cloud_init_template),
        )
        .route(
            "/v1/cloud-init-templates/:id",
            delete(crate::handlers::templates::delete_cloud_init_template),
        )
        // Firewall Rules
        .route(
            "/v1/firewall-rules",
            post(crate::handlers::firewall::list_firewall_rules),
        )
        .route(
            "/v1/firewall-rules/create",
            post(crate::handlers::firewall::create_firewall_rule),
        )
        .route(
            "/v1/firewall-rules/delete",
            post(crate::handlers::firewall::delete_firewall_rule),
        )
        // VM Snapshots
        .route(
            "/v1/vms/snapshots",
            post(crate::handlers::snapshots::list_vm_snapshots),
        )
        .route(
            "/v1/vms/snapshots/create",
            post(crate::handlers::snapshots::create_snapshot),
        )
        .route(
            "/v1/vms/snapshots/delete",
            post(crate::handlers::snapshots::delete_snapshot),
        )
        .route(
            "/v1/vms/snapshots/restore",
            post(crate::handlers::snapshots::restore_snapshot),
        )
        // API Tokens
        .route("/v1/tokens", post(crate::handlers::tokens::list_tokens))
        .route(
            "/v1/tokens/create",
            post(crate::handlers::tokens::create_token),
        )
        .route(
            "/v1/tokens/revoke",
            post(crate::handlers::tokens::revoke_token),
        )
}
