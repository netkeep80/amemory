//! Native-only accepted-v0.15 source replay provider. No host compiler copy.
//! Trust boundary: deployment owns Git checkout + tools + UID; an untrusted
//! request controls ONLY the candidate snapshot. A hostile same-UID process or
//! compromised toolchain/dependency registry is outside the supported boundary.
use super::{
    SemanticLoadError, SourceReplayVerified, UpstreamReplayAnchor,
    COMPACT_AND_CONTENT_SHA256, COMPACT_AND_SOURCE_SHA256, MTS_ACCEPTED_REVISION,
    TRANSPORT_IMPLEMENTATION_REVISION,
};
use std::path::{Path, PathBuf};
use std::process::Command;

struct PrivateReplayDir(PathBuf);

impl PrivateReplayDir {
    fn new() -> Result<Self, SemanticLoadError> {
        use std::os::unix::fs::DirBuilderExt;
        let ticks = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| SemanticLoadError::UntrustedSource)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "amemory-trusted-v015-{}-{ticks}", std::process::id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&path).map_err(|_| SemanticLoadError::UntrustedSource)?;
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
fn replay(ts: &Path, name: &str, artifact: &Path) -> Result<String, SemanticLoadError> {
    let mut cmd = Command::new("node");
    cmd.current_dir(ts)
        .arg("dist/test/v015-semantic-snapshot-transport.test.js")
        .env_remove("MTS_V015_SNAPSHOT_OUTPUT")
        .env_remove("MTS_V015_SNAPSHOT_INPUT")
        .env_remove("MTS_V015_SNAPSHOT_CHILD")
        .env_remove("NODE_OPTIONS")
        .env(name, artifact);
    checked(&mut cmd)
}
impl SourceReplayVerified {
    /// Mint the ONLY public v0.15 replay capability from accepted compact AND.
    ///
    /// The checkout path MUST come from the trusted deployment, never untrusted
    /// request input. Pins are compiled into A-memory; checkout HEAD, tracked
    /// files and accepted FORMAL byte digest are checked before any execution.
    /// Existing upstream TS build, compiler, denotation, and independent source
    /// verifier are reused unmodified, in separate Node processes. The source
    /// output passes through a Rust-owned mode-0700 directory, not user files.
    /// This is NOT a claim of universal FORMAL/JSON production provisioning.
    pub fn from_pinned_compact_and_checkout(
        upstream_checkout: &Path,
    ) -> Result<Self, SemanticLoadError> {
        let checkout = upstream_checkout
            .canonicalize().map_err(|_| SemanticLoadError::UntrustedSource)?;
        let head = checked(Command::new("git").arg("-C").arg(&checkout)
            .args(["rev-parse", "HEAD"]))?;
        if head.trim() != TRANSPORT_IMPLEMENTATION_REVISION {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let clean = || checked(Command::new("git").arg("-C").arg(&checkout)
            .args(["status", "--porcelain", "--untracked-files=no"]));
        if !clean()?.trim().is_empty() {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let source = checkout.join("formal/v0.15/regression/compact-and.formal");
        let sha = checked(Command::new("sha256sum").arg(&source))?;
        if sha.split_whitespace().next() != Some(COMPACT_AND_SOURCE_SHA256) {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let ts = checkout.join("ts");
        checked(Command::new("npm").args(["ci", "--ignore-scripts", "--prefix"]).arg(&ts))?;
        checked(Command::new("npm").args(["run", "build", "--prefix"]).arg(&ts))?;
        if !clean()?.trim().is_empty() {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let private = PrivateReplayDir::new()?;
        let file = private.0.join("semantic-anet.json");
        if !replay(&ts, "MTS_V015_SNAPSHOT_OUTPUT", &file)?
            .contains("P3 RESTORE_AND_SOURCE_REPLAY=PASS") {
            return Err(SemanticLoadError::UntrustedSource);
        }
        if !replay(&ts, "MTS_V015_SNAPSHOT_INPUT", &file)?
            .contains("EXTERNAL_TRANSPORT_FILE_SOURCE_REPLAY=PASS") {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let generated = std::fs::read_to_string(&file)
            .map_err(|_| SemanticLoadError::UntrustedSource)?;
        let snapshot: serde_json::Value = serde_json::from_str(&generated)
            .map_err(|_| SemanticLoadError::UntrustedSource)?;
        let receipt = serde_json::json!({
            "status": "SOURCE_REPLAY_VERIFIED",
            "topology": snapshot["topology"],
            "members": snapshot["members"],
            "source": snapshot["source"],
            "contentSha256": snapshot["contentSha256"]
        }).to_string();
        let anchor = UpstreamReplayAnchor {
            authenticated_transport_revision: TRANSPORT_IMPLEMENTATION_REVISION,
            accepted_mts_revision: MTS_ACCEPTED_REVISION,
            expected_source_sha256: COMPACT_AND_SOURCE_SHA256,
            expected_content_sha256: COMPACT_AND_CONTENT_SHA256,
            expected_source_path: "formal/v0.15/regression/compact-and.formal",
            authenticated_replay_json: &receipt,
        };
        Self::authenticate(&receipt, &anchor)
    }
}
