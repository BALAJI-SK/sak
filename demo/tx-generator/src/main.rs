use rand::Rng;
use sak_core::{Decision, TxMeta};
use sak_guardian::Guardian;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use solana_signer::Signer;
use solana_address::Address;
use tokio::time::{interval, Duration};

struct TxFactory {
    svm: litesvm::LiteSVM,
    payer: Keypair,
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
        Self { svm, payer }
    }

    fn generate(&self) -> (Transaction, TxMeta) {
        let mut rng = rand::thread_rng();
        let recipient = Address::new_unique();

        if rng.gen_bool(0.7) {
            // Evil transaction (70%)
            let (lamports, slippage_bps, desc) = match rng.gen_range(0..5) {
                0 => (u64::MAX, None, "Drain entire balance"),
                1 => (1_000, Some(9900), "99% slippage swap"),
                2 => (5_000_000_000, None, "Disguised fee drain"),
                _ => (1_000, None, "Unknown program"),
            };

            let ix = transfer(&self.payer.pubkey(), &recipient, lamports);
            let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
            let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
            let meta = TxMeta {
                slippage_bps,
                description: Some(desc.into()),
                ..Default::default()
            };
            (tx, meta)
        } else {
            // Valid transaction (30%)
            let ix = transfer(&self.payer.pubkey(), &recipient, 500_000);
            let msg = Message::new(&[ix], Some(&self.payer.pubkey()));
            let tx = Transaction::new(&[&self.payer], msg, self.svm.latest_blockhash());
            let meta = TxMeta {
                slippage_bps: Some(100),
                description: Some("Valid transfer 0.005 SOL".into()),
                ..Default::default()
            };
            (tx, meta)
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let factory = TxFactory::new();
    
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
    
    let mut ticker = interval(Duration::from_secs(2));

    tracing::info!("Transaction generator started - sending to stdout");

    loop {
        ticker.tick().await;

        let (tx, meta) = factory.generate();
        let decision = guardian.evaluate(&tx.into(), &meta);

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
