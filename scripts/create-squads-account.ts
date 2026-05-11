import * as multisig from "@sqds/multisig";
import {
  Connection,
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { readFileSync, writeFileSync, existsSync } from "fs";
import { homedir } from "os";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const DEVNET_RPC =
  process.env.DEVNET_RPC || "https://api.devnet.solana.com";
const SQUADS_PROGRAM_ID = new PublicKey(
  "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
);

const __dirname = dirname(fileURLToPath(import.meta.url));
const GENERATED_KEYPAIR_PATH = join(__dirname, "generated-creator.json");

function loadKeypair(path: string): Keypair {
  const raw = readFileSync(path, "utf8");
  const arr = JSON.parse(raw) as number[];
  return Keypair.fromSecretKey(Uint8Array.from(arr));
}

function saveKeypair(keypair: Keypair): void {
  writeFileSync(
    GENERATED_KEYPAIR_PATH,
    JSON.stringify(Array.from(keypair.secretKey))
  );
  console.log(`  Saved generated keypair to: ${GENERATED_KEYPAIR_PATH}`);
}

function findDefaultKeypair(): Keypair | null {
  const candidates = [
    join(homedir(), ".config", "solana", "id.json"),
    join(homedir(), ".config", "solana", "id_ed25519.json"),
  ];
  for (const p of candidates) {
    if (existsSync(p)) {
      console.log(`Using keypair from: ${p}`);
      return loadKeypair(p);
    }
  }
  if (existsSync(GENERATED_KEYPAIR_PATH)) {
    console.log(`Using previously generated keypair: ${GENERATED_KEYPAIR_PATH}`);
    return loadKeypair(GENERATED_KEYPAIR_PATH);
  }
  return null;
}

function printFundingInstructions(creator: Keypair): void {
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("  FUNDING REQUIRED");
  console.log("");
  console.log(`  Fund this address with ~0.05 SOL on devnet:`);
  console.log(`    ${creator.publicKey.toBase58()}`);
  console.log("");
  console.log("  Options:");
  console.log("    solana airdrop 2 --url devnet");
  console.log("    https://faucet.solana.com/");
  console.log("    cargo install devnet-pow && devnet-pow mine -d 3 --reward 0.02");
  console.log("");
  console.log("  After funding, re-run this script.");
  console.log("  The generated keypair is saved — no need to");
  console.log("  pass any arguments.");
  console.log("");
  if (!existsSync(GENERATED_KEYPAIR_PATH)) {
    console.log("  Keypair saved to keep between runs:");
    console.log(`    ${GENERATED_KEYPAIR_PATH}`);
  }
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}

async function main() {
  const keypairArg = process.argv.find((a) => a.startsWith("--keypair="));
  let creator: Keypair;

  if (keypairArg) {
    const path = keypairArg.split("=")[1];
    creator = loadKeypair(path);
    console.log(`Creator loaded from: ${path}`);
  } else {
    const found = findDefaultKeypair();
    if (found) {
      creator = found;
    } else {
      console.log("No keypair found. Generating fresh keypair...");
      creator = Keypair.generate();
      console.log(`  Generated public key: ${creator.publicKey.toBase58()}`);
      saveKeypair(creator);
      printFundingInstructions(creator);
      process.exit(0);
    }
  }

  console.log(`Creator: ${creator.publicKey.toBase58()}`);
  console.log("");

  const connection = new Connection(DEVNET_RPC, "confirmed");

  const balance = await connection.getBalance(creator.publicKey);
  if (balance < 0.05 * LAMPORTS_PER_SOL) {
    console.log(`Balance: ${balance / LAMPORTS_PER_SOL} SOL (insufficient)`);
    printFundingInstructions(creator);
    process.exit(1);
  }
  console.log(`Balance: ${(balance / LAMPORTS_PER_SOL).toFixed(4)} SOL (sufficient)`);
  console.log("");

  const createKey = Keypair.generate();
  console.log(`Create key: ${createKey.publicKey.toBase58()}`);

  const [multisigPda] = multisig.getMultisigPda({
    createKey: createKey.publicKey,
    programId: SQUADS_PROGRAM_ID,
  });
  console.log(`Multisig PDA (derived): ${multisigPda.toBase58()}`);

  console.log("Fetching program config for treasury...");
  const [programConfigPda] = multisig.getProgramConfigPda({
    programId: SQUADS_PROGRAM_ID,
  });
  const programConfig =
    await multisig.accounts.ProgramConfig.fromAccountAddress(
      connection,
      programConfigPda
    );
  const treasury = programConfig.treasury;
  console.log(`  Treasury: ${treasury.toBase58()}`);
  console.log("");

  console.log("Creating multisig on devnet...");
  const txSig = await multisig.rpc.multisigCreateV2({
    connection,
    createKey: createKey,
    creator: creator,
    multisigPda,
    configAuthority: null,
    timeLock: 0,
    threshold: 1,
    members: [
      {
        key: creator.publicKey,
        permissions: multisig.types.Permissions.all(),
      },
    ],
    treasury,
    rentCollector: null,
    memo: "SAK Demo Agent",
    programId: SQUADS_PROGRAM_ID,
  });
  console.log(`  ✅ Transaction confirmed: ${txSig}`);
  console.log("");

  console.log("╔══════════════════════════════════════════╗");
  console.log("║            SQUADS ACCOUNT CREATED        ║");
  console.log("╠══════════════════════════════════════════╣");
  console.log(`║  ${multisigPda.toBase58()}`);
  console.log("╠══════════════════════════════════════════╣");
  console.log("║  Solscan:");
  console.log(`║  https://solscan.io/account/${multisigPda.toBase58()}?cluster=devnet`);
  console.log("║");
  console.log("║  Squads app:");
  console.log(`║  https://backup.app.squads.so/squads/${multisigPda.toBase58()}`);
  console.log("╚══════════════════════════════════════════╝");
  console.log("");
  console.log("Next step: hardcode this address into");
  console.log("  demo/race-server/src/main.rs");
  console.log("  in squads_create_wallet_handler().");
}

main().catch((err) => {
  console.error("FAILED:", err);
  process.exit(1);
});
