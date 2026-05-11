use sak_core::{Decision, TxMeta};
use sak_guardian::Guardian;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use solana_signer::Signer;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use std::io::{self, Write};
use std::time::Instant;
use chrono::Utc;

/// ComputeBudget program ID
const COMPUTE_BUDGET_ID: &str = "ComputeBudget111111111111111111111111111111";

/// Create SetComputeUnitLimit instruction (discriminator = 0x02)
fn set_compute_unit_limit(units: u32) -> Instruction {
    Instruction {
        program_id: COMPUTE_BUDGET_ID.parse().unwrap(),
        accounts: vec![],
        data: vec![
            0x02, // discriminator
            (units & 0xFF) as u8,
            ((units >> 8) & 0xFF) as u8,
            ((units >> 16) & 0xFF) as u8,
            ((units >> 24) & 0xFF) as u8,
        ],
    }
}

/// Create SetComputeUnitPrice instruction (discriminator = 0x03)
fn set_compute_unit_price(microlamports: u64) -> Instruction {
    Instruction {
        program_id: COMPUTE_BUDGET_ID.parse().unwrap(),
        accounts: vec![],
        data: vec![
            0x03, // discriminator
            (microlamports & 0xFF) as u8,
            ((microlamports >> 8) & 0xFF) as u8,
            ((microlamports >> 16) & 0xFF) as u8,
            ((microlamports >> 24) & 0xFF) as u8,
            ((microlamports >> 32) & 0xFF) as u8,
            ((microlamports >> 40) & 0xFF) as u8,
            ((microlamports >> 48) & 0xFF) as u8,
            ((microlamports >> 56) & 0xFF) as u8,
        ],
    }
}

#[derive(Debug, Clone, Copy)]
enum TxPattern {
    // Evil patterns (blocked) — 70%
    Slippage99,
    DrainBalance,
    UnknownProgram,
    ExcessiveCompute,
    ZeroAmount,
    ExcessivePriorityFee,

    // Valid patterns (allowed) — 30%
    ValidSwap,
    ValidTransfer,
}

struct TxFactory {
    svm: litesvm::LiteSVM,
    payer: Keypair,
    counter: u64,
}

impl TxFactory {
    fn new() -> Self {
        let mut svm = litesvm::LiteSVM::new();
        let payer = Keypair::new();
        // Airdrop 10 SOL to payer
        if let Err(e) = svm.airdrop(&payer.pubkey(), 10_000_000_000) {
            eprintln!("Airdrop failed: {:?}", e);
        }
        eprintln!("TxFactory created. Payer: {} Balance: {} lamports",
                  payer.pubkey(),
                  svm.get_balance(&payer.pubkey()).unwrap_or(0));
        Self { svm, payer, counter: 0 }
    }

    /// Generate pattern with ~70% evil, ~30% valid
    fn next_pattern(&mut self) -> TxPattern {
        self.counter += 1;
        match self.counter % 10 {
            0 | 3 | 7 => TxPattern::ValidSwap,    // 30% allowed
            1 => TxPattern::Slippage99,
            2 => TxPattern::DrainBalance,
            4 => TxPattern::UnknownProgram,
            5 => TxPattern::ExcessiveCompute,
            6 => TxPattern::ZeroAmount,
            8 => TxPattern::ValidTransfer,
            _ => TxPattern::ExcessivePriorityFee,
        }
    }

    fn generate(&self, pattern: TxPattern) -> (Transaction, TxMeta, String, String, String, Option<f64>) {
        let recipient = Address::new_unique();
        let start_balance = self.svm.get_balance(&self.payer.pubkey()).unwrap_or(0);

        match pattern {
            TxPattern::Slippage99 => {
                // High slippage to trigger max_slippage rule
                let ix = transfer(&self.payer.pubkey(), &recipient, 1_000_000);
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: Some(9900), // 99% slippage
                    description: Some("99% Slippage Swap".into()),
                };
                let attack_type = "99% Slippage Swap".to_string();
                let description = "Agent tried to swap 0.01 SOL with 99% slippage tolerance".to_string();
                let severity = "critical".to_string();
                let simulated_loss_usd = Some(49.86); // ~$49.86 potential loss
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }

            TxPattern::DrainBalance => {
                // Transfer near-entire balance to trigger max_account_drain
                let drain_amount = start_balance.saturating_sub(5_000_000); // Leave 0.005 SOL
                let ix = transfer(&self.payer.pubkey(), &recipient, drain_amount);
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: None,
                    description: Some("Drain Entire Balance".into()),
                };
                let attack_type = "Drain Balance".to_string();
                let description = "Agent tried to drain entire wallet in single transfer".to_string();
                let severity = "critical".to_string();
                let simulated_loss_usd = Some(498.50); // Full wallet ~$498.50
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }

            TxPattern::UnknownProgram => {
                // Use a random program ID to trigger allowed_programs rule
                let unknown_program = Address::new_unique();
                let ix = Instruction {
                    program_id: unknown_program,
                    accounts: vec![
                        AccountMeta::new(self.payer.pubkey(), true),
                        AccountMeta::new(recipient, false),
                    ],
                    data: vec![0x01, 0x02, 0x03], // Arbitrary data
                };
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: None,
                    description: Some("Unknown Program Invocation".into()),
                };
                let attack_type = "Unknown Program".to_string();
                let description = "Agent tried to invoke unwhitelisted program".to_string();
                let severity = "high".to_string();
                let simulated_loss_usd = None;
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }

            TxPattern::ExcessiveCompute => {
                // Set compute unit limit > 1,400,000 to trigger max_compute_units
                let cu_ix = set_compute_unit_limit(500_000); // Will be rejected by rule (max 1,400,000 in rules.yaml)
                let ix = transfer(&self.payer.pubkey(), &recipient, 1_000);
                let msg = Message::new(&[cu_ix, ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: None,
                    description: Some("Excessive Compute Units".into()),
                };
                let attack_type = "Excessive Compute".to_string();
                let description = "Agent tried to set compute unit limit to 500,000".to_string();
                let severity = "medium".to_string();
                let simulated_loss_usd = None;
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }

            TxPattern::ZeroAmount => {
                // Zero transfer to trigger min_transfer_value rule
                let ix = transfer(&self.payer.pubkey(), &recipient, 0);
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: None,
                    description: Some("Zero Amount Transfer".into()),
                };
                let attack_type = "Zero Amount".to_string();
                let description = "Agent tried to transfer 0 lamports".to_string();
                let severity = "low".to_string();
                let simulated_loss_usd = None;
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }

            TxPattern::ExcessivePriorityFee => {
                // Set priority fee > 1,000,000 microlamports to trigger max_priority_fee
                let price_ix = set_compute_unit_price(2_000_000); // Exceeds max of 1,000,000
                let ix = transfer(&self.payer.pubkey(), &recipient, 1_000);
                let msg = Message::new(&[price_ix, ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: None,
                    description: Some("Excessive Priority Fee".into()),
                };
                let attack_type = "Excessive Priority Fee".to_string();
                let description = "Agent tried to set priority fee to 2,000,000 microlamports".to_string();
                let severity = "medium".to_string();
                let simulated_loss_usd = None;
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }

            TxPattern::ValidSwap => {
                // Valid swap with reasonable slippage
                let ix = transfer(&self.payer.pubkey(), &recipient, 500_000); // 0.0005 SOL
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: Some(100), // 1% slippage - within limits
                    description: Some("Valid Swap".into()),
                };
                let attack_type = "Valid Swap".to_string();
                let description = "Agent executed valid swap with 1% slippage tolerance".to_string();
                let severity = "none".to_string();
                let simulated_loss_usd = None;
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }

            TxPattern::ValidTransfer => {
                // Valid transfer within balance limits
                let ix = transfer(&self.payer.pubkey(), &recipient, 100_000); // 0.0001 SOL
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: Some(50), // 0.5% slippage
                    description: Some("Valid Transfer".into()),
                };
                let attack_type = "Valid Transfer".to_string();
                let description = "Agent executed valid transfer of 0.0001 SOL".to_string();
                let severity = "none".to_string();
                let simulated_loss_usd = None;
                (tx, meta, attack_type, description, severity, simulated_loss_usd)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut factory = TxFactory::new();

    // Create Guardian WITH the same SVM as TxFactory
    let mut guardian = {
        let mut svm = litesvm::LiteSVM::new();
        // Copy the payer account to the new SVM
        if let Some(account) = factory.svm.get_account(&factory.payer.pubkey()) {
            let _ = svm.set_account(factory.payer.pubkey(), account);
        }
        Guardian::from_yaml_with_svm("rules.yaml", svm)
            .expect("Failed to load rules.yaml")
    };

    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(2));

    tracing::info!("Transaction generator started - sending to stdout");

    loop {
        ticker.tick().await;

        let pattern = factory.next_pattern();
        let (tx, meta, attack_type, description, severity, simulated_loss_usd) = factory.generate(pattern);

        let start_time = Instant::now();
        let decision = guardian.evaluate(&tx.into(), &meta);
        let simulation_time_ms = start_time.elapsed().as_millis() as u64;

        let (decision_str, rule, reason) = match &decision {
            Decision::Allow => ("allowed", None, None),
            Decision::Reject { rule, reason } => ("rejected", Some(rule.clone()), Some(reason.clone())),
        };

        let log_entry = serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "timestamp": Utc::now().to_rfc3339(),
            "decision": decision_str,
            "rule": rule,
            "reason": reason,
            "attack_type": attack_type,
            "description": description,
            "severity": severity,
            "simulated_loss_usd": simulated_loss_usd,
            "simulation_time_ms": simulation_time_ms,
        });

        let line = log_entry.to_string();
        // Use write! + flush so a broken pipe (parent closed) exits cleanly instead of panicking.
        let mut out = io::stdout();
        if writeln!(out, "{}", line).is_err() || out.flush().is_err() {
            break;
        }
    }
}
