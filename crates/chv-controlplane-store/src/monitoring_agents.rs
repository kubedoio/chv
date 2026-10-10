//! Optional guest monitoring agent registry (ADR-026, campaign #602,
//! prompt 03 / gate G3).
//!
//! Durable manager metadata for the optional `chv-monitor-agent`:
//! hashed single-use enrollment claims and enrolled agent identities.
//! This is registry metadata in the main control-plane database —
//! never time-series storage (samples live in the isolated
//! `chv-monitoring-store`).
//!
//! Security invariants (agent security contract v1):
//! - Claims are stored **hashed** (SHA-256). The plaintext token is
//!   returned exactly once at issuance and never persisted.
//! - Claim consumption is a single atomic `UPDATE … RETURNING` — the
//!   existence/expiry/single-use checks live in the WHERE clause, so
//!   there is no TOCTOU window between two racing redemptions.
//! - The manager stores only **public** credential material (cert
//!   serial, fingerprint, not_after). Agent private keys never exist
//!   on the manager.
//! - At most one `active` agent identity per VM (partial unique
//!   index): replacement is an explicit revoke-then-re-enroll
//!   operation, never a silent takeover.
//! - `install_id` mismatches flag `identity_conflict` (cloned-image
//!   detection); recovery requires an authorized reset.

use crate::{StoreError, StorePool};
use chv_common::sha256_hex;
use rand::RngExt;

/// Claim tokens are `chvm_` + 32 random bytes, base64url (no padding):
/// 256 bits of entropy, safe to paste into a credential file, never a
/// valid shell-quoting hazard beyond ordinary quoting.
const CLAIM_PREFIX: &str = "chvm_";

fn generate_claim_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    format!("{CLAIM_PREFIX}{}", base64url_nopad(&bytes))
}

fn base64url_nopad(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        out.push(ALPHABET[(b[0] >> 2) as usize] as char);
        out.push(ALPHABET[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(b[2] & 0x3f) as usize] as char);
        }
    }
    out
}

/// A freshly issued claim. The `token` plaintext exists only here —
/// the caller shows it once, then drops it.
#[derive(Debug, Clone)]
pub struct IssuedClaim {
    pub token: String,
    pub vm_id: String,
    pub tenant_id: Option<String>,
    pub expires_at_ms: i64,
}

/// A live (issued, unconsumed, unexpired) claim for a VM — powers the
/// `enrolling` wire state between claim issuance and redemption. No
/// token material: only the hash row's metadata.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LiveClaim {
    pub vm_id: String,
    pub issued_by: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsumedClaim {
    pub vm_id: String,
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ClaimConsumeError {
    /// No claim row matches the token hash.
    Unknown,
    /// The claim matched but was already consumed.
    AlreadyUsed,
    /// The claim matched but its expiry has passed.
    Expired,
}

/// Public credential material for one enrolled agent.
#[derive(Debug, Clone)]
pub struct AgentCredential {
    pub cert_serial: String,
    pub cert_fingerprint: String,
    pub cert_not_after_ms: i64,
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct MonitoringAgentRow {
    pub agent_id: String,
    pub vm_id: String,
    pub tenant_id: Option<String>,
    pub install_id: String,
    pub credential_epoch: i64,
    pub cert_serial: String,
    pub cert_fingerprint: String,
    pub cert_not_after_ms: i64,
    pub previous_serial: Option<String>,
    pub previous_fingerprint: Option<String>,
    pub rotated_at_ms: Option<i64>,
    pub rotation_pending: bool,
    pub status: String,
    pub identity_conflict: bool,
    pub conflict_reason: Option<String>,
    pub enrolled_at_ms: i64,
    pub enrolled_by: String,
    pub last_seen_at_ms: Option<i64>,
    pub last_boot_id: Option<String>,
    pub last_sequence: Option<i64>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub os_kernel_release: Option<String>,
    pub revoked_at_ms: Option<i64>,
    pub revoked_by: Option<String>,
}

/// Why an authenticated-looking agent was rejected at the registry.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AgentAuthError {
    Unknown,
    Revoked,
    CredentialExpired,
    /// The presented certificate matches neither the current credential
    /// nor the previous one inside the rotation grace window.
    StaleCredential {
        grace_ms: i64,
    },
    /// The registry flagged a cloned-image identity conflict; an
    /// authorized reset is required before this agent may report again.
    IdentityConflict,
}

/// OS identity metadata from the ingestion v1 `os` envelope allowlist.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentOsMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub kernel_release: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RecordBatchOutcome {
    Accepted,
    /// install_id differs from the enrolled record — cloned-image
    /// signal. The agent is flagged; the batch is rejected.
    InstallMismatch,
}

#[derive(Clone)]
pub struct MonitoringAgentRepository {
    pool: StorePool,
}

impl MonitoringAgentRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    /// Issue a single-use claim scoped to an existing VM. The plaintext
    /// token is returned once; only its SHA-256 is stored.
    pub async fn issue_claim(
        &self,
        vm_id: &str,
        issued_by: &str,
        ttl_ms: i64,
        now_ms: i64,
    ) -> Result<IssuedClaim, StoreError> {
        // The VM must exist: claims are scoped to exactly one existing
        // VM identity (security contract enrollment step 1).
        let (tenant_id,): (Option<String>,) =
            sqlx::query_as("SELECT tenant_id FROM vms WHERE vm_id = $1")
                .bind(vm_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound {
                    entity: "vm",
                    id: vm_id.to_string(),
                })?;

        let token = generate_claim_token();
        let claim_hash = sha256_hex(&token);
        let expires_at_ms = now_ms + ttl_ms;
        sqlx::query(
            "INSERT INTO monitoring_agent_claims
                 (claim_hash, vm_id, tenant_id, issued_by, issued_at_ms, expires_at_ms)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&claim_hash)
        .bind(vm_id)
        .bind(&tenant_id)
        .bind(issued_by)
        .bind(now_ms)
        .bind(expires_at_ms)
        .execute(&self.pool)
        .await?;

        // Retention: consumed/expired claims are unusable the moment
        // they pass their expiry — keep them for a 24 h audit window,
        // then delete. Pruning here (rather than a background task)
        // keeps the table bounded with zero extra infrastructure; an
        // unused claim's hash row is the only secret-shaped data
        // involved, and it is already inert.
        let _ = sqlx::query(
            "DELETE FROM monitoring_agent_claims
             WHERE expires_at_ms < $1 - 86_400_000",
        )
        .bind(now_ms)
        .execute(&self.pool)
        .await;

        Ok(IssuedClaim {
            token,
            vm_id: vm_id.to_string(),
            tenant_id,
            expires_at_ms,
        })
    }

    /// The newest live (unconsumed, unexpired) claim for a VM, if any —
    /// the `enrolling` state's backing query. Newest first: re-issuing
    /// a claim while an older one is still live is legitimate (the
    /// old token simply expires unused).
    pub async fn find_live_claim_by_vm(
        &self,
        vm_id: &str,
        now_ms: i64,
    ) -> Result<Option<LiveClaim>, StoreError> {
        Ok(sqlx::query_as::<_, LiveClaim>(
            "SELECT vm_id, issued_by, issued_at_ms, expires_at_ms
             FROM monitoring_agent_claims
             WHERE vm_id = $1 AND consumed_at_ms IS NULL AND expires_at_ms > $2
             ORDER BY issued_at_ms DESC
             LIMIT 1",
        )
        .bind(vm_id)
        .bind(now_ms)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Atomically consume a claim. All validity checks (exists, unused,
    /// unexpired) are pushed into the UPDATE's WHERE clause so two
    /// racing redemptions cannot both succeed.
    pub async fn consume_claim(
        &self,
        token: &str,
        install_id: &str,
        remote_ip: Option<&str>,
        now_ms: i64,
    ) -> Result<Result<ConsumedClaim, ClaimConsumeError>, StoreError> {
        let claim_hash = sha256_hex(token);

        // Single atomic consume. Integer ms comparison — no text clock.
        let consumed: Option<(String, Option<String>)> = sqlx::query_as(
            "UPDATE monitoring_agent_claims
             SET consumed_at_ms = $1, consumed_install_id = $2, consumed_by_ip = $3
             WHERE claim_hash = $4
               AND consumed_at_ms IS NULL
               AND expires_at_ms > $1
             RETURNING vm_id, tenant_id",
        )
        .bind(now_ms)
        .bind(install_id)
        .bind(remote_ip)
        .bind(&claim_hash)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((vm_id, tenant_id)) = consumed {
            return Ok(Ok(ConsumedClaim { vm_id, tenant_id }));
        }

        // The atomic consume did not match — classify for the caller
        // (and the audit log) without leaking which part failed to the
        // unauthenticated requester.
        let row: Option<(Option<i64>, i64)> = sqlx::query_as(
            "SELECT consumed_at_ms, expires_at_ms FROM monitoring_agent_claims
             WHERE claim_hash = $1",
        )
        .bind(&claim_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(Err(match row {
            None => ClaimConsumeError::Unknown,
            Some((Some(_), _)) => ClaimConsumeError::AlreadyUsed,
            Some((None, expires_at_ms)) if expires_at_ms <= now_ms => ClaimConsumeError::Expired,
            // Raced with a concurrent consumer between the two queries.
            Some((None, _)) => ClaimConsumeError::AlreadyUsed,
        }))
    }

    /// Enroll a new agent identity for a VM. Fails with a Conflict when
    /// an active agent already exists for that VM — replacement is an
    /// explicit revoke-then-re-enroll operation, never a takeover.
    pub async fn enroll_agent(
        &self,
        agent_id: &str,
        vm_id: &str,
        install_id: &str,
        credential: &AgentCredential,
        enrolled_by: &str,
        now_ms: i64,
    ) -> Result<MonitoringAgentRow, StoreError> {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT agent_id FROM monitoring_agents WHERE vm_id = $1 AND status = 'active'",
        )
        .bind(vm_id)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(existing_id) = existing {
            return Err(StoreError::Conflict {
                entity: "monitoring_agent",
                id: existing_id,
                reason: "an active guest agent is already enrolled for this vm; revoke it before enrolling a replacement",
            });
        }

        sqlx::query(
            "INSERT INTO monitoring_agents
                 (agent_id, vm_id, tenant_id, install_id, credential_epoch,
                  cert_serial, cert_fingerprint, cert_not_after_ms,
                  status, enrolled_at_ms, enrolled_by)
             SELECT $1, $2, v.tenant_id, $3, 1, $4, $5, $6, 'active', $7, $8
             FROM vms v WHERE v.vm_id = $2",
        )
        .bind(agent_id)
        .bind(vm_id)
        .bind(install_id)
        .bind(&credential.cert_serial)
        .bind(&credential.cert_fingerprint)
        .bind(credential.cert_not_after_ms)
        .bind(now_ms)
        .bind(enrolled_by)
        .execute(&self.pool)
        .await?;

        self.find_agent(agent_id)
            .await?
            .ok_or(StoreError::NotFound {
                entity: "monitoring_agent",
                id: agent_id.to_string(),
            })
    }

    pub async fn find_agent(
        &self,
        agent_id: &str,
    ) -> Result<Option<MonitoringAgentRow>, StoreError> {
        Ok(sqlx::query_as::<_, MonitoringAgentRow>(
            "SELECT * FROM monitoring_agents WHERE agent_id = $1",
        )
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn find_active_agent_by_vm(
        &self,
        vm_id: &str,
    ) -> Result<Option<MonitoringAgentRow>, StoreError> {
        Ok(sqlx::query_as::<_, MonitoringAgentRow>(
            "SELECT * FROM monitoring_agents WHERE vm_id = $1 AND status = 'active'",
        )
        .bind(vm_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Authenticate an agent by id + presented certificate fingerprint.
    /// Checks revocation, expiry, the current credential, and the
    /// previous credential inside the rotation grace window.
    pub async fn authenticate_agent(
        &self,
        agent_id: &str,
        cert_fingerprint: &str,
        now_ms: i64,
        rotation_grace_ms: i64,
    ) -> Result<Result<MonitoringAgentRow, AgentAuthError>, StoreError> {
        let Some(agent) = self.find_agent(agent_id).await? else {
            return Ok(Err(AgentAuthError::Unknown));
        };
        if agent.status == "revoked" {
            return Ok(Err(AgentAuthError::Revoked));
        }
        if agent.identity_conflict {
            return Ok(Err(AgentAuthError::IdentityConflict));
        }
        if now_ms >= agent.cert_not_after_ms {
            return Ok(Err(AgentAuthError::CredentialExpired));
        }
        if agent.cert_fingerprint == cert_fingerprint {
            return Ok(Ok(agent));
        }
        // Rotation grace: the previous credential still authenticates
        // until rotated_at + grace elapses.
        let within_grace = match (agent.previous_fingerprint.as_deref(), agent.rotated_at_ms) {
            (Some(prev), Some(rotated_at)) => {
                prev == cert_fingerprint && now_ms < rotated_at + rotation_grace_ms
            }
            _ => false,
        };
        if within_grace {
            return Ok(Ok(agent));
        }
        Ok(Err(AgentAuthError::StaleCredential {
            grace_ms: rotation_grace_ms,
        }))
    }

    /// Record a successful batch: last-seen, boot/sequence, OS metadata.
    /// An `install_id` mismatch flags the cloned-image conflict and
    /// rejects the batch.
    pub async fn record_batch(
        &self,
        agent_id: &str,
        install_id: &str,
        boot_id: &str,
        sequence: u64,
        os: &AgentOsMetadata,
        now_ms: i64,
    ) -> Result<RecordBatchOutcome, StoreError> {
        let outcome: Option<i64> = sqlx::query_scalar(
            "UPDATE monitoring_agents SET
                 last_seen_at_ms = $1,
                 last_boot_id = $2,
                 last_sequence = $3,
                 os_name = COALESCE($4, os_name),
                 os_version = COALESCE($5, os_version),
                 os_kernel_release = COALESCE($6, os_kernel_release),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE agent_id = $7 AND install_id = $8 AND status = 'active'
             RETURNING 1",
        )
        .bind(now_ms)
        .bind(boot_id)
        .bind(sequence as i64)
        .bind(&os.name)
        .bind(&os.version)
        .bind(&os.kernel_release)
        .bind(agent_id)
        .bind(install_id)
        .fetch_optional(&self.pool)
        .await?;

        if outcome.is_some() {
            return Ok(RecordBatchOutcome::Accepted);
        }

        // No row matched: either unknown/revoked, or an install_id
        // mismatch — the cloned-image signal. Flag it for operator
        // review only when the agent row is live under a different
        // install id.
        let flagged = sqlx::query_scalar::<_, i64>(
            "UPDATE monitoring_agents SET
                 identity_conflict = 1,
                 conflict_reason = 'install_id mismatch: a credential was presented from a different install than the one enrolled',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE agent_id = $1 AND install_id != $2 AND status = 'active' AND identity_conflict = 0
             RETURNING 1",
        )
        .bind(agent_id)
        .bind(install_id)
        .fetch_optional(&self.pool)
        .await?;

        if flagged.is_some() {
            tracing::warn!(
                agent_id = %agent_id,
                "monitoring agent identity conflict flagged (install_id mismatch)"
            );
        }
        Ok(RecordBatchOutcome::InstallMismatch)
    }

    /// Rotate an agent's credential: the current credential moves to
    /// `previous` (valid for the grace window) and the new one becomes
    /// current. The epoch increments so guest samples fence on it.
    pub async fn rotate_agent(
        &self,
        agent_id: &str,
        new_credential: &AgentCredential,
        now_ms: i64,
    ) -> Result<MonitoringAgentRow, StoreError> {
        let updated = sqlx::query(
            "UPDATE monitoring_agents SET
                 credential_epoch = credential_epoch + 1,
                 previous_serial = cert_serial,
                 previous_fingerprint = cert_fingerprint,
                 cert_serial = $2,
                 cert_fingerprint = $3,
                 cert_not_after_ms = $4,
                 rotated_at_ms = $5,
                 rotation_pending = 0,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE agent_id = $1 AND status = 'active'",
        )
        .bind(agent_id)
        .bind(&new_credential.cert_serial)
        .bind(&new_credential.cert_fingerprint)
        .bind(new_credential.cert_not_after_ms)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: "monitoring_agent",
                id: agent_id.to_string(),
            });
        }
        self.find_agent(agent_id)
            .await?
            .ok_or(StoreError::NotFound {
                entity: "monitoring_agent",
                id: agent_id.to_string(),
            })
    }

    /// Revoke an agent's credential. Terminal for this identity; a
    /// replacement enrolls through a fresh claim.
    pub async fn revoke_agent(
        &self,
        agent_id: &str,
        revoked_by: &str,
        now_ms: i64,
    ) -> Result<MonitoringAgentRow, StoreError> {
        let updated = sqlx::query(
            "UPDATE monitoring_agents SET
                 status = 'revoked',
                 revoked_at_ms = $2,
                 revoked_by = $3,
                 previous_serial = NULL,
                 previous_fingerprint = NULL,
                 rotation_pending = 0,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE agent_id = $1 AND status = 'active'",
        )
        .bind(agent_id)
        .bind(now_ms)
        .bind(revoked_by)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: "monitoring_agent",
                id: agent_id.to_string(),
            });
        }
        self.find_agent(agent_id)
            .await?
            .ok_or(StoreError::NotFound {
                entity: "monitoring_agent",
                id: agent_id.to_string(),
            })
    }

    /// Mark an agent for a forced credential rotation (operator
    /// action). The next accepted batch response tells the agent its
    /// credential is `renewal_due`; the rotate call clears the flag.
    pub async fn set_rotation_pending(&self, agent_id: &str) -> Result<(), StoreError> {
        let updated = sqlx::query(
            "UPDATE monitoring_agents SET
                 rotation_pending = 1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE agent_id = $1 AND status = 'active'",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: "monitoring_agent",
                id: agent_id.to_string(),
            });
        }
        Ok(())
    }

    /// Clear a cloned-image identity conflict so the agent may report
    /// again (explicit authorized recovery; the credential itself was
    /// never the thing in doubt — the install identity was).
    pub async fn clear_conflict(&self, agent_id: &str) -> Result<(), StoreError> {
        let updated = sqlx::query(
            "UPDATE monitoring_agents SET
                 identity_conflict = 0,
                 conflict_reason = NULL,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE agent_id = $1",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: "monitoring_agent",
                id: agent_id.to_string(),
            });
        }
        Ok(())
    }

    /// List agents (optionally filtered by VM), newest enrollment
    /// first, bounded by `limit`.
    pub async fn list_agents(
        &self,
        vm_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<MonitoringAgentRow>, StoreError> {
        Ok(sqlx::query_as::<_, MonitoringAgentRow>(
            "SELECT * FROM monitoring_agents
             WHERE ($1 IS NULL OR vm_id = $1)
             ORDER BY enrolled_at_ms DESC LIMIT $2",
        )
        .bind(vm_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::create_test_pool;

    async fn seed_vm(pool: &StorePool, vm_id: &str, tenant_id: Option<&str>) {
        sqlx::query("INSERT INTO vms (vm_id, display_name, tenant_id) VALUES ($1, $1, $2)")
            .bind(vm_id)
            .bind(tenant_id)
            .execute(pool)
            .await
            .expect("seed vm");
    }

    fn credential(epoch_serial: &str) -> AgentCredential {
        AgentCredential {
            cert_serial: epoch_serial.to_string(),
            cert_fingerprint: format!("fp-{epoch_serial}"),
            cert_not_after_ms: 4_102_444_800_000, // 2100-01-01
        }
    }

    #[tokio::test]
    async fn claim_issues_consumes_once_and_expires() {
        let pool = create_test_pool().await;
        let repo = MonitoringAgentRepository::new(pool.clone());
        seed_vm(&pool, "vm-a", Some("tenant-1")).await;

        let claim = repo
            .issue_claim("vm-a", "op-user", 600_000, 1_000_000)
            .await
            .expect("issue");
        assert!(claim.token.starts_with(CLAIM_PREFIX));
        assert_eq!(claim.tenant_id.as_deref(), Some("tenant-1"));

        // Unknown token.
        assert_eq!(
            repo.consume_claim("chvm_bogus", "install-1", None, 1_000_001)
                .await
                .unwrap(),
            Err(ClaimConsumeError::Unknown)
        );

        // First use succeeds and returns the binding.
        let consumed = repo
            .consume_claim(&claim.token, "install-1", Some("10.0.0.9"), 1_000_001)
            .await
            .unwrap()
            .expect("first consume");
        assert_eq!(consumed.vm_id, "vm-a");
        assert_eq!(consumed.tenant_id.as_deref(), Some("tenant-1"));

        // Second use: already consumed.
        assert_eq!(
            repo.consume_claim(&claim.token, "install-1", None, 1_000_002)
                .await
                .unwrap(),
            Err(ClaimConsumeError::AlreadyUsed)
        );

        // Expired claim is never consumable.
        let expiring = repo
            .issue_claim("vm-a", "op-user", 10_000, 2_000_000)
            .await
            .expect("issue");
        assert_eq!(
            repo.consume_claim(&expiring.token, "install-1", None, 2_011_000)
                .await
                .unwrap(),
            Err(ClaimConsumeError::Expired)
        );
    }

    #[tokio::test]
    async fn claim_requires_existing_vm() {
        let pool = create_test_pool().await;
        let repo = MonitoringAgentRepository::new(pool);
        let err = repo
            .issue_claim("vm-missing", "op-user", 600_000, 1)
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::NotFound { .. }));
    }

    #[tokio::test]
    async fn enrollment_binds_identity_and_blocks_takeover() {
        let pool = create_test_pool().await;
        let repo = MonitoringAgentRepository::new(pool.clone());
        seed_vm(&pool, "vm-b", None).await;

        let agent = repo
            .enroll_agent("agent-1", "vm-b", "install-1", &credential("s1"), "op", 100)
            .await
            .expect("enroll");
        assert_eq!(agent.credential_epoch, 1);
        assert_eq!(agent.status, "active");
        assert!(agent.last_seen_at_ms.is_none());

        // A second active agent for the same VM is a conflict.
        let err = repo
            .enroll_agent("agent-2", "vm-b", "install-2", &credential("s2"), "op", 200)
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::Conflict { .. }));

        // After revocation, a replacement may enroll.
        repo.revoke_agent("agent-1", "op", 300).await.unwrap();
        repo.enroll_agent("agent-2", "vm-b", "install-2", &credential("s2"), "op", 400)
            .await
            .expect("replacement enrolls after revoke");
    }

    #[tokio::test]
    async fn authentication_checks_all_rejection_paths() {
        let pool = create_test_pool().await;
        let repo = MonitoringAgentRepository::new(pool.clone());
        seed_vm(&pool, "vm-c", None).await;
        let not_after = 10_000_000;
        let cred = AgentCredential {
            cert_serial: "s1".into(),
            cert_fingerprint: "fp-1".into(),
            cert_not_after_ms: not_after,
        };
        repo.enroll_agent("agent-1", "vm-c", "install-1", &cred, "op", 0)
            .await
            .unwrap();

        // Happy path.
        assert!(repo
            .authenticate_agent("agent-1", "fp-1", not_after - 1, 60_000)
            .await
            .unwrap()
            .is_ok());

        assert_eq!(
            repo.authenticate_agent("agent-x", "fp-1", 1, 60_000)
                .await
                .unwrap(),
            Err(AgentAuthError::Unknown)
        );

        // Wrong cert (neither current nor previous-within-grace).
        assert_eq!(
            repo.authenticate_agent("agent-1", "fp-other", 1, 60_000)
                .await
                .unwrap(),
            Err(AgentAuthError::StaleCredential { grace_ms: 60_000 })
        );

        // Expired credential.
        assert_eq!(
            repo.authenticate_agent("agent-1", "fp-1", not_after, 60_000)
                .await
                .unwrap(),
            Err(AgentAuthError::CredentialExpired)
        );

        // Revoked.
        repo.revoke_agent("agent-1", "op", 5).await.unwrap();
        assert_eq!(
            repo.authenticate_agent("agent-1", "fp-1", 1, 60_000)
                .await
                .unwrap(),
            Err(AgentAuthError::Revoked)
        );
    }

    #[tokio::test]
    async fn rotation_grace_accepts_previous_then_expires() {
        let pool = create_test_pool().await;
        let repo = MonitoringAgentRepository::new(pool.clone());
        seed_vm(&pool, "vm-d", None).await;
        repo.enroll_agent("agent-1", "vm-d", "install-1", &credential("s1"), "op", 0)
            .await
            .unwrap();

        let rotated = repo
            .rotate_agent("agent-1", &credential("s2"), 1_000)
            .await
            .expect("rotate");
        assert_eq!(rotated.credential_epoch, 2);
        assert_eq!(rotated.previous_fingerprint.as_deref(), Some("fp-s1"));

        // Current cert authenticates.
        assert!(repo
            .authenticate_agent("agent-1", "fp-s2", 1_100, 60_000)
            .await
            .unwrap()
            .is_ok());
        // Previous cert inside the grace window authenticates.
        assert!(repo
            .authenticate_agent("agent-1", "fp-s1", 1_100, 60_000)
            .await
            .unwrap()
            .is_ok());
        // Previous cert outside the grace window does not.
        assert_eq!(
            repo.authenticate_agent("agent-1", "fp-s1", 1_000 + 60_000, 60_000)
                .await
                .unwrap(),
            Err(AgentAuthError::StaleCredential { grace_ms: 60_000 })
        );

        // Operator-forced rotation sets the renewal flag; a completed
        // rotation clears it.
        repo.set_rotation_pending("agent-1").await.unwrap();
        assert!(
            repo.find_agent("agent-1")
                .await
                .unwrap()
                .unwrap()
                .rotation_pending
        );
        let rotated = repo
            .rotate_agent("agent-1", &credential("s3"), 2_000)
            .await
            .expect("rotate again");
        assert!(!rotated.rotation_pending);
    }

    #[tokio::test]
    async fn batch_recording_flags_install_mismatch() {
        let pool = create_test_pool().await;
        let repo = MonitoringAgentRepository::new(pool.clone());
        seed_vm(&pool, "vm-e", None).await;
        repo.enroll_agent("agent-1", "vm-e", "install-1", &credential("s1"), "op", 0)
            .await
            .unwrap();

        let os = AgentOsMetadata {
            name: Some("Ubuntu".into()),
            version: Some("24.04".into()),
            kernel_release: Some("6.8.0-42".into()),
        };
        assert_eq!(
            repo.record_batch("agent-1", "install-1", "boot-1", 7, &os, 500)
                .await
                .unwrap(),
            RecordBatchOutcome::Accepted
        );
        let agent = repo.find_agent("agent-1").await.unwrap().unwrap();
        assert_eq!(agent.last_seen_at_ms, Some(500));
        assert_eq!(agent.last_boot_id.as_deref(), Some("boot-1"));
        assert_eq!(agent.last_sequence, Some(7));
        assert_eq!(agent.os_name.as_deref(), Some("Ubuntu"));

        // Absent OS fields leave the record unchanged.
        let empty = AgentOsMetadata::default();
        repo.record_batch("agent-1", "install-1", "boot-1", 8, &empty, 600)
            .await
            .unwrap();
        let agent = repo.find_agent("agent-1").await.unwrap().unwrap();
        assert_eq!(agent.os_name.as_deref(), Some("Ubuntu"));
        assert_eq!(agent.last_sequence, Some(8));

        // Cloned-image signal: different install_id flags the conflict.
        assert_eq!(
            repo.record_batch("agent-1", "install-clone", "boot-1", 9, &empty, 700)
                .await
                .unwrap(),
            RecordBatchOutcome::InstallMismatch
        );
        let agent = repo.find_agent("agent-1").await.unwrap().unwrap();
        assert!(agent.identity_conflict);
        assert!(agent.conflict_reason.is_some());

        // The conflict blocks authentication until cleared.
        assert_eq!(
            repo.authenticate_agent("agent-1", "fp-s1", 800, 60_000)
                .await
                .unwrap(),
            Err(AgentAuthError::IdentityConflict)
        );
        repo.clear_conflict("agent-1").await.unwrap();
        assert!(repo
            .authenticate_agent("agent-1", "fp-s1", 900, 60_000)
            .await
            .unwrap()
            .is_ok());
    }

    #[tokio::test]
    async fn claim_hash_is_stored_not_plaintext() {
        let pool = create_test_pool().await;
        let repo = MonitoringAgentRepository::new(pool.clone());
        seed_vm(&pool, "vm-f", None).await;
        let claim = repo
            .issue_claim("vm-f", "op", 600_000, 1)
            .await
            .expect("issue");

        let raw: String =
            sqlx::query_scalar("SELECT claim_hash FROM monitoring_agent_claims LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_ne!(raw, claim.token);
        assert_eq!(raw, sha256_hex(&claim.token));
        assert_eq!(raw.len(), 64);
    }
}
