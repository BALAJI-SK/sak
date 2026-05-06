Read the full audit in the document I just shared. 
Read every file listed before touching anything.
Implement all fixes in this exact order. 
Do not move to the next fix until the current one compiles.

---

FIX 1 — crates/sak-guardian/src/lib.rs (30 min)
PROBLEM: evaluate() hardcodes TxMeta::default(), 
discarding actual slippage from the transaction.

CHANGE: Add meta: &TxMeta parameter to evaluate():

// Before:
pub fn evaluate(&mut self, tx: &VersionedTransaction) -> Decision

// After:
pub fn evaluate(
    &mut self, 
    tx: &VersionedTransaction, 
    meta: &TxMeta
) -> Decision

Remove the line: let meta = TxMeta::default();
Pass the meta parameter through to evaluate(&self.rules, &view, meta)

Update all callers of evaluate() to pass meta.
Update kernel.submit() in sak-sdk to accept and pass meta too.

---

FIX 2 — crates/sak-guardian/src/simulator.rs (45 min)
PROBLEM: pre_accounts is cleared to vec![] BEFORE 
simulation runs, so pre_balances is always empty 
and DrainCheck never fires.

CHANGE: Snapshot accounts BEFORE clearing:

pub fn simulate(
    &mut self, 
    tx: &VersionedTransaction
) -> Result<SimulationResult, String> {
    // Step 1: snapshot pre-state from tx account keys
    let msg = tx.message.as_legacy_message()
        .ok_or_else(|| "not a legacy tx".to_string())?;
    
    self.pre_accounts = msg.account_keys
        .iter()
        .filter_map(|key| {
            self.svm.get_account(key).map(|acc| (*key, acc))
        })
        .collect();
    
    // Step 2: now run simulation
    let result = self.svm.simulate_transaction(tx.clone());
    // ... rest of existing code unchanged
}

---

FIX 3 — crates/sak-guardian/src/evaluator.rs (1 hour)
PROBLEM: TxView::from_sim_result hardcodes 
instructions: vec![], so ProgramWhitelist, 
ComputeUnitsCheck, SlippageCheck never fire 
on the Simulated path.

CHANGE: Replace from_sim_result with from_tx_and_sim 
that takes both the original tx AND the sim result:

pub fn from_tx_and_sim(
    tx: &VersionedTransaction, 
    sim: &SimulationResult
) -> Self {
    let msg = tx.message.as_legacy_message()
        .expect("legacy message required");
    
    let account_keys: Vec<String> = msg.account_keys
        .iter()
        .map(|k| k.to_string())
        .collect();
    
    let instructions: Vec<(u8, Vec<u8>)> = msg.instructions
        .iter()
        .map(|ix| (ix.program_id_index, ix.data.clone()))
        .collect();
    
    TxView::Simulated {
        account_keys,
        instructions,
        pre_balances: sim.pre_balances.clone(),
        post_balances: sim.post_balances.clone(),
        logs: sim.logs.clone(),
    }
}

Update lib.rs to call from_tx_and_sim(tx, &sim) 
instead of from_sim_result(&sim).

---

FIX 4 — crates/sak-guardian/src/lib.rs (15 min)
PROBLEM: When LiteSVM rejects a transaction outright,
the rule name shows as "simulation_failed" which 
looks like an internal crash to judges.

CHANGE: Rename to human-readable rule name:

// Before:
Err(e) => Decision::Reject {
    rule: "simulation_failed".into(),
    reason: format!("simulation error: {}", e),
}

// After:
Err(e) => Decision::Reject {
    rule: "pre_sign_simulation".into(),
    reason: format!(
        "transaction would fail on-chain: {}", e
    ),
}

---

FIX 5 — demo/race-server/src/main.rs (30 min)
THREE sub-fixes needed:

Sub-fix A: Pass actual meta to guardian.evaluate()
The TxFactory::generate() already creates meta with 
real slippage_bps. Pass it through:

// Before:
let decision = guardian.evaluate(&tx);

// After:
let decision = guardian.evaluate(&tx, &meta);

Sub-fix B: Add a real unknown program evil case.
Currently "Unknown program" generates a system 
transfer — a lie. Add a case that creates an 
instruction with a non-whitelisted program ID:

// Add this evil case:
EvilPattern::UnknownProgram => {
    let fake_program = Pubkey::new_unique();
    let ix = Instruction {
        program_id: fake_program,  // not in whitelist
        accounts: vec![],
        data: vec![1, 2, 3],
    };
    (1_000, Some(9900), "Unknown program", ix)
}

Sub-fix C: Expand evil patterns from 4 to at least 8.
Add these cases to TxFactory::generate():
1. u64::MAX lamports (already works via simulation_failed)
2. 99% slippage (now works after Fix 1)
3. Drain balance (now works after Fix 2)
4. Unknown program (fixed in Sub-fix B)
5. Excessive compute units (>500_000)
6. Zero amount transfer (dust attack)
7. Excessive priority fee (>100_000 microlamports)
8. Valid transaction (should pass — control case)

Mix them randomly so the UI shows 
a realistic blocked/allowed distribution 
(roughly 70% blocked, 30% allowed).

---

VERIFICATION — After all 5 fixes:

Run: cargo test --workspace
Expected: 20/20 still passing (evaluate_raw path 
is unchanged, tests should not break)

Run: cargo run -p race-server
In separate terminal: cd demo/race-ui && npm run dev
Open: http://localhost:3000

Verify this exact distribution in the UI:
✅ "99% slippage swap"     → BLOCKED  rule: max_slippage
✅ "Drain balance"         → BLOCKED  rule: max_account_drain  
✅ "Unknown program"       → BLOCKED  rule: allowed_programs
✅ "Excessive compute"     → BLOCKED  rule: compute_units_check
✅ "Valid transfer"        → ALLOWED  (green)
✅ "u64::MAX transfer"     → BLOCKED  rule: pre_sign_simulation

If any evil transaction shows ALLOWED, 
stop and report which fix is incomplete 
before continuing.

---

DO NOT touch:
- sak-reflex (fake Geyser is fine for demo — 
  don't waste time on real gRPC)
- sak-state (in-memory HashMap is fine for demo)
- The 20 evil corpus unit tests (leave them passing)
- The UI code (it works)

After fixes are verified working, report:
1. Full output of cargo test --workspace
2. Screenshot description of what localhost:3000 shows
3. Any remaining warnings that could confuse judges