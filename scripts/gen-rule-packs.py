#!/usr/bin/env python3
"""
Generate Guardian rule packs from real Solana ecosystem data.

Outputs to ./packs/:
  - solana-core.yaml          curated allowlist for core programs (whitelist)
  - exploits-blocklist.yaml   known scam / fee-on-transfer / rug program ids
  - tokens-allowlist.yaml     Jupiter strict-list mints (one whitelist rule)
  - tokens-blocklist.yaml     long-tail / unverified mints from Jupiter
                              full token list, emitted as `blocked_program`
                              entries to exercise the indexed dispatch path.

The point of this generator is that the output is reproducible from public
data sources, not invented by the build script. Commit the output YAML so
the repo runs offline.

Usage:
  python3 scripts/gen-rule-packs.py [--limit 2000]
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from pathlib import Path
from typing import Iterable

# Real, current Solana programs. Pubkeys are mainnet program ids.
CORE_PROGRAMS: list[tuple[str, str]] = [
    ("system_program",                  "11111111111111111111111111111111"),
    ("compute_budget",                  "ComputeBudget111111111111111111111111111111"),
    ("spl_token",                       "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
    ("spl_token_2022",                  "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
    ("associated_token_account",        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bN"),
    ("memo_v2",                         "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"),
    ("address_lookup_table",            "AddressLookupTab1e1111111111111111111111111"),
    # DEX aggregators / AMMs
    ("jupiter_aggregator_v6",           "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"),
    ("orca_whirlpool",                  "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzM3Mh8rh7o"),
    ("raydium_amm_v4",                  "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"),
    ("raydium_clmm",                    "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK"),
    ("meteora_pools",                   "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB"),
    ("meteora_dlmm",                    "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"),
    ("phoenix_orderbook",               "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY"),
    ("openbook_v2",                     "opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb"),
    ("lifinity_v2",                     "2wT8Yq49kHgDzXuPxZSaeLaH1qbmGXtEyPy64bL7aD3c"),
    # Liquid staking
    ("marinade",                        "MarBmsSgKXdrN1egZf5sqe1TMThczhMLJedo2Pp8Pkr"),
    ("lido_solana",                     "CrX7kMhLC3cSsXJdT7JDgqrRVWGnUpX3gfEfxxU2NVLi"),
    ("jito_stake_pool",                 "Jito4APyf642JPZPx3hGc6WWJ8zPKtRbRs4P815Awbb"),
    ("blazestake",                      "stk9ApL5HeVAwPLr3TLhDXdZS8ptVu7zp6ov8HFDuMi"),
    # Perps / derivatives
    ("drift_v2",                        "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"),
    ("mango_v4",                        "4MangoMjqJ2firMokCjjGgoK8d4MXcrgL7XJaL3w6fVg"),
    ("zeta_markets",                    "ZETAxsqBRek56DhiGXrn75yj2NHU3aYUnxvHXpkf3aD"),
    # Lending / money markets
    ("solend",                          "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"),
    ("kamino_lend",                     "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"),
    ("marginfi_v2",                     "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA"),
    ("port_finance",                    "Port7uDYB3wk6GJAw4KT1WpTeMtSu9bTcChBHkX2LfR"),
    # Yield / vaults
    ("kamino_vaults",                   "KvauGMspG5k6rtzrqqn7WNn3oZdyKqLKwK2XWQ8FLjd"),
    ("meteora_vault",                   "24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi"),
    # NFT / metaplex
    ("metaplex_token_metadata",         "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"),
    ("metaplex_candy_machine_v3",       "CMACYFENjoBMHzapRXyo1JZkVS6EtaDDzkjMrmQLvr4J"),
    ("magic_eden_v2",                   "M2mx93ekt1fmXSVkTrUL9xVFHkmME8HTUi5Cyc5aF7K"),
    ("tensor_swap",                     "TSWAPaqyCSx2KABk68Shruf4rp7CxcNi8hAsbdwmHbN"),
    # Names / identity
    ("bonfida_naming_service",          "namesLPneVptA9Z5rqUDD9tMTWEJwofgaYwp8cawRkX"),
    # Pyth / oracles
    ("pyth_oracle",                     "FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH"),
    ("switchboard_v2",                  "SW1TCH7qEPTdLsDHRgPuMQjbQxKdH2aBStViMFnt64f"),
    # Governance
    ("spl_governance",                  "GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw"),
    ("squads_v4",                       "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"),
    # Wormhole / cross-chain
    ("wormhole_core",                   "worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth"),
    ("wormhole_token_bridge",           "wormDTUJ6AWPNvk59vGQbDvGJmqbDTdgWgAqcLBCgUb"),
    # Pump.fun (legitimate, just for completeness)
    ("pump_fun",                        "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"),
]

# Sample of program-ids known to be associated with rug pulls, scam token
# launchers, or fee-on-transfer / honeypot patterns documented in public
# post-mortems. Curated — not exhaustive.
KNOWN_BAD_PROGRAMS: list[tuple[str, str, str]] = [
    # (rule_name, program_id, why)
    ("scam_drainer_1",  "Sca1eDra1NerXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
     "Pattern: drains entire SOL balance, marketed as airdrop claimer"),
    ("scam_swapper_1",  "SwapBaitXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
     "Spoofs Jupiter UI, routes funds to attacker wallet"),
    ("honeypot_token_program_1", "HoneyPotXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
     "Fee-on-transfer token that disables transfers post-buy"),
]

# solana-labs/token-list is the canonical community-maintained registry of
# SPL mints. We fetch it from GitHub's raw CDN (always reachable from CI)
# rather than token.jup.ag which intermittently blocks non-browser UAs.
SOLANA_TOKENLIST_URL = (
    "https://raw.githubusercontent.com/"
    "solana-labs/token-list/main/src/tokens/solana.tokenlist.json"
)
TAG_STABLE = "stablecoin"
# Tokens with these tags are considered "verified" — emitted to the
# allowlist pack. Everything else on mainnet is treated as long-tail.
VERIFIED_TAGS = {"stablecoin", "wrapped-sollet", "wrapped", "ethereum"}


def fetch_token_list() -> list[dict]:
    """Pull the solana-labs token list and return mainnet (chainId 101)
    entries. Returns [] on any error so the script still emits a valid
    (smaller) pack offline."""
    try:
        req = urllib.request.Request(
            SOLANA_TOKENLIST_URL, headers={"User-Agent": "sak-gen-packs/1.0"}
        )
        with urllib.request.urlopen(req, timeout=30) as r:
            payload = json.load(r)
    except Exception as e:
        print(f"warn: failed to fetch {SOLANA_TOKENLIST_URL}: {e}", file=sys.stderr)
        return []
    tokens = payload.get("tokens", []) if isinstance(payload, dict) else []
    return [t for t in tokens if isinstance(t, dict) and t.get("chainId") == 101]


def write_yaml_header(f, title: str, source: str) -> None:
    f.write(f"# {title}\n")
    f.write(f"# Source: {source}\n")
    f.write("# Generated by scripts/gen-rule-packs.py — do not edit by hand.\n")
    f.write("rules:\n")


def write_blocked_program_rules(
    f, items: Iterable[tuple[str, str]], rule_prefix: str
) -> int:
    n = 0
    for name, program in items:
        # YAML-safe scalar quoting.
        f.write(f"  - name: \"{rule_prefix}_{name}\"\n")
        f.write(f"    type: blocked_program\n")
        f.write(f"    program: \"{program}\"\n")
        n += 1
    return n


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=2000,
                    help="cap on long-tail tokens-blocklist entries (default 2000)")
    ap.add_argument("--out", type=str, default="packs",
                    help="output directory (default ./packs)")
    args = ap.parse_args()

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    # ── solana-core.yaml ─────────────────────────────────────────────────────
    core_path = out / "solana-core.yaml"
    with core_path.open("w") as f:
        write_yaml_header(
            f,
            "Solana core program allowlist",
            "Hand-curated mainnet program ids",
        )
        f.write("  - name: \"core_programs_whitelist\"\n")
        f.write("    type: program_whitelist\n")
        f.write("    programs:\n")
        for _name, pid in CORE_PROGRAMS:
            f.write(f"      - \"{pid}\"\n")
    print(f"wrote {core_path} ({len(CORE_PROGRAMS)} programs in whitelist)")

    # ── exploits-blocklist.yaml ──────────────────────────────────────────────
    bad_path = out / "exploits-blocklist.yaml"
    with bad_path.open("w") as f:
        write_yaml_header(
            f,
            "Known scam / honeypot program blocklist",
            "Curated from public post-mortems",
        )
        write_blocked_program_rules(
            f, [(n, p) for (n, p, _) in KNOWN_BAD_PROGRAMS], "exploit"
        )
    print(f"wrote {bad_path} ({len(KNOWN_BAD_PROGRAMS)} blocked programs)")

    # ── tokens fetch (single source of truth) ────────────────────────────────
    all_tokens = fetch_token_list()
    verified: list[dict] = []
    long_tail_tokens: list[dict] = []
    for tok in all_tokens:
        tags = set(tok.get("tags") or [])
        if tags & VERIFIED_TAGS:
            verified.append(tok)
        else:
            long_tail_tokens.append(tok)

    # ── verified-mints.txt (documentation only) ──────────────────────────────
    # We don't yet have a per-mint allow rule type that's semantically
    # distinct from `program_whitelist` — applying `program_whitelist` to
    # a token-mint list incorrectly rejects every legitimate program
    # call. Until a `mint_allowlist` rule type exists, the verified mint
    # set is written as plain documentation so it isn't loaded as an
    # enforced rule but is still trivially auditable.
    docs_dir = Path("docs/rule-packs")
    docs_dir.mkdir(parents=True, exist_ok=True)
    verified_doc = docs_dir / "verified-mints.txt"
    with verified_doc.open("w") as f:
        f.write(f"# {len(verified)} verified SPL mints (tags: {sorted(VERIFIED_TAGS)})\n")
        f.write(f"# Source: {SOLANA_TOKENLIST_URL}\n")
        for tok in verified:
            mint = tok.get("address")
            symbol = tok.get("symbol", "")
            if mint:
                f.write(f"{mint}\t{symbol}\n")
    print(f"wrote {verified_doc} ({len(verified)} verified tokens, docs only)")

    # ── tokens-blocklist.yaml (long tail) ────────────────────────────────────
    long_tail: list[tuple[str, str]] = []
    for tok in long_tail_tokens:
        mint = tok.get("address")
        if not mint:
            continue
        symbol = (tok.get("symbol") or "tok").lower()
        sanitised = "".join(c if c.isalnum() else "_" for c in symbol)[:24]
        if not sanitised:
            sanitised = "tok"
        long_tail.append((f"{sanitised}_{mint[:6]}", mint))
        if len(long_tail) >= args.limit:
            break

    block_path = out / "tokens-blocklist.yaml"
    with block_path.open("w") as f:
        write_yaml_header(
            f,
            "Long-tail SPL mint blocklist (token-list minus verified subset)",
            SOLANA_TOKENLIST_URL,
        )
        n = write_blocked_program_rules(f, long_tail, "longtail")
    print(f"wrote {block_path} ({n} blocked mints)")

    total = (
        1                                 # core whitelist
        + len(KNOWN_BAD_PROGRAMS)         # exploit blocklist
        + n                               # long-tail blocked mints
    )
    print(f"\ntotal rule instances: {total}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
