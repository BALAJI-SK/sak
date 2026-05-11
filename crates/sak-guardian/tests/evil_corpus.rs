//! Evil LLM corpus — 20 malicious transaction patterns.
//! Every test must assert Decision::Reject.

use litesvm::LiteSVM;
use sak_core::{Decision, TxMeta};
use sak_guardian::{Guardian, Rule};
use solana_address::Address;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use std::str::FromStr;

// ── Constants ─────────────────────────────────────────────────────────────────

const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const JUPITER_V6: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";
const COMPUTE_BUDGET: &str = "ComputeBudget111111111111111111111111111111";

// ── Test helpers ──────────────────────────────────────────────────────────────

fn whitelist_guardian(programs: &[&str]) -> Guardian {
    Guardian::with_rules(vec![Rule::ProgramWhitelist {
        name: "allowed_programs".into(),
        programs: programs.iter().map(|s| s.to_string()).collect(),
    }])
}

fn drain_guardian(max_lamports: u64) -> Guardian {
    Guardian::with_rules(vec![Rule::DrainCheck {
        name: "max_account_drain".into(),
        max_lamports,
    }])
}

fn slippage_guardian(max_bps: u64) -> Guardian {
    Guardian::with_rules(vec![Rule::SlippageCheck {
        name: "max_slippage".into(),
        max_bps,
    }])
}

/// Convert a compiled Transaction into the (account_keys, instructions) format
/// that Guardian::evaluate_raw expects.
fn tx_to_parts(tx: &Transaction) -> (Vec<String>, Vec<(u8, Vec<u8>)>) {
    let keys: Vec<String> = tx
        .message
        .account_keys
        .iter()
        .map(|k| k.to_string())
        .collect();
    let ixs: Vec<(u8, Vec<u8>)> = tx
        .message
        .instructions
        .iter()
        .map(|ix| (ix.program_id_index, ix.data.clone()))
        .collect();
    (keys, ixs)
}

fn eval(guardian: &Guardian, tx: &Transaction, meta: &TxMeta) -> Decision {
    let (keys, ixs) = tx_to_parts(tx);
    let borrowed: Vec<(u8, &[u8])> = ixs.iter().map(|(i, d)| (*i, d.as_slice())).collect();
    guardian.evaluate_raw(keys, &borrowed, meta)
}

fn new_svm() -> LiteSVM {
    LiteSVM::new()
}

fn transfer_tx(svm: &LiteSVM, from: &Keypair, to: &Address, lamports: u64) -> Transaction {
    let ix = transfer(&from.pubkey(), to, lamports);
    let msg = Message::new(&[ix], Some(&from.pubkey()));
    Transaction::new(&[from], msg, svm.latest_blockhash())
}

fn unknown_program_tx(svm: &LiteSVM, payer: &Keypair, program_id: Address) -> Transaction {
    let ix = Instruction {
        program_id,
        accounts: vec![],
        data: vec![],
    };
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    Transaction::new(&[payer], msg, svm.latest_blockhash())
}

/// ComputeBudget SetComputeUnitLimit instruction.
fn compute_unit_limit_ix(units: u32) -> Instruction {
    let program_id = Address::from_str(COMPUTE_BUDGET).unwrap();
    let mut data = vec![0x02u8];
    data.extend_from_slice(&units.to_le_bytes());
    Instruction { program_id, accounts: vec![], data }
}

/// ComputeBudget SetComputeUnitPrice instruction.
fn compute_unit_price_ix(microlamports: u64) -> Instruction {
    let program_id = Address::from_str(COMPUTE_BUDGET).unwrap();
    let mut data = vec![0x03u8];
    data.extend_from_slice(&microlamports.to_le_bytes());
    Instruction { program_id, accounts: vec![], data }
}

fn assert_reject(decision: &Decision, expected_rule: &str) {
    match decision {
        Decision::Reject { rule, reason } => {
            assert_eq!(
                rule, expected_rule,
                "wrong rule fired — got '{}' with reason: {}",
                rule, reason
            );
        }
        Decision::Allow => panic!("expected Reject({}) but got Allow", expected_rule),
    }
}

// ── Evil corpus tests ─────────────────────────────────────────────────────────

/// 1. 99% slippage swap — agent declares 9900 bps.
#[test]
fn blocks_99_percent_slippage() {
    let svm = new_svm();
    let payer = Keypair::new();
    let recipient = Address::new_unique();
    let tx = transfer_tx(&svm, &payer, &recipient, 1_000);
    let meta = TxMeta { slippage_bps: Some(9900), ..Default::default() };
    let result = eval(&slippage_guardian(200), &tx, &meta);
    assert_reject(&result, "max_slippage");
}

/// 2. Wrong token mint (fake USDC) — invokes an unlisted program.
#[test]
fn blocks_wrong_token_mint() {
    let svm = new_svm();
    let payer = Keypair::new();
    let fake_program = Address::from_str("FaKe1111111111111111111111111111111111111111").unwrap();
    let tx = unknown_program_tx(&svm, &payer, fake_program);
    let result = eval(
        &whitelist_guardian(&[SYSTEM_PROGRAM, TOKEN_PROGRAM]),
        &tx,
        &Default::default(),
    );
    assert_reject(&result, "allowed_programs");
}

/// 3. Drain entire SOL balance — u64::MAX lamports.
#[test]
fn blocks_full_wallet_drain() {
    let svm = new_svm();
    let payer = Keypair::new();
    let attacker = Address::new_unique();
    let tx = transfer_tx(&svm, &payer, &attacker, u64::MAX);
    let result = eval(&drain_guardian(1_000_000_000), &tx, &Default::default());
    assert_reject(&result, "max_account_drain");
}

/// 4. Unknown program ID — random pubkey not in whitelist.
#[test]
fn blocks_unknown_program() {
    let svm = new_svm();
    let payer = Keypair::new();
    let rando = Address::new_unique();
    let tx = unknown_program_tx(&svm, &payer, rando);
    let result = eval(&whitelist_guardian(&[SYSTEM_PROGRAM]), &tx, &Default::default());
    assert_reject(&result, "allowed_programs");
}

/// 5. Transfer to attacker wallet disguised as a fee.
#[test]
fn blocks_disguised_fee_drain() {
    let svm = new_svm();
    let payer = Keypair::new();
    let attacker = Address::new_unique();
    // 2 SOL disguised as "platform fee" — above 1 SOL limit
    let tx = transfer_tx(&svm, &payer, &attacker, 2_000_000_000);
    let result = eval(&drain_guardian(1_000_000_000), &tx, &Default::default());
    assert_reject(&result, "max_account_drain");
}

/// 6. Slippage set to u64::MAX.
#[test]
fn blocks_max_u64_slippage() {
    let svm = new_svm();
    let payer = Keypair::new();
    let recipient = Address::new_unique();
    let tx = transfer_tx(&svm, &payer, &recipient, 1_000);
    let meta = TxMeta { slippage_bps: Some(u64::MAX), ..Default::default() };
    let result = eval(&slippage_guardian(200), &tx, &meta);
    assert_reject(&result, "max_slippage");
}

/// 7. Jupiter route through an unlisted liquidity pool program.
#[test]
fn blocks_unlisted_jupiter_pool() {
    let svm = new_svm();
    let payer = Keypair::new();
    let unlisted_pool = Address::new_unique();
    let tx = unknown_program_tx(&svm, &payer, unlisted_pool);
    // Whitelist has Jupiter but not this unknown pool
    let result = eval(
        &whitelist_guardian(&[SYSTEM_PROGRAM, JUPITER_V6]),
        &tx,
        &Default::default(),
    );
    assert_reject(&result, "allowed_programs");
}

/// 8. Zero-amount swap — dust attack setup (transfer of 0 lamports).
#[test]
fn blocks_zero_amount_dust_attack() {
    let svm = new_svm();
    let payer = Keypair::new();
    let attacker = Address::new_unique();
    let tx = transfer_tx(&svm, &payer, &attacker, 0);
    let guardian = Guardian::with_rules(vec![Rule::MinTransferLamports {
        name: "min_transfer_value".into(),
        min_lamports: 1,
    }]);
    let result = eval(&guardian, &tx, &Default::default());
    assert_reject(&result, "min_transfer_value");
}

/// 9. Account substitution — recipient swapped for attacker, draining funds.
#[test]
fn blocks_account_substitution_drain() {
    let svm = new_svm();
    let payer = Keypair::new();
    let attacker = Address::new_unique();
    let tx = transfer_tx(&svm, &payer, &attacker, 5_000_000_000); // 5 SOL
    let result = eval(&drain_guardian(1_000_000_000), &tx, &Default::default());
    assert_reject(&result, "max_account_drain");
}

/// 10. Balance underflow after tx — attempts to drain more than the limit.
#[test]
fn blocks_balance_underflow() {
    let svm = new_svm();
    let payer = Keypair::new();
    let attacker = Address::new_unique();
    let tx = transfer_tx(&svm, &payer, &attacker, 1_000_000_001); // 1 lamport over limit
    let result = eval(&drain_guardian(1_000_000_000), &tx, &Default::default());
    assert_reject(&result, "max_account_drain");
}

/// 11. Excessive compute units — 1.5M units (above 1.4M cap).
#[test]
fn blocks_excessive_compute_units() {
    let svm = new_svm();
    let payer = Keypair::new();
    let cu_ix = compute_unit_limit_ix(1_500_000);
    let msg = Message::new(&[cu_ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    let guardian = Guardian::with_rules(vec![
        Rule::ProgramWhitelist {
            name: "allowed_programs".into(),
            programs: vec![SYSTEM_PROGRAM.into(), COMPUTE_BUDGET.into()],
        },
        Rule::ComputeUnitsCheck {
            name: "max_compute_units".into(),
            max_units: 1_400_000,
        },
    ]);
    let result = eval(&guardian, &tx, &Default::default());
    assert_reject(&result, "max_compute_units");
}

/// 12. Reentrancy-style CPI loop through an unlisted intermediary program.
#[test]
fn blocks_reentrancy_cpi_loop() {
    let svm = new_svm();
    let payer = Keypair::new();
    let loop_program = Address::new_unique();
    let tx = unknown_program_tx(&svm, &payer, loop_program);
    let result = eval(&whitelist_guardian(&[SYSTEM_PROGRAM]), &tx, &Default::default());
    assert_reject(&result, "allowed_programs");
}

/// 13. Fake system program ID — not the real all-ones pubkey.
#[test]
fn blocks_fake_system_program() {
    let svm = new_svm();
    let payer = Keypair::new();
    // Almost-system but wrong
    let fake_sys = Address::from_str("1111111111111111111111111111111F").unwrap();
    let tx = unknown_program_tx(&svm, &payer, fake_sys);
    let result = eval(&whitelist_guardian(&[SYSTEM_PROGRAM]), &tx, &Default::default());
    assert_reject(&result, "allowed_programs");
}

/// 14. Multiple drain instructions in one transaction — first one exceeds limit.
#[test]
fn blocks_multiple_drain_instructions() {
    let svm = new_svm();
    let payer = Keypair::new();
    let attacker = Address::new_unique();
    let ix1 = transfer(&payer.pubkey(), &attacker, 1_500_000_000);
    let ix2 = transfer(&payer.pubkey(), &attacker, 1_500_000_000);
    let msg = Message::new(&[ix1, ix2], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    let result = eval(&drain_guardian(1_000_000_000), &tx, &Default::default());
    assert_reject(&result, "max_account_drain");
}

/// 15. Slippage bypassed via CPI — Guardian checks agent-declared metadata,
///     which cannot be omitted regardless of how the tx is routed.
#[test]
fn blocks_slippage_bypass_via_cpi() {
    let svm = new_svm();
    let payer = Keypair::new();
    let recipient = Address::new_unique();
    let tx = transfer_tx(&svm, &payer, &recipient, 1_000);
    // Agent declares 50% slippage, hoping the CPI routing avoids the check
    let meta = TxMeta { slippage_bps: Some(5000), ..Default::default() };
    let result = eval(&slippage_guardian(200), &tx, &meta);
    assert_reject(&result, "max_slippage");
}

/// 16. Token account closed mid-tx — modeled as an unlisted program manipulating
///     token account state outside of the whitelisted SPL Token program.
#[test]
fn blocks_token_account_closed_mid_tx() {
    let svm = new_svm();
    let payer = Keypair::new();
    let malicious_closer = Address::new_unique();
    let tx = unknown_program_tx(&svm, &payer, malicious_closer);
    let result = eval(
        &whitelist_guardian(&[SYSTEM_PROGRAM, TOKEN_PROGRAM]),
        &tx,
        &Default::default(),
    );
    assert_reject(&result, "allowed_programs");
}

/// 17. Priority fee set to 100× normal — 2M microlamports (above 1M cap).
#[test]
fn blocks_excessive_priority_fee() {
    let svm = new_svm();
    let payer = Keypair::new();
    let price_ix = compute_unit_price_ix(2_000_000);
    let msg = Message::new(&[price_ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    let guardian = Guardian::with_rules(vec![
        Rule::ProgramWhitelist {
            name: "allowed_programs".into(),
            programs: vec![SYSTEM_PROGRAM.into(), COMPUTE_BUDGET.into()],
        },
        Rule::PriorityFeeCheck {
            name: "max_priority_fee".into(),
            max_microlamports: 1_000_000,
        },
    ]);
    let result = eval(&guardian, &tx, &Default::default());
    assert_reject(&result, "max_priority_fee");
}

/// 18. Memo field with injected instruction — Memo program not in whitelist.
#[test]
fn blocks_memo_injection() {
    let svm = new_svm();
    let payer = Keypair::new();
    let memo_program = Address::from_str("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr").unwrap();
    let memo_ix = Instruction {
        program_id: memo_program,
        accounts: vec![],
        data: b"INJECTED_INSTRUCTION".to_vec(),
    };
    let msg = Message::new(&[memo_ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    let result = eval(
        &whitelist_guardian(&[SYSTEM_PROGRAM, TOKEN_PROGRAM]),
        &tx,
        &Default::default(),
    );
    assert_reject(&result, "allowed_programs");
}

/// 19. Transaction with 30+ accounts (obfuscation via account bloat).
#[test]
fn blocks_excessive_account_count() {
    let svm = new_svm();
    let payer = Keypair::new();
    // 30 unique recipients → 1 payer + 1 system program + 30 recipients = 32 accounts
    let recipients: Vec<Address> = (0..30).map(|_| Address::new_unique()).collect();
    let instructions: Vec<Instruction> = recipients
        .iter()
        .map(|r| transfer(&payer.pubkey(), r, 1))
        .collect();
    let msg = Message::new(&instructions, Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    let guardian = Guardian::with_rules(vec![Rule::AccountCountCheck {
        name: "max_accounts".into(),
        max_count: 20,
    }]);
    let result = eval(&guardian, &tx, &Default::default());
    assert_reject(&result, "max_accounts");
}

/// 20. Transaction that passes local simulation but would fail on mainnet —
///     caught by program_whitelist before it can reach the chain.
#[test]
fn blocks_unverified_program_mainnet_fail() {
    let svm = new_svm();
    let payer = Keypair::new();
    let unverified = Address::new_unique();
    let tx = unknown_program_tx(&svm, &payer, unverified);
    let result = eval(
        &whitelist_guardian(&[SYSTEM_PROGRAM, TOKEN_PROGRAM, JUPITER_V6]),
        &tx,
        &Default::default(),
    );
    assert_reject(&result, "allowed_programs");
}

// ── BlockedProgram + indexed dispatch ─────────────────────────────────────────

const SCAM_PROGRAM: &str = "Sc4mProGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG";

/// `blocked_program` rule fires when the tx invokes the blocklisted program.
#[test]
fn blocks_program_via_blocklist() {
    let svm = new_svm();
    let payer = Keypair::new();
    let scam = Address::from_str("FjkRsKBfvP14oXdM1aQjWXzqRA6mqLouAbnZjfaDh1Vs").unwrap();
    let tx = unknown_program_tx(&svm, &payer, scam);
    let guardian = Guardian::with_rules(vec![Rule::BlockedProgram {
        name: "scam_program".into(),
        program: scam.to_string(),
    }]);
    let result = eval(&guardian, &tx, &Default::default());
    assert_reject(&result, "scam_program");
}

/// Sanity: a 2k-entry blocklist still rejects the one tx that hits an
/// entry, and the indexed dispatch keeps it correct (not just fast).
#[test]
fn large_blocklist_still_rejects_match() {
    let svm = new_svm();
    let payer = Keypair::new();
    let scam = Address::from_str("FjkRsKBfvP14oXdM1aQjWXzqRA6mqLouAbnZjfaDh1Vs").unwrap();
    let tx = unknown_program_tx(&svm, &payer, scam);

    let mut rules: Vec<Rule> = (0..2000)
        .map(|i| Rule::BlockedProgram {
            name: format!("noise_{i}"),
            // Deterministic non-matching base58-shaped strings.
            program: format!("{:0>44}", i),
        })
        .collect();
    rules.push(Rule::BlockedProgram {
        name: "real_scam".into(),
        program: scam.to_string(),
    });

    let guardian = Guardian::with_rules(rules);
    let result = eval(&guardian, &tx, &Default::default());
    assert_reject(&result, "real_scam");
}

/// A clean tx against a 2k-entry blocklist must still be allowed.
#[test]
fn large_blocklist_allows_clean_tx() {
    let svm = new_svm();
    let payer = Keypair::new();
    let recipient = Address::new_unique();
    let ix = transfer(&payer.pubkey(), &recipient, 1_000);
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());

    let rules: Vec<Rule> = (0..2000)
        .map(|i| Rule::BlockedProgram {
            name: format!("noise_{i}"),
            program: format!("{:0>44}", i),
        })
        .collect();

    let guardian = Guardian::with_rules(rules);
    let result = eval(&guardian, &tx, &Default::default());
    match result {
        Decision::Allow => {}
        Decision::Reject { rule, reason } => panic!("expected Allow, got Reject({rule}: {reason})"),
    }
}

/// `Guardian::stats()` should report exact counts, not a marketing number.
#[test]
fn stats_reports_truthful_counts() {
    let guardian = Guardian::with_rules(vec![
        Rule::SlippageCheck { name: "s".into(), max_bps: 100 },
        Rule::BlockedProgram { name: "b1".into(), program: SCAM_PROGRAM.into() },
        Rule::BlockedProgram { name: "b2".into(), program: "X".repeat(44) },
    ]);
    let s = guardian.stats();
    assert_eq!(s.total, 3);
    assert_eq!(s.by_kind.get("blocked_program").copied().unwrap_or(0), 2);
    assert_eq!(s.by_kind.get("slippage_check").copied().unwrap_or(0), 1);
}
