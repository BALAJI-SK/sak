//! Verify the shipped rule packs in `./packs/` parse and produce a
//! Guardian with sensible counts. This is the integration point that
//! protects against regressions in the YAML schema or generator output.

use sak_guardian::Guardian;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` for this crate is `crates/sak-guardian`; the
    // workspace root is two directories up.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // -> crates/
    p.pop(); // -> workspace root
    p
}

#[test]
fn loads_all_shipped_packs() {
    let root = workspace_root();
    let packs: Vec<PathBuf> = [
        "packs/defaults.yaml",
        "packs/solana-core.yaml",
        "packs/exploits-blocklist.yaml",
        "packs/tokens-blocklist.yaml",
    ]
    .iter()
    .map(|rel| root.join(rel))
    .collect();

    // If any pack is missing, the loader skips it — so first assert that
    // they actually exist on disk before we trust the count.
    for p in &packs {
        assert!(p.exists(), "rule pack missing: {}", p.display());
    }

    let guardian =
        Guardian::from_yaml_files(&packs).expect("failed to load shipped rule packs");
    let stats = guardian.stats();

    assert!(
        stats.total >= 2000,
        "expected at least 2000 rule instances, got {}",
        stats.total
    );
    assert!(
        stats.by_kind.get("blocked_program").copied().unwrap_or(0) >= 1500,
        "expected at least 1500 blocked_program rules, got {:?}",
        stats.by_kind
    );
    assert_eq!(stats.packs.len(), 4, "expected 4 packs, got {:?}", stats.packs);

    // Defaults pack must contribute the slippage / drain / compute rules.
    for k in [
        "slippage_check",
        "drain_check",
        "compute_units_check",
        "priority_fee_check",
        "account_count_check",
    ] {
        assert!(
            stats.by_kind.get(k).copied().unwrap_or(0) >= 1,
            "expected at least one rule of kind {k}, got {:?}",
            stats.by_kind
        );
    }
}
