use crate::{run_migrations, StorePool};
use sqlx::SqlitePool;

pub struct TestDb {
    pub pool: StorePool,
}

impl TestDb {
    pub async fn new() -> Self {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("failed to create in-memory SQLite pool");
        run_migrations(&pool, None)
            .await
            .expect("failed to run migrations on test pool");
        Self { pool }
    }
}

pub async fn create_test_pool() -> StorePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory SQLite pool");
    run_migrations(&pool, None)
        .await
        .expect("failed to run migrations on test pool");
    pool
}
