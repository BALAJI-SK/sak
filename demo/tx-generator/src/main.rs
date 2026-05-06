use rand::Rng;
use sak_core::{Decision, TxMeta};
use sak_guardian::Guardian;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use solana_signer::Signer;
use solana_address::Address;
use std::str::FromStr;
use tokio::time::{interval, Duration};

struct TxFactory {
    svm: litesvm::LiteSVM,
    payer: Keypair,
}

impl TxFactory {
    fn new() -> Self {
        let svm = litesvm::LiteSVM::new();
        let payer = Keypair::new();
        // Note: In production, properly fund the payer
        // For demo, we'll handle simulation failures gracefully
        Self { svm, payer }
    }

    /// Generate a malicious transaction (evil corpus pattern)
    fn evil_tx(&self, pattern: usize) -> (Transaction, TxMeta) {
        let recipient = Address::new_unique();

        match pattern % 20 {
            0 => {
                // 99% slippage
                let ix = transfer(&self.payer.pubkey(), &recipient, 1_000);
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                let meta = TxMeta {
                    slippage_bps: Some(9900),
                    description: Some("99% slippage swap".into()),
                };
                (tx, meta)
            }
            1 => {
                // Drain entire balance
                let ix = transfer(&self.payer.pubkey(), &recipient, u64::MAX);
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                (tx, TxMeta {
                    description: Some("Drain entire balance".into()),
                    ..Default::default()
                })
            }
            2 => {
                // Unknown program
                let fake_program = Address::from_str("FaKe1111111111111111111111111111111111111111").unwrap();
                let ix = solana_instruction::Instruction {
                    program_id: fake_program,
                    accounts: vec![],
                    data: vec![],
                };
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                (tx, TxMeta {
                    description: Some("Unknown program".into()),
                    ..Default::default()
                })
            }
            _ => {
                // Default: simple transfer (valid)
                let ix = transfer(&self.payer.pubkey(), &recipient, 1_000);
                let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
                let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
                (tx, TxMeta {
                    description: Some("Valid transfer".into()),
                    ..Default::default()
                })
            }
        }
    }

    /// Generate a valid transaction
    fn valid_tx(&self) -> (Transaction, TxMeta) {
        let recipient = Address::new_unique();
        let ix = transfer(&self.payer.pubkey(), &recipient, 500_000);
        let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
        let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
        (tx, TxMeta {
            slippage_bps: Some(100), // 1% slippage - within limits
            description: Some("Valid transfer 0.005 SOL".into()),
        })
    }

    fn random_tx(&self) -> (Transaction, TxMeta) {
        let mut rng = rand::thread_rng();
        if rng.gen_bool(0.7) {
            self.evil_tx(rng.gen_range(0..20))
        } else {
            self.valid_tx()
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut guardian = Guardian::from_yaml("rules.yaml").expect("Failed to load rules.yaml");
    let factory = TxFactory::new();
    let mut ticker = interval(Duration::from_secs(2));

    tracing::info!("Transaction generator started - sending to stdout");

    loop {
        ticker.tick().await;

        let (tx, meta) = factory.random_tx();
        let decision = guardian.evaluate(&tx.into());

        let log_entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "decision": match &decision {
                Decision::Allow => "allowed",
                Decision::Reject { .. } => "rejected",
            },
            "rule": match &decision {
                Decision::Reject { rule, .. } => Some(rule),
                _ => None,
            },
            "reason": match &decision {
                Decision::Reject { reason, .. } => Some(reason),
                _ => None,
            },
            "description": meta.description,
        });

        println!("{}", log_entry.to_string());
    }
}
