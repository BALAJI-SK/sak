use anyhow::Result;
use litesvm::LiteSVM;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use tracing::info;

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("Throwaway B — LiteSVM local transaction simulation");

    let mut svm = LiteSVM::new();

    let payer = Keypair::new();
    let recipient = Address::new_unique();

    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    let balance_before = svm.get_account(&payer.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    info!(
        pubkey = %payer.pubkey(),
        balance_before,
        "Payer funded"
    );

    let transfer_lamports = LAMPORTS_PER_SOL;
    let ix = transfer(&payer.pubkey(), &recipient, transfer_lamports);
    let tx = Transaction::new(
        &[&payer],
        Message::new(&[ix.clone()], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );

    // Simulate first
    match svm.simulate_transaction(tx.clone()) {
        Ok(sim) => {
            info!("Simulation OK");
            info!(logs = ?sim.meta.logs, "Simulation logs");
        }
        Err(e) => {
            info!(error = ?e, "Simulation REJECTED — tx would fail");
            return Ok(());
        }
    }

    // Execute
    match svm.send_transaction(tx) {
        Ok(meta) => {
            let balance_after = svm.get_account(&payer.pubkey())
                .map(|a| a.lamports)
                .unwrap_or(0);
            let recipient_balance = svm.get_account(&recipient)
                .map(|a| a.lamports)
                .unwrap_or(0);

            info!(logs = ?meta.logs, "Transaction landed");
            info!(
                balance_before,
                balance_after,
                delta = balance_before as i64 - balance_after as i64,
                "Payer balance change (lamports)"
            );
            info!(
                recipient_balance,
                sol = recipient_balance as f64 / LAMPORTS_PER_SOL as f64,
                "Recipient received"
            );
        }
        Err(e) => {
            info!(error = ?e, "Transaction failed");
        }
    }

    Ok(())
}
