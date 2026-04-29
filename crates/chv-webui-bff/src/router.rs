use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use chv_controlplane_store::{
    AlertRepository, BackupRepository, DesiredStateRepository, EventRepository, NodeRepository,
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
    pub backup_repo: BackupRepository,
    pub mutations: Arc<dyn MutationService>,
    pub jwt_secret: String,
    pub agent_runtime_dir: PathBuf,
}

pub fn bff_router(state: AppState) -> Router<AppState> {
    // Unauthenticated
    let login = Router::new().route("/v1/auth/login", post(crate::handlers::auth::login));

    // Viewer — read-only and personal endpoints
    let viewer = Router::new()
        .route(
            "/v1/overview",
            post(crate::handlers::overview::get_overview),
        )
        .route("/v1/metrics", post(crate::handlers::metrics::get_metrics))
        .route("/v1/nodes", post(crate::handlers::nodes::list_nodes))
        .route("/v1/nodes/get", post(crate::handlers::nodes::get_node))
        .route("/v1/vms", post(crate::handlers::vms::list_vms))
        .route("/v1/vms/get", post(crate::handlers::vms::get_vm))
        .route(
            "/v1/vms/console",
            post(crate::handlers::vms::get_vm_console),
        )
        .route(
            "/v1/vms/:vm_id/console-url",
            get(crate::handlers::vms::get_vm_console_url),
        )
        .route(
            "/v1/exports/:export_id/download",
            get(crate::handlers::exports::download_export),
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
        .route("/v1/events", post(crate::handlers::events::list_events))
        .route("/v1/images", post(crate::handlers::images::list_images))
        .route(
            "/v1/maintenance",
            post(crate::handlers::maintenance::get_maintenance),
        )
        .route(
            "/v1/settings",
            post(crate::handlers::settings::get_settings),
        )
        .route(
            "/v1/settings/hypervisor",
            get(crate::handlers::hypervisor_settings::get_settings)
                .post(crate::handlers::hypervisor_settings::get_settings),
        )
        .route(
            "/v1/settings/hypervisor/profiles",
            get(crate::handlers::hypervisor_settings::list_profiles)
                .post(crate::handlers::hypervisor_settings::list_profiles),
        )
        .route("/v1/volumes", post(crate::handlers::volumes::list_volumes))
        .route(
            "/v1/volumes/get",
            post(crate::handlers::volumes::get_volume),
        )
        .route(
            "/v1/storage-pools",
            post(crate::handlers::storage::list_storage_pools),
        )
        .route(
            "/v1/vm-templates",
            get(crate::handlers::templates::list_vm_templates),
        )
        .route(
            "/v1/vm-templates/:id",
            get(crate::handlers::templates::preview_vm_template),
        )
        .route(
            "/v1/cloud-init-templates",
            get(crate::handlers::templates::list_cloud_init_templates),
        )
        .route(
            "/v1/firewall-rules",
            post(crate::handlers::firewall::list_firewall_rules),
        )
        .route(
            "/v1/vms/snapshots",
            post(crate::handlers::snapshots::list_vm_snapshots),
        )
        .route("/v1/quotas", post(crate::handlers::quotas::list_quotas))
        .route("/api/v1/quotas", post(crate::handlers::quotas::list_quotas))
        .route("/v1/quotas/me", post(crate::handlers::quotas::get_my_quota))
        .route(
            "/api/v1/quotas/me",
            post(crate::handlers::quotas::get_my_quota),
        )
        .route(
            "/v1/quotas/:user_id",
            get(crate::handlers::quotas::get_quota),
        )
        .route(
            "/api/v1/quotas/:user_id",
            get(crate::handlers::quotas::get_quota),
        )
        .route(
            "/v1/quotas/:user_id/usage",
            post(crate::handlers::quotas::get_usage),
        )
        .route(
            "/api/v1/quotas/:user_id/usage",
            post(crate::handlers::quotas::get_usage),
        )
        .route("/v1/usage", post(crate::handlers::quotas::get_usage))
        .route("/api/v1/usage", post(crate::handlers::quotas::get_usage))
        .route(
            "/v1/quotas/check",
            post(crate::handlers::quotas::check_quota),
        )
        .route(
            "/api/v1/quotas/check",
            post(crate::handlers::quotas::check_quota),
        )
        .route("/v1/tokens", post(crate::handlers::tokens::list_tokens))
        // Legacy backup endpoints
        .route(
            "/v1/backup-jobs",
            post(crate::handlers::backups::list_backup_jobs),
        )
        .route(
            "/v1/backup-history",
            post(crate::handlers::backups::list_backup_history),
        )
        // New backup endpoints (viewer)
        .route(
            "/v1/backups/jobs",
            get(crate::handlers::backups::list_backup_jobs_rest),
        )
        .route(
            "/v1/backups/jobs/:job_id",
            get(crate::handlers::backups::get_backup_job),
        )
        .route(
            "/v1/backups/schedules",
            get(crate::handlers::backups::list_backup_schedules),
        )
        .route(
            "/v1/backups/schedules/:schedule_id",
            get(crate::handlers::backups::get_backup_schedule),
        )
        .route(
            "/v1/backups/restores",
            get(crate::handlers::backups::list_backup_restores),
        )
        .route(
            "/v1/backups/restores/:restore_id",
            get(crate::handlers::backups::get_backup_restore),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::viewer_middleware,
        ));

    // Operator — VM lifecycle, network/storage mutations
    let operator = Router::new()
        .route(
            "/v1/nodes/mutate",
            post(crate::handlers::nodes::mutate_node),
        )
        .route("/v1/vms/create", post(crate::handlers::vms::create_vm))
        .route("/v1/vms/delete", post(crate::handlers::vms::delete_vm))
        .route("/v1/vms/resize", post(crate::handlers::vms::resize_vm))
        .route("/v1/vms/mutate", post(crate::handlers::vms::mutate_vm))
        .route(
            "/v1/vms/:vm_id/export",
            post(crate::handlers::exports::export_vm),
        )
        .route("/v1/vms/import", post(crate::handlers::imports::import_vm))
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
        .route(
            "/v1/images/import",
            post(crate::handlers::images::import_image),
        )
        .route(
            "/v1/images/delete",
            post(crate::handlers::images::delete_image),
        )
        .route(
            "/v1/volumes/mutate",
            post(crate::handlers::volumes::mutate_volume),
        )
        .route(
            "/v1/volumes/snapshot",
            post(crate::handlers::volumes::snapshot_volume),
        )
        .route(
            "/v1/volumes/restore-snapshot",
            post(crate::handlers::volumes::restore_volume_snapshot),
        )
        .route(
            "/v1/volumes/delete-snapshot",
            post(crate::handlers::volumes::delete_volume_snapshot),
        )
        .route(
            "/v1/volumes/clone",
            post(crate::handlers::volumes::clone_volume),
        )
        .route(
            "/v1/storage-pools/create",
            post(crate::handlers::storage::create_storage_pool),
        )
        .route(
            "/v1/vm-templates",
            post(crate::handlers::templates::create_vm_template),
        )
        .route(
            "/v1/vm-templates/:id",
            delete(crate::handlers::templates::delete_vm_template),
        )
        .route(
            "/v1/vm-templates/:id/clone",
            post(crate::handlers::templates::clone_vm_template),
        )
        .route(
            "/v1/cloud-init-templates",
            post(crate::handlers::templates::create_cloud_init_template),
        )
        .route(
            "/v1/cloud-init-templates/:id",
            delete(crate::handlers::templates::delete_cloud_init_template),
        )
        .route(
            "/v1/cloud-init-templates/:id/render",
            post(crate::handlers::templates::render_cloud_init_template),
        )
        .route(
            "/v1/firewall-rules/create",
            post(crate::handlers::firewall::create_firewall_rule),
        )
        .route(
            "/v1/firewall-rules/delete",
            post(crate::handlers::firewall::delete_firewall_rule),
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
        .route(
            "/v1/tokens/create",
            post(crate::handlers::tokens::create_token),
        )
        .route(
            "/v1/tokens/revoke",
            post(crate::handlers::tokens::revoke_token),
        )
        // New backup endpoints (operator)
        .route(
            "/v1/backups/jobs",
            post(crate::handlers::backups::create_backup_job),
        )
        .route(
            "/v1/backups/jobs/:job_id",
            patch(crate::handlers::backups::update_backup_job)
                .delete(crate::handlers::backups::delete_backup_job),
        )
        .route(
            "/v1/backups/jobs/:job_id/execute",
            post(crate::handlers::backups::execute_backup_job),
        )
        .route(
            "/v1/backups/schedules",
            post(crate::handlers::backups::create_backup_schedule),
        )
        .route(
            "/v1/backups/schedules/:schedule_id",
            patch(crate::handlers::backups::update_backup_schedule)
                .delete(crate::handlers::backups::delete_backup_schedule),
        )
        .route(
            "/v1/backups/restores",
            post(crate::handlers::backups::create_backup_restore),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::operator_middleware,
        ));

    // Admin — user management, settings, node enrollment
    let admin = Router::new()
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
        .route(
            "/v1/nodes/enroll",
            post(crate::handlers::nodes::enroll_node),
        )
        .route(
            "/v1/settings/hypervisor",
            patch(crate::handlers::hypervisor_settings::update_settings),
        )
        .route(
            "/v1/settings/hypervisor/update",
            post(crate::handlers::hypervisor_settings::update_settings),
        )
        .route(
            "/v1/settings/hypervisor/apply-profile",
            post(crate::handlers::hypervisor_settings::apply_profile),
        )
        .route(
            "/v1/settings/hypervisor/apply-profile/:id",
            post(crate::handlers::hypervisor_settings::apply_profile_by_path),
        )
        .route(
            "/v1/quotas/create",
            post(crate::handlers::quotas::create_quota),
        )
        .route(
            "/api/v1/quotas/create",
            post(crate::handlers::quotas::create_quota),
        )
        .route(
            "/v1/quotas/:user_id",
            patch(crate::handlers::quotas::update_quota)
                .delete(crate::handlers::quotas::delete_quota),
        )
        .route(
            "/api/v1/quotas/:user_id",
            patch(crate::handlers::quotas::update_quota)
                .delete(crate::handlers::quotas::delete_quota),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::admin_middleware,
        ));

    login.merge(viewer).merge(operator).merge(admin).layer(
        axum::middleware::from_fn(crate::metrics_middleware::track_metrics),
    )
}
