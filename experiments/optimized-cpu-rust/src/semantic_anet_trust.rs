//! A1-R2 native-only accepted-v0.15 source replay provider.
//! The deployment owns the immutable upstream checkout, runtime toolchain and UID.
//! A hostile same-UID process or compromised toolchain is outside the trust model.
//! This module does not compile FORMAL, choose a Theory or interpret a Rule.
use super::{
    SemanticLoadError, SourceReplayVerified, UpstreamReplayAnchor, COMPACT_AND_CONTENT_SHA256,
    COMPACT_AND_SOURCE_SHA256, MTS_ACCEPTED_REVISION, TRANSPORT_IMPLEMENTATION_REVISION,
    TWO_ROLE_CONTENT_SHA256, TWO_ROLE_JSON_SHA256, TWO_ROLE_SOURCE_SHA256,
};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Explicit finite allowlist; no public caller-provided digest, source path or
/// transport provenance can mint a source-replay capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptedV015Source {
    CompactAnd,
    TwoRoleMetaRule,
}

struct SourcePin {
    id: &'static str,
    path: &'static str,
    source_sha: &'static str,
    json_path: &'static str,
    json_sha: &'static str,
    content_sha: &'static str,
}

impl AcceptedV015Source {
    fn pin(self) -> SourcePin {
        match self {
            Self::CompactAnd => SourcePin {
                id: "compact-and",
                path: "formal/v0.15/regression/compact-and.formal",
                source_sha: COMPACT_AND_SOURCE_SHA256,
                json_path: "formal/v0.15/regression/compact-and.json",
                json_sha: "cf5028f68500bba04ee10e06ac5512eec223078d8b428f4d5de1de6433ba5941",
                content_sha: COMPACT_AND_CONTENT_SHA256,
            },
            Self::TwoRoleMetaRule => SourcePin {
                id: "two-role-meta-rule",
                path: "formal/v0.15/regression/two-role-meta-rule.formal",
                source_sha: TWO_ROLE_SOURCE_SHA256,
                json_path: "formal/v0.15/regression/two-role-meta-rule.json",
                json_sha: TWO_ROLE_JSON_SHA256,
                content_sha: TWO_ROLE_CONTENT_SHA256,
            },
        }
    }
}

struct PrivateReplayDir(PathBuf);
impl PrivateReplayDir {
    fn new() -> Result<Self, SemanticLoadError> {
        use std::os::unix::fs::DirBuilderExt;
        let ticks = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| SemanticLoadError::UntrustedSource)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "amemory-trusted-v015-{}-{ticks}",
            std::process::id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(&path)
            .map_err(|_| SemanticLoadError::UntrustedSource)?;
        Ok(Self(path))
    }
}
impl Drop for PrivateReplayDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn checked(command: &mut Command) -> Result<String, SemanticLoadError> {
    let result = command.output().map_err(|_| SemanticLoadError::UntrustedSource)?;
    if !result.status.success() {
        return Err(SemanticLoadError::UntrustedSource);
    }
    String::from_utf8(result.stdout).map_err(|_| SemanticLoadError::UntrustedSource)
}
fn digest(path: &Path) -> Result<String, SemanticLoadError> {
    let out = checked(Command::new("sha256sum").arg(path))?;
    Ok(out.split_whitespace().next().unwrap_or_default().to_owned())
}
fn clean(checkout: &Path) -> Result<bool, SemanticLoadError> {
    Ok(checked(Command::new("git").arg("-C").arg(checkout).args([
        "status", "--porcelain", "--untracked-files=no",
    ]))?.trim().is_empty())
}
fn invoke_bridge(
    bridge: &Path,
    checkout: &Path,
    id: &str,
    action: &str,
    artifact: &Path,
) -> Result<String, SemanticLoadError> {
    checked(
        Command::new("node")
            .env_remove("NODE_OPTIONS")
            .env_remove("MTS_V015_SNAPSHOT_OUTPUT")
            .env_remove("MTS_V015_SNAPSHOT_INPUT")
            .env_remove("MTS_V015_SNAPSHOT_CHILD")
            .arg(bridge)
            .arg(checkout)
            .arg(id)
            .arg(action)
            .arg(artifact),
    )
}

impl SourceReplayVerified {
    /// Same private replay protocol for BOTH Author-approved sources.
    /// Trust is granted only after exact checkout, source hashes, upstream
    /// compiler/denotation, independent second-process replay and digest pin.
    pub fn from_pinned_accepted_checkout(
        upstream_checkout: &Path,
        source: AcceptedV015Source,
    ) -> Result<Self, SemanticLoadError> {
        let pin = source.pin();
        let checkout = upstream_checkout
            .canonicalize()
            .map_err(|_| SemanticLoadError::UntrustedSource)?;
        let head = checked(Command::new("git").arg("-C").arg(&checkout)
            .args(["rev-parse", "HEAD"]))?;
        if head.trim() != TRANSPORT_IMPLEMENTATION_REVISION || !clean(&checkout)? {
            return Err(SemanticLoadError::UntrustedSource);
        }
        if digest(&checkout.join(pin.path))? != pin.source_sha
            || digest(&checkout.join(pin.json_path))? != pin.json_sha
        {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let ts = checkout.join("ts");
        checked(Command::new("npm")
            .env_remove("NODE_OPTIONS")
            .args(["ci", "--ignore-scripts", "--prefix"])
            .arg(&ts))?;
        checked(Command::new("npm")
            .env_remove("NODE_OPTIONS")
            .args(["run", "build", "--prefix"])
            .arg(&ts))?;
        if !clean(&checkout)? {
            return Err(SemanticLoadError::UntrustedSource);
        }

        // Repository-owned adapter calls only the pinned upstream compiler and
        // source-backed verifier, never a second locally implemented compiler.
        let bridge = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../a-circuit/upstream/v015-accepted-source-replay.mjs");
        if !bridge.is_file() {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let private = PrivateReplayDir::new()?;
        let file = private.0.join("semantic-anet.json");
        let producer = invoke_bridge(&bridge, &checkout, pin.id, "export", &file)?;
        let verifier = invoke_bridge(&bridge, &checkout, pin.id, "verify", &file)?;
        if !producer.contains("A1_R2_PINNED_SOURCE_INDEPENDENT_REPLAY_AND_FALSIFIERS=PASS")
            || !verifier.contains("EXTERNAL_TRANSPORT_FILE_SOURCE_REPLAY=PASS")
        {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let generated = std::fs::read_to_string(&file)
            .map_err(|_| SemanticLoadError::UntrustedSource)?;
        let snapshot: serde_json::Value =
            serde_json::from_str(&generated).map_err(|_| SemanticLoadError::UntrustedSource)?;
        let receipt = serde_json::json!({
            "status": "SOURCE_REPLAY_VERIFIED",
            "topology": snapshot["topology"],
            "members": snapshot["members"],
            "source": snapshot["source"],
            "contentSha256": snapshot["contentSha256"],
        }).to_string();
        let anchor = UpstreamReplayAnchor {
            authenticated_transport_revision: TRANSPORT_IMPLEMENTATION_REVISION,
            accepted_mts_revision: MTS_ACCEPTED_REVISION,
            expected_source_sha256: pin.source_sha,
            expected_content_sha256: pin.content_sha,
            expected_source_path: pin.path,
            authenticated_replay_json: &receipt,
        };
        Self::authenticate(&receipt, &anchor)
    }

    /// Existing public API retained, now using the common provider.
    pub fn from_pinned_compact_and_checkout(
        upstream_checkout: &Path,
    ) -> Result<Self, SemanticLoadError> {
        Self::from_pinned_accepted_checkout(upstream_checkout, AcceptedV015Source::CompactAnd)
    }

    /// Approved source identity only; caller cannot supply arbitrary accepted
    /// source metadata, identity, expected digest or semantic membership.
    pub fn from_pinned_two_role_meta_rule_checkout(
        upstream_checkout: &Path,
    ) -> Result<Self, SemanticLoadError> {
        Self::from_pinned_accepted_checkout(upstream_checkout, AcceptedV015Source::TwoRoleMetaRule)
    }
}
