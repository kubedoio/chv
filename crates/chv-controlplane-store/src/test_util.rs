use crate::{connect_pool, migrations_path, run_migrations, ControlPlaneStoreConfig, StorePool};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

pub struct TestDb {
    pub pool: StorePool,
    _container: testcontainers::ContainerAsync<Postgres>,
}

impl TestDb {
    pub async fn new() -> Self {
        let container = Postgres::default()
            .with_user("postgres")
            .with_password("postgres")
            .with_db_name("chv_test")
            .start()
            .await
            .expect("Failed to start Postgres container");

        let host = container.get_host().await.expect("Failed to get host");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");
        let database_url = format!("postgres://postgres:postgres@{}:{}/chv_test", host, port);

        let config = ControlPlaneStoreConfig {
            database_url: database_url.clone(),
            migrations_dir: migrations_path(None).to_path_buf(),
            max_connections: 5,
            min_connections: 1,
            acquire_timeout_secs: 10,
            idle_timeout_secs: 60,
            max_lifetime_secs: 120,
        };

        let pool = connect_pool(&config)
            .await
            .expect("Failed to connect to test database");

        run_migrations(&pool, Some(&config))
            .await
            .expect("Failed to run migrations on test database");

        Self {
            pool,
            _container: container,
        }
    }
}
