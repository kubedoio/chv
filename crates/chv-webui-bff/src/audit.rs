pub fn log_mutation(user: &str, action: &str, resource_kind: &str, resource_id: &str) {
    tracing::info!(
        target: "audit",
        user = %user,
        action = %action,
        resource_kind = %resource_kind,
        resource_id = %resource_id,
        "mutation"
    );
}
