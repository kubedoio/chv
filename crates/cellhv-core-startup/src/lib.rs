//! Fail-closed startup authority selection for the NodeCache-to-Core cutover.
//!
//! This library is intentionally not wired into `cmd/chv-agent`. It decides
//! which persistence authority a future startup path may activate and performs
//! only durable migration bookkeeping; it has no VM or provider side effects.

mod identity;

pub use identity::{
    create_fresh_authority, resolve_host_identity, resolve_host_identity_with, FreshHostIdentity,
    FreshIdentitySource, HostIdentityDecision, HostIdentityError, HostIdentityInputs,
};

use cellhv_core_operations::{MigrationDisposition, OperationService, OperationServiceError};
use cellhv_nodecache_migration::{plan, MigrationError, SOURCE_NAME};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct StartupPaths {
    pub node_cache: PathBuf,
    pub core_database: PathBuf,
    pub node_cache_archive: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityDecision {
    /// Core is authoritative; the legacy cache must never be opened for VM writes.
    ActivateCore,
    /// Neither persistence source exists. Future wiring may initialize a new Core host.
    InitializeFreshCore,
}

/// Evidence describing how an [`ActivatedStore`] became authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationKind {
    Existing,
    ImportedNodeCache,
    Fresh,
}

/// A lock-held startup snapshot used only for durable database activation.
pub struct StartupTransaction {
    paths: StartupPaths,
    cache: Option<Vec<u8>>,
    database_exists: bool,
    runtime_lease: cellhv_core_fs::RuntimeAuthorityLease,
    authority_lock: cellhv_core_fs::AuthorityLock,
}

/// Opaque proof that this process still owns the Core database runtime lease.
pub struct RuntimeAuthorityGuard {
    _lease: cellhv_core_fs::RuntimeAuthorityLease,
}

/// Validated provenance for the compatibility snapshot used during activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationProvenance {
    source_checksum: Option<String>,
    live_cache_present: bool,
    any_migration_state: bool,
}

impl ActivationProvenance {
    pub fn source_checksum(&self) -> Option<&str> {
        self.source_checksum.as_deref()
    }

    pub fn live_cache_present(&self) -> bool {
        self.live_cache_present
    }

    pub fn has_any_migration_state(&self) -> bool {
        self.any_migration_state
    }
}

/// An already-open Core store with process-lifetime database exclusion held.
/// This does not select or authorize a NodeCache compatibility mode.
pub struct ActivatedStore {
    service: OperationService,
    kind: ActivationKind,
    runtime_guard: RuntimeAuthorityGuard,
    provenance: ActivationProvenance,
}

/// Database activation completed while the exact NodeCache snapshot and its
/// short transaction lock are still retained for agent composition.
pub struct PendingActivatedStore {
    activated: ActivatedStore,
    node_cache_path: PathBuf,
    cache: Option<Vec<u8>>,
    authority_lock: cellhv_core_fs::AuthorityLock,
}

impl PendingActivatedStore {
    pub fn cache_bytes(&self) -> Option<&[u8]> {
        self.cache.as_deref()
    }

    pub fn node_cache_path(&self) -> &Path {
        &self.node_cache_path
    }

    pub fn provenance(&self) -> &ActivationProvenance {
        self.activated.provenance()
    }

    pub fn finish(self) -> ActivatedStore {
        let Self {
            activated,
            authority_lock,
            ..
        } = self;
        drop(authority_lock);
        activated
    }
}

impl ActivatedStore {
    pub fn service(&self) -> &OperationService {
        &self.service
    }

    pub fn service_mut(&mut self) -> &mut OperationService {
        &mut self.service
    }

    pub fn kind(&self) -> ActivationKind {
        self.kind
    }

    pub fn provenance(&self) -> &ActivationProvenance {
        &self.provenance
    }

    pub fn into_runtime_parts(
        self,
    ) -> (
        OperationService,
        ActivationKind,
        RuntimeAuthorityGuard,
        ActivationProvenance,
    ) {
        (self.service, self.kind, self.runtime_guard, self.provenance)
    }
}

impl StartupTransaction {
    /// Acquires process-lifetime exclusion and the NodeCache transaction lock,
    /// then snapshots both persistence sources while both remain held.
    pub fn begin(paths: &StartupPaths) -> Result<Self> {
        validate_paths(paths)?;
        let runtime_lease = cellhv_core_fs::RuntimeAuthorityLease::acquire(&paths.core_database)
            .map_err(|source| io_error(&paths.core_database, source))?;
        let authority_lock = cellhv_core_fs::AuthorityLock::acquire(&paths.node_cache)
            .map_err(|source| io_error(&paths.node_cache, source))?;
        let cache = read_optional(&paths.node_cache)?;
        let database_exists = paths
            .core_database
            .try_exists()
            .map_err(|source| io_error(&paths.core_database, source))?;
        Ok(Self {
            paths: paths.clone(),
            cache,
            database_exists,
            runtime_lease,
            authority_lock,
        })
    }

    /// Resolves identity, completes import/fresh/open, releases the short cache
    /// lock, and transfers only process-lifetime exclusion into the result.
    pub fn activate(
        self,
        configured_seed: Option<String>,
        precreation_enrollment: Option<String>,
    ) -> Result<ActivatedStore> {
        Ok(self
            .prepare_activation(configured_seed, precreation_enrollment)?
            .finish())
    }

    pub fn prepare_activation(
        self,
        configured_seed: Option<String>,
        precreation_enrollment: Option<String>,
    ) -> Result<PendingActivatedStore> {
        let Self {
            paths,
            cache,
            database_exists,
            runtime_lease,
            authority_lock,
        } = self;

        let live_cache_present = cache.is_some();
        let retained_cache = cache.clone();
        let (service, kind, source_checksum) = match (cache, database_exists) {
            (None, false) => {
                let decision = resolve_host_identity(HostIdentityInputs {
                    configured_seed,
                    precreation_enrollment,
                    ..HostIdentityInputs::default()
                })?;
                (
                    create_fresh_authority(&paths.core_database, &decision)?,
                    ActivationKind::Fresh,
                    None,
                )
            }
            (Some(bytes), false) => {
                let import = plan(&bytes)?;
                resolve_host_identity(HostIdentityInputs {
                    importable_nodecache: Some(import.host().clone()),
                    configured_seed,
                    precreation_enrollment,
                    ..HostIdentityInputs::default()
                })?;
                archive_exact(&paths.node_cache_archive, &bytes, &mut |_| Ok(()))?;
                let mut service = OperationService::create_migration_target(&paths.core_database)?;
                set_owner_only(&paths.core_database)?;
                import.import(&mut service)?;
                import.cutover(&mut service)?;
                let checksum = import.checksum().to_owned();
                (service, ActivationKind::ImportedNodeCache, Some(checksum))
            }
            (cache, true) => {
                let mut service = OperationService::open_existing(&paths.core_database)?;
                let host = service.host()?.identity;
                let import = cache.as_deref().map(plan).transpose()?;
                resolve_host_identity(HostIdentityInputs {
                    existing_core: Some(host),
                    importable_nodecache: import.as_ref().map(|value| value.host().clone()),
                    configured_seed,
                    precreation_enrollment,
                })?;
                let (kind, checksum) =
                    activate_existing(&paths, cache.as_deref(), import.as_ref(), &mut service)?;
                (service, kind, checksum)
            }
        };

        // The short cache transaction ends once the validated snapshot has
        // been consumed. Runtime database exclusion remains process-lifetime.
        let any_migration_state = service.has_any_migration_state()?;
        let activated = ActivatedStore {
            service,
            kind,
            runtime_guard: RuntimeAuthorityGuard {
                _lease: runtime_lease,
            },
            provenance: ActivationProvenance {
                source_checksum,
                live_cache_present,
                any_migration_state,
            },
        };
        Ok(PendingActivatedStore {
            activated,
            node_cache_path: paths.node_cache,
            cache: retained_cache,
            authority_lock,
        })
    }
}

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("Core has an imported snapshot but the source NodeCache is missing")]
    ImportedSourceMissing,
    #[error("NodeCache checksum disagrees with the persisted Core migration marker")]
    ChecksumMismatch,
    #[error("Core database and NodeCache coexist without a NodeCache migration marker")]
    UnrelatedAuthority,
    #[error(
        "migration archive exists beside a markerless Core database, but NodeCache is missing"
    )]
    InterruptedMigrationSourceMissing,
    #[error("archive checksum disagrees with the exact NodeCache bytes")]
    ArchiveMismatch,
    #[error("archive path has no parent directory")]
    InvalidArchivePath,
    #[error("unsafe authority path configuration: {0}")]
    UnsafePath(String),
    #[error("I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Migration(#[from] MigrationError),
    #[error(transparent)]
    Operations(#[from] OperationServiceError),
    #[error(transparent)]
    Identity(#[from] HostIdentityError),
}

pub type Result<T> = std::result::Result<T, StartupError>;

/// Classifies and, where necessary, completes the one-way authority cutover.
pub fn coordinate(paths: &StartupPaths) -> Result<AuthorityDecision> {
    coordinate_with_hook(paths, |_| Ok(()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    ArchiveSynced,
    ArchiveRenamed,
    DatabaseCreated,
    Imported,
    Cutover,
}

fn coordinate_with_hook(
    paths: &StartupPaths,
    mut hook: impl FnMut(Step) -> io::Result<()>,
) -> Result<AuthorityDecision> {
    validate_paths(paths)?;
    let _lock = cellhv_core_fs::AuthorityLock::acquire(&paths.node_cache)
        .map_err(|source| io_error(&paths.node_cache, source))?;
    let cache = read_optional(&paths.node_cache)?;
    let database_exists = paths
        .core_database
        .try_exists()
        .map_err(|source| StartupError::Io {
            path: paths.core_database.clone(),
            source,
        })?;

    match (cache, database_exists) {
        (None, false) => Ok(AuthorityDecision::InitializeFreshCore),
        (Some(bytes), false) => {
            let import = plan(&bytes)?;
            archive_exact(&paths.node_cache_archive, &bytes, &mut hook)?;
            let mut service = OperationService::create_migration_target(&paths.core_database)?;
            set_owner_only(&paths.core_database)?;
            hook(Step::DatabaseCreated).map_err(|source| io_error(&paths.core_database, source))?;
            import.import(&mut service)?;
            hook(Step::Imported).map_err(|source| io_error(&paths.core_database, source))?;
            import.cutover(&mut service)?;
            hook(Step::Cutover).map_err(|source| io_error(&paths.core_database, source))?;
            Ok(AuthorityDecision::ActivateCore)
        }
        (cache, true) => {
            let mut service = OperationService::open_existing(&paths.core_database)?;
            let marker = service.legacy_migration_state(SOURCE_NAME)?;
            match (cache, marker) {
                (None, None) => {
                    if paths
                        .node_cache_archive
                        .try_exists()
                        .map_err(|source| io_error(&paths.node_cache_archive, source))?
                    {
                        return Err(StartupError::InterruptedMigrationSourceMissing);
                    }
                    Ok(AuthorityDecision::ActivateCore)
                }
                (None, Some(marker)) if marker.cutover => {
                    verify_archive(&paths.node_cache_archive, &marker.checksum)?;
                    Ok(AuthorityDecision::ActivateCore)
                }
                (None, Some(_)) => Err(StartupError::ImportedSourceMissing),
                (Some(bytes), None) => {
                    if !service.is_pristine_migration_target()? {
                        return Err(StartupError::UnrelatedAuthority);
                    }
                    let import = plan(&bytes)?;
                    archive_exact(&paths.node_cache_archive, &bytes, &mut hook)?;
                    import.import(&mut service)?;
                    hook(Step::Imported)
                        .map_err(|source| io_error(&paths.core_database, source))?;
                    import.cutover(&mut service)?;
                    hook(Step::Cutover).map_err(|source| io_error(&paths.core_database, source))?;
                    Ok(AuthorityDecision::ActivateCore)
                }
                (Some(bytes), Some(marker)) => {
                    let import = plan(&bytes)?;
                    if import.checksum() != marker.checksum {
                        return Err(StartupError::ChecksumMismatch);
                    }
                    archive_exact(&paths.node_cache_archive, &bytes, &mut hook)?;
                    if !marker.cutover {
                        let disposition = import.import(&mut service)?;
                        debug_assert_eq!(disposition, MigrationDisposition::Replay);
                        import.cutover(&mut service)?;
                        hook(Step::Cutover)
                            .map_err(|source| io_error(&paths.core_database, source))?;
                    }
                    Ok(AuthorityDecision::ActivateCore)
                }
            }
        }
    }
}

fn activate_existing(
    paths: &StartupPaths,
    cache: Option<&[u8]>,
    import: Option<&cellhv_nodecache_migration::ImportPlan>,
    service: &mut OperationService,
) -> Result<(ActivationKind, Option<String>)> {
    let marker = service.legacy_migration_state(SOURCE_NAME)?;
    let source_checksum = marker
        .as_ref()
        .map(|value| value.checksum.clone())
        .or_else(|| import.map(|value| value.checksum().to_owned()));
    let imported = marker.is_some() || import.is_some();
    match (cache, import, marker) {
        (None, None, None) => {
            if paths
                .node_cache_archive
                .try_exists()
                .map_err(|source| io_error(&paths.node_cache_archive, source))?
            {
                return Err(StartupError::InterruptedMigrationSourceMissing);
            }
        }
        (None, None, Some(marker)) if marker.cutover => {
            verify_archive(&paths.node_cache_archive, &marker.checksum)?;
        }
        (None, None, Some(_)) => return Err(StartupError::ImportedSourceMissing),
        (Some(bytes), Some(import), None) => {
            if !service.is_pristine_migration_target()? {
                return Err(StartupError::UnrelatedAuthority);
            }
            archive_exact(&paths.node_cache_archive, bytes, &mut |_| Ok(()))?;
            import.import(service)?;
            import.cutover(service)?;
        }
        (Some(bytes), Some(import), Some(marker)) => {
            if import.checksum() != marker.checksum {
                return Err(StartupError::ChecksumMismatch);
            }
            archive_exact(&paths.node_cache_archive, bytes, &mut |_| Ok(()))?;
            if !marker.cutover {
                let disposition = import.import(service)?;
                debug_assert_eq!(disposition, MigrationDisposition::Replay);
                import.cutover(service)?;
            }
        }
        _ => {
            return Err(StartupError::UnsafePath(
                "inconsistent NodeCache activation snapshot".to_owned(),
            ))
        }
    }
    Ok((
        if imported {
            ActivationKind::ImportedNodeCache
        } else {
            ActivationKind::Existing
        },
        source_checksum,
    ))
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(StartupError::UnsafePath(format!(
                "{} is not a regular file",
                path.display()
            )))
        }
        Ok(metadata) => validate_owner_file(path, &metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(path, source)),
    }
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(io_error(path, source)),
    }
}

fn validate_paths(paths: &StartupPaths) -> Result<()> {
    let parents = [
        safe_parent(&paths.node_cache)?,
        safe_parent(&paths.core_database)?,
        safe_parent(&paths.node_cache_archive)?,
    ];
    for parent in parents {
        let metadata = fs::symlink_metadata(&parent).map_err(|source| io_error(&parent, source))?;
        if !metadata.file_type().is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o777 != 0o700
        {
            return Err(StartupError::UnsafePath(format!(
                "{} must be an owner-owned 0700 directory",
                parent.display()
            )));
        }
    }
    let configured_paths = [
        &paths.node_cache,
        &paths.core_database,
        &paths.node_cache_archive,
    ];
    for path in configured_paths {
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(StartupError::UnsafePath(format!(
                    "{} is not a regular file",
                    path.display()
                )))
            }
            Ok(metadata) => validate_owner_file(path, &metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(io_error(path, source)),
        }
    }
    let archive_temp = archive_temp_path(&paths.node_cache_archive);
    let authority_lock = cellhv_core_fs::lock_path(&paths.node_cache)
        .map_err(|error| StartupError::UnsafePath(error.to_string()))?;
    let runtime_lease = cellhv_core_fs::runtime_lease_path(&paths.core_database)
        .map_err(|error| StartupError::UnsafePath(error.to_string()))?;
    let database_wal = PathBuf::from(format!("{}-wal", paths.core_database.display()));
    let database_shm = PathBuf::from(format!("{}-shm", paths.core_database.display()));
    let all_paths = [
        paths.node_cache.clone(),
        paths.core_database.clone(),
        paths.node_cache_archive.clone(),
        archive_temp,
        authority_lock,
        runtime_lease,
        database_wal,
        database_shm,
    ];
    for (index, left) in all_paths.iter().enumerate() {
        for right in &all_paths[index + 1..] {
            if normalize(left)? == normalize(right)? {
                return Err(StartupError::UnsafePath("authority paths alias".to_owned()));
            }
            if let (Ok(a), Ok(b)) = (fs::metadata(left), fs::metadata(right)) {
                if a.dev() == b.dev() && a.ino() == b.ino() {
                    return Err(StartupError::UnsafePath(
                        "authority paths are hardlink aliases".to_owned(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn safe_parent(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| StartupError::UnsafePath(format!("{} has no parent", path.display())))?;
    let metadata = fs::symlink_metadata(parent).map_err(|source| io_error(parent, source))?;
    if !metadata.file_type().is_dir() {
        return Err(StartupError::UnsafePath(format!(
            "{} must be a real directory",
            parent.display()
        )));
    }
    parent
        .canonicalize()
        .map_err(|source| io_error(parent, source))
}

fn normalize(path: &Path) -> Result<PathBuf> {
    Ok(safe_parent(path)?.join(
        path.file_name()
            .ok_or_else(|| StartupError::UnsafePath("path has no filename".to_owned()))?,
    ))
}

fn validate_owner_file(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(StartupError::UnsafePath(format!(
            "{} must be owner-owned, owner-only, and have one link",
            path.display()
        )));
    }
    Ok(())
}

fn set_owner_only(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|source| io_error(path, source))
}

fn verify_archive(path: &Path, checksum: &str) -> Result<()> {
    let bytes = read_optional(path)?.ok_or(StartupError::ArchiveMismatch)?;
    if format!("{:x}", Sha256::digest(bytes)) != checksum {
        return Err(StartupError::ArchiveMismatch);
    }
    Ok(())
}

fn archive_exact(
    path: &Path,
    bytes: &[u8],
    hook: &mut impl FnMut(Step) -> io::Result<()>,
) -> Result<()> {
    if let Some(existing) = read_optional(path)? {
        if Sha256::digest(existing) != Sha256::digest(bytes) {
            return Err(StartupError::ArchiveMismatch);
        }
        let parent = path.parent().ok_or(StartupError::InvalidArchivePath)?;
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|source| io_error(parent, source))?;
        return Ok(());
    }
    let parent = path.parent().ok_or(StartupError::InvalidArchivePath)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let temp = archive_temp_path(path);
    if let Some(existing) = read_optional(&temp)? {
        if Sha256::digest(existing) != Sha256::digest(bytes) {
            return Err(StartupError::ArchiveMismatch);
        }
        File::open(&temp)
            .and_then(|file| file.sync_all())
            .map_err(|source| io_error(&temp, source))?;
        fs::rename(&temp, path).map_err(|source| io_error(path, source))?;
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|source| io_error(parent, source))?;
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|source| io_error(&temp, source))?;
    if let Err(source) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(io_error(&temp, source));
    }
    hook(Step::ArchiveSynced).map_err(|source| io_error(&temp, source))?;
    fs::rename(&temp, path).map_err(|source| io_error(path, source))?;
    set_owner_only(path)?;
    hook(Step::ArchiveRenamed).map_err(|source| io_error(path, source))?;
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|source| io_error(parent, source))?;
    Ok(())
}

fn archive_temp_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|v| v.to_str())
            .unwrap_or("archive")
    ))
}

fn io_error(path: &Path, source: io::Error) -> StartupError {
    StartupError::Io {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn source() -> Vec<u8> {
        let spec = serde_json::to_vec(&json!({
            "name":"legacy-vm", "cpus":2, "memory_bytes":1073741824_u64,
            "kernel_path":"/kernel", "disks":[], "nics":[], "desired_state":"Running"
        }))
        .unwrap();
        serde_json::to_vec(&json!({
            "cache_version":1, "node_id":"node-a", "observed_generation":"7",
            "node_state":"TenantReady", "enrollment_complete":true,
            "vm_generations":{"vm-a":"3"}, "volume_generations":{}, "network_generations":{},
            "vm_fragments":{"vm-a":{"id":"vm-a","kind":"vm","generation":"3",
                "spec_json":spec,"policy_json":b"{}","updated_at":"2026-07-21T00:00:00Z","updated_by":"controller"}},
            "volume_fragments":{}, "network_fragments":{}, "vm_attachments":{},
            "volume_handles":{}, "pending_control_plane":[]
        })).unwrap()
    }

    fn test_paths(dir: &tempfile::TempDir) -> StartupPaths {
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        StartupPaths {
            node_cache: dir.path().join("node-cache.json"),
            core_database: dir.path().join("core.db"),
            node_cache_archive: dir.path().join("node-cache-v1.archive"),
        }
    }

    fn write_private(path: &Path, bytes: impl AsRef<[u8]>) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn fresh_host_creates_only_the_shared_exclusion_lock() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        assert_eq!(
            coordinate(&paths).unwrap(),
            AuthorityDecision::InitializeFreshCore
        );
        assert!(!paths.core_database.exists());
        assert!(!paths.node_cache.exists());
        assert!(!paths.node_cache_archive.exists());
        assert!(cellhv_core_fs::lock_path(&paths.node_cache)
            .unwrap()
            .is_file());
    }

    #[test]
    fn archives_exact_bytes_then_imports_and_cuts_over() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        let bytes = source();
        write_private(&paths.node_cache, &bytes);
        assert_eq!(coordinate(&paths).unwrap(), AuthorityDecision::ActivateCore);
        assert_eq!(fs::read(&paths.node_cache_archive).unwrap(), bytes);
        let service = OperationService::open_existing(&paths.core_database).unwrap();
        assert!(
            service
                .legacy_migration_state(SOURCE_NAME)
                .unwrap()
                .unwrap()
                .cutover
        );
        assert_eq!(service.vms().unwrap()[0].id.as_str(), "vm-a");
    }

    #[test]
    fn cutover_marker_prevents_changed_json_from_reactivating_authority() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        coordinate(&paths).unwrap();
        let mut changed = source();
        changed.push(b' ');
        write_private(&paths.node_cache, changed);
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::ChecksumMismatch)
        ));
        let service = OperationService::open_existing(&paths.core_database).unwrap();
        assert!(
            service
                .legacy_migration_state(SOURCE_NAME)
                .unwrap()
                .unwrap()
                .cutover
        );
    }

    #[test]
    fn cache_absence_is_allowed_only_after_cutover() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        let bytes = source();
        write_private(&paths.node_cache, &bytes);
        coordinate(&paths).unwrap();
        fs::remove_file(&paths.node_cache).unwrap();
        assert_eq!(coordinate(&paths).unwrap(), AuthorityDecision::ActivateCore);

        let other = tempfile::tempdir().unwrap();
        let other_paths = test_paths(&other);
        let import = plan(&bytes).unwrap();
        let mut service =
            OperationService::create_migration_target(&other_paths.core_database).unwrap();
        import.import(&mut service).unwrap();
        assert!(matches!(
            coordinate(&other_paths),
            Err(StartupError::ImportedSourceMissing)
        ));
    }

    #[test]
    fn markerless_database_with_migration_archive_fails_closed_without_source() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache_archive, source());
        let service = OperationService::create_migration_target(&paths.core_database).unwrap();
        drop(service);

        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::InterruptedMigrationSourceMissing)
        ));
        assert_eq!(fs::read(&paths.node_cache_archive).unwrap(), source());
        let service = OperationService::open_existing(&paths.core_database).unwrap();
        assert!(service.is_pristine_migration_target().unwrap());
    }

    #[test]
    fn malformed_source_fails_before_database_creation() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, b"{");
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::Migration(MigrationError::Malformed(_)))
        ));
        assert!(!paths.core_database.exists());
    }

    #[test]
    fn unsupported_source_fails_before_archive_or_database_creation() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        let mut cache: serde_json::Value = serde_json::from_slice(&source()).unwrap();
        let spec_bytes = cache["vm_fragments"]["vm-a"]["spec_json"]
            .as_array()
            .unwrap()
            .iter()
            .map(|byte| byte.as_u64().unwrap() as u8)
            .collect::<Vec<_>>();
        let mut spec: serde_json::Value = serde_json::from_slice(&spec_bytes).unwrap();
        spec["cloud_init_userdata"] = json!("must not be discarded");
        cache["vm_fragments"]["vm-a"]["spec_json"] = json!(serde_json::to_vec(&spec).unwrap());
        write_private(&paths.node_cache, serde_json::to_vec(&cache).unwrap());

        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::Migration(MigrationError::Unsupported(_)))
        ));
        assert!(!paths.node_cache_archive.exists());
        assert!(!paths.core_database.exists());
    }

    #[test]
    fn every_durable_boundary_is_restartable() {
        for failed_step in [
            Step::ArchiveSynced,
            Step::ArchiveRenamed,
            Step::DatabaseCreated,
            Step::Imported,
            Step::Cutover,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let paths = test_paths(&dir);
            write_private(&paths.node_cache, source());
            let mut fired = false;
            let first = coordinate_with_hook(&paths, |step| {
                if step == failed_step && !fired {
                    fired = true;
                    return Err(io::Error::other("injected crash"));
                }
                Ok(())
            });
            assert!(first.is_err(), "{failed_step:?} did not inject");
            assert_eq!(
                coordinate(&paths).unwrap(),
                AuthorityDecision::ActivateCore,
                "restart after {failed_step:?}"
            );
        }
    }

    #[test]
    fn concurrent_coordinators_converge_under_one_lock() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        let left = paths.clone();
        let right = paths.clone();
        let first = std::thread::spawn(move || coordinate(&left));
        let second = std::thread::spawn(move || coordinate(&right));
        assert_eq!(
            first.join().unwrap().unwrap(),
            AuthorityDecision::ActivateCore
        );
        assert_eq!(
            second.join().unwrap().unwrap(),
            AuthorityDecision::ActivateCore
        );
    }

    #[test]
    fn coordinator_reads_source_only_after_shared_save_lock() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        let lock = cellhv_core_fs::AuthorityLock::acquire(&paths.node_cache).unwrap();
        let worker_paths = paths.clone();
        let worker = std::thread::spawn(move || coordinate(&worker_paths));
        let mut replacement = source();
        replacement.push(b' ');
        write_private(&paths.node_cache, &replacement);
        drop(lock);
        assert_eq!(
            worker.join().unwrap().unwrap(),
            AuthorityDecision::ActivateCore
        );
        assert_eq!(fs::read(&paths.node_cache_archive).unwrap(), replacement);
    }

    #[test]
    fn cutover_without_matching_owner_only_archive_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        coordinate(&paths).unwrap();
        fs::remove_file(&paths.node_cache).unwrap();
        fs::remove_file(&paths.node_cache_archive).unwrap();
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::ArchiveMismatch)
        ));
        write_private(&paths.node_cache_archive, b"corrupt");
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::ArchiveMismatch)
        ));
    }

    #[test]
    fn path_aliases_special_files_and_unsafe_modes_are_rejected() {
        for pair in [(0, 1), (0, 2), (1, 2)] {
            let dir = tempfile::tempdir().unwrap();
            let mut paths = test_paths(&dir);
            let mut values = [
                paths.node_cache.clone(),
                paths.core_database.clone(),
                paths.node_cache_archive.clone(),
            ];
            values[pair.1] = values[pair.0].clone();
            paths.node_cache = values[0].clone();
            paths.core_database = values[1].clone();
            paths.node_cache_archive = values[2].clone();
            assert!(matches!(
                coordinate(&paths),
                Err(StartupError::UnsafePath(_))
            ));
        }

        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        fs::hard_link(&paths.node_cache, &paths.node_cache_archive).unwrap();
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));
        fs::remove_file(&paths.node_cache_archive).unwrap();
        fs::create_dir(&paths.node_cache_archive).unwrap();
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));
        fs::remove_dir(&paths.node_cache_archive).unwrap();
        let mut normalized = paths.clone();
        fs::create_dir(dir.path().join("unused")).unwrap();
        normalized.node_cache_archive = dir.path().join("unused/../node-cache.json");
        assert!(matches!(
            coordinate(&normalized),
            Err(StartupError::UnsafePath(_))
        ));
        let mut sidecar = paths.clone();
        sidecar.node_cache_archive =
            PathBuf::from(format!("{}-wal", paths.core_database.display()));
        assert!(matches!(
            coordinate(&sidecar),
            Err(StartupError::UnsafePath(_))
        ));
        let mut archive_temp_cache = paths.clone();
        archive_temp_cache.node_cache = archive_temp_path(&paths.node_cache_archive);
        assert!(matches!(
            coordinate(&archive_temp_cache),
            Err(StartupError::UnsafePath(_))
        ));
        let mut archive_temp_database = paths.clone();
        archive_temp_database.core_database = archive_temp_path(&paths.node_cache_archive);
        assert!(matches!(
            coordinate(&archive_temp_database),
            Err(StartupError::UnsafePath(_))
        ));
        let mut lock_database = paths.clone();
        lock_database.core_database = cellhv_core_fs::lock_path(&paths.node_cache).unwrap();
        assert!(matches!(
            coordinate(&lock_database),
            Err(StartupError::UnsafePath(_))
        ));
        let mut shm_cache = paths.clone();
        shm_cache.node_cache = PathBuf::from(format!("{}-shm", paths.core_database.display()));
        assert!(matches!(
            coordinate(&shm_cache),
            Err(StartupError::UnsafePath(_))
        ));
        let derived_temp = archive_temp_path(&paths.node_cache_archive);
        write_private(&derived_temp, b"derived");
        let derived_lock = cellhv_core_fs::lock_path(&paths.node_cache).unwrap();
        fs::hard_link(&derived_temp, &derived_lock).unwrap();
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));
        fs::remove_file(&derived_lock).unwrap();
        fs::remove_file(&derived_temp).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));
    }

    #[test]
    fn archive_and_database_are_explicitly_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        coordinate(&paths).unwrap();
        for path in [&paths.node_cache_archive, &paths.core_database] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", paths.core_database.display()));
            if sidecar.exists() {
                assert_eq!(
                    fs::metadata(sidecar).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
    }

    #[test]
    fn symlink_parents_and_external_hardlinks_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let cache_dir = dir.path().join("cache");
        let real_archive = dir.path().join("real-archive");
        let real_db = dir.path().join("real-db");
        for path in [&cache_dir, &real_archive, &real_db] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let archive_link = dir.path().join("archive-link");
        std::os::unix::fs::symlink(&real_archive, &archive_link).unwrap();
        let paths = StartupPaths {
            node_cache: cache_dir.join("cache.json"),
            core_database: real_db.join("core.db"),
            node_cache_archive: archive_link.join("archive.json"),
        };
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));

        fs::remove_file(&archive_link).unwrap();
        let db_link = dir.path().join("db-link");
        std::os::unix::fs::symlink(&real_db, &db_link).unwrap();
        let paths = StartupPaths {
            core_database: db_link.join("core.db"),
            node_cache_archive: real_archive.join("archive.json"),
            ..paths
        };
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));

        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        fs::hard_link(&paths.node_cache, dir.path().join("external-cache-link")).unwrap();
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));
        fs::remove_file(dir.path().join("external-cache-link")).unwrap();
        coordinate(&paths).unwrap();
        fs::remove_file(&paths.node_cache).unwrap();
        fs::hard_link(
            &paths.node_cache_archive,
            dir.path().join("external-archive-link"),
        )
        .unwrap();
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::UnsafePath(_))
        ));
    }

    #[test]
    fn mismatched_archive_never_allows_import() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        write_private(&paths.node_cache_archive, b"different");
        assert!(matches!(
            coordinate(&paths),
            Err(StartupError::ArchiveMismatch)
        ));
        assert!(!paths.core_database.exists());
    }

    #[test]
    fn startup_transaction_yields_open_fresh_authority_and_restart_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        let transaction = StartupTransaction::begin(&paths).unwrap();
        let active = transaction
            .activate(Some("host-transaction".to_owned()), None)
            .unwrap();
        assert_eq!(active.kind(), ActivationKind::Fresh);
        assert_eq!(
            active.service().host().unwrap().identity.id.as_str(),
            "host-transaction"
        );
        let (service, kind, runtime_guard, provenance) = active.into_runtime_parts();
        assert_eq!(kind, ActivationKind::Fresh);
        assert!(provenance.source_checksum().is_none());
        assert!(!provenance.live_cache_present());
        assert!(matches!(
            StartupTransaction::begin(&paths),
            Err(StartupError::Io { source, .. })
                if source.kind() == io::ErrorKind::WouldBlock
        ));

        drop(service);
        drop(runtime_guard);
        let restarted = StartupTransaction::begin(&paths)
            .unwrap()
            .activate(Some("host-transaction".to_owned()), None)
            .unwrap();
        assert_eq!(restarted.kind(), ActivationKind::Existing);
        assert_eq!(
            restarted.service().host().unwrap().identity.id.as_str(),
            "host-transaction"
        );
    }

    #[test]
    fn activated_store_releases_cache_lock_but_retains_runtime_lease() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        let transaction = StartupTransaction::begin(&paths).unwrap();
        let active = transaction
            .activate(Some("host-lock-held".to_owned()), None)
            .unwrap();
        let cache_guard = cellhv_core_fs::AuthorityLock::acquire(&paths.node_cache).unwrap();
        assert!(matches!(
            StartupTransaction::begin(&paths),
            Err(StartupError::Io { source, .. })
                if source.kind() == io::ErrorKind::WouldBlock
        ));
        drop(cache_guard);
        drop(active);
        drop(StartupTransaction::begin(&paths).unwrap());
    }

    #[test]
    fn startup_transaction_rejects_runtime_lease_aliases_before_locking() {
        let dir = tempfile::tempdir().unwrap();
        let mut paths = test_paths(&dir);
        paths.node_cache = cellhv_core_fs::runtime_lease_path(&paths.core_database).unwrap();
        assert!(matches!(
            StartupTransaction::begin(&paths),
            Err(StartupError::UnsafePath(_))
        ));
        assert!(!paths.core_database.exists());
    }

    #[test]
    fn startup_transaction_imports_exact_snapshot_and_restarts_under_one_lease() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(&dir);
        write_private(&paths.node_cache, source());
        let active = StartupTransaction::begin(&paths)
            .unwrap()
            .activate(Some("node-a".to_owned()), None)
            .unwrap();
        assert_eq!(active.kind(), ActivationKind::ImportedNodeCache);
        assert!(paths.node_cache_archive.exists());
        assert_eq!(
            active.service().host().unwrap().identity.id.as_str(),
            "node-a"
        );
        drop(active);
        fs::remove_file(&paths.node_cache).unwrap();

        let restarted = StartupTransaction::begin(&paths)
            .unwrap()
            .activate(Some("node-a".to_owned()), None)
            .unwrap();
        assert_eq!(restarted.kind(), ActivationKind::ImportedNodeCache);
        assert_eq!(
            restarted.service().host().unwrap().identity.id.as_str(),
            "node-a"
        );
    }
}
