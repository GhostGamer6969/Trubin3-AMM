import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Amm } from "../target/types/amm";
// import wallet from "/home/ghostgamer/.config/solana/id.json"
import { PublicKey } from "@solana/web3.js"
// import { bytes } from "@coral-xyz/anchor/dist/cjs/utils";
import { expect } from "chai";
import { SendTransactionError } from "@solana/web3.js";

import { getAccount, getAssociatedTokenAddress, getOrCreateAssociatedTokenAccount } from "@solana/spl-token";

describe("amm", () => {
  const provider = anchor.AnchorProvider.env()
  anchor.setProvider(provider);

  const program = anchor.workspace.amm as Program<Amm>;

  const wallet = provider.wallet

  const seed = new anchor.BN(Math.random() * 10_000)
  console.log(seed.toString())
  // const seed = new anchor.BN(866);
  const seed2 = new anchor.BN(Math.random() * 100_000)

  const fee = 30

  const Zoro = new PublicKey("6QRY8Stw4VzgGSLeTavBNAcpVNHV2ArFnpg7GKQx5vrL")
  const G5 = new PublicKey("HHL8VZCfzdUvPp6hHq2V1h7oWxZBFGgrkyT9SCTAJ3Nm")

  const max_zoro = new anchor.BN(50_000 * 1_000_000)
  const max_g5 = new anchor.BN(50_000 * 1_000_000)

  const [config] = PublicKey.findProgramAddressSync([
    Buffer.from("config"),
    seed.toArrayLike(Buffer, "le", 8)
  ],
    program.programId,
  );
  const [lp] = PublicKey.findProgramAddressSync([
    Buffer.from("lp"),
    config.toBuffer(),
  ],
    program.programId,
  );


  it("Is initialized with authority!", async () => {
    console.log("seed: ", seed.toString())
    // Add your test here.
    const tx = await program.methods.initialize(seed, fee, wallet.publicKey)
      .accountsPartial({
        initializer: wallet.publicKey,
        mintX: Zoro,
        mintY: G5,
        mintLp: lp,
        config: config,
      })
      .rpc();
    const configAccount = await program.account.config.fetch(config);
    expect(configAccount.authority.equals(wallet.publicKey)).to.be.true;
    expect(configAccount.seed.toString()).to.equal(seed.toString());
    expect(configAccount.fee.toString()).to.equal(fee.toString());

  });

  it("fails initialize with same seed!", async () => {
    try {

      const tx = await program.methods.initialize(seed, fee, wallet.publicKey)
        .accountsPartial({
          initializer: wallet.publicKey,
          mintX: Zoro,
          mintY: G5,
          mintLp: lp,
          config: config,
        })
        .rpc();
      throw new Error("Expected initialize to fail, but it succeeded");
    } catch (e) {
      expect(e).to.be.instanceOf(SendTransactionError);
      expect(e.message).to.include("already in use");
    }
  });

  it("Deposit!", async () => {
    const userZoroATA = await getOrCreateAssociatedTokenAccount(provider.connection, wallet.payer, Zoro, wallet.publicKey);
    const userG5ATA = await getOrCreateAssociatedTokenAccount(provider.connection, wallet.payer, G5, wallet.publicKey);
    const userLpATA = await getOrCreateAssociatedTokenAccount(provider.connection, wallet.payer, lp, wallet.publicKey);

    const depositAmount = new anchor.BN(50_000 * 1_000_000);

    const userZoroBefore = await getAccount(provider.connection, userZoroATA.address);
    const userG5Before = await getAccount(provider.connection, userG5ATA.address);
    const userLpBefore = await getAccount(provider.connection, userLpATA.address);
    const tx = await program.methods.deposit(new anchor.BN(depositAmount), max_zoro, max_g5)
      .accountsPartial({
        user: wallet.publicKey,
        mintX: Zoro,
        mintY: G5,
        mintLp: lp,
        config: config,
      })
      .rpc();
    const userZoroAfter = await getAccount(provider.connection, userZoroATA.address);
    const userG5After = await getAccount(provider.connection, userG5ATA.address);
    const userLpAfter = await getAccount(provider.connection, userLpATA.address);

    expect(Number(userZoroAfter.amount)).to.be.lessThan(Number(userZoroBefore.amount));
    expect(Number(userG5After.amount)).to.be.lessThan(Number(userG5Before.amount));
    expect(Number(userLpAfter.amount)).to.be.greaterThan(Number(userLpBefore.amount));

  });
  it("fails to deposit invalid amount!", async () => {
    const depositAmount = new anchor.BN(0);

    try {
      await program.methods
        .deposit(depositAmount, max_zoro, max_g5)
        .accountsPartial({
          user: wallet.publicKey,
          mintX: Zoro,
          mintY: G5,
          mintLp: lp,
          config: config,
        })
        .rpc();
      expect.fail("Deposit should have failed due to invalid amount");
    } catch (e) {
      expect(e).to.have.property("logs");
      expect(e.logs.join("\n")).to.include("Invalid amount.");
    }
  });

  it("Swapping Zoro to Gear5!", async () => {
    const userZoroATA = await getAssociatedTokenAddress(Zoro, wallet.publicKey);
    const userG5ATA = await getAssociatedTokenAddress(G5, wallet.publicKey);
    const vaultZoro = await getAssociatedTokenAddress(Zoro, config, true);
    const vaultG5 = await getAssociatedTokenAddress(G5, config, true);

    const userZoroBefore = await getAccount(provider.connection, userZoroATA);
    const userG5Before = await getAccount(provider.connection, userG5ATA);
    const vaultZoroBefore = await getAccount(provider.connection, vaultZoro);
    const vaultG5Before = await getAccount(provider.connection, vaultG5);

    const amountIn = new anchor.BN(50 * 1_000_000);
    const slippage = 500;

    const tx = await program.methods
      .swap(true, amountIn, slippage)
      .accountsPartial({
        user: wallet.publicKey,
        mintX: Zoro,
        mintY: G5,
        config: config,
      })
      .rpc();

    const userZoroAfter = await getAccount(provider.connection, userZoroATA);
    const userG5After = await getAccount(provider.connection, userG5ATA);
    const vaultZoroAfter = await getAccount(provider.connection, vaultZoro);
    const vaultG5After = await getAccount(provider.connection, vaultG5);

    expect(Number(userZoroAfter.amount)).to.be.lessThan(Number(userZoroBefore.amount));
    expect(Number(userG5After.amount)).to.be.greaterThan(Number(userG5Before.amount));
    expect(Number(vaultZoroAfter.amount)).to.be.greaterThan(Number(vaultZoroBefore.amount));
    expect(Number(vaultG5After.amount)).to.be.lessThan(Number(vaultG5Before.amount));
  });

  it("Swapping Gear5 to Zoro!", async () => {
    const userZoroATA = await getAssociatedTokenAddress(Zoro, wallet.publicKey);
    const userG5ATA = await getAssociatedTokenAddress(G5, wallet.publicKey);
    const vaultZoro = await getAssociatedTokenAddress(Zoro, config, true);
    const vaultG5 = await getAssociatedTokenAddress(G5, config, true);

    const userZoroBefore = await getAccount(provider.connection, userZoroATA);
    const userG5Before = await getAccount(provider.connection, userG5ATA);
    const vaultZoroBefore = await getAccount(provider.connection, vaultZoro);
    const vaultG5Before = await getAccount(provider.connection, vaultG5);

    const amountIn = new anchor.BN(50 * 1_000_000);
    const slippage = 500;

    const tx = await program.methods
      .swap(false, amountIn, slippage)
      .accountsPartial({
        user: wallet.publicKey,
        mintX: Zoro,
        mintY: G5,
        config: config,
      })
      .rpc();

    const userZoroAfter = await getAccount(provider.connection, userZoroATA);
    const userG5After = await getAccount(provider.connection, userG5ATA);
    const vaultZoroAfter = await getAccount(provider.connection, vaultZoro);
    const vaultG5After = await getAccount(provider.connection, vaultG5);

    expect(Number(userG5After.amount)).to.be.lessThan(Number(userG5Before.amount));
    expect(Number(userZoroAfter.amount)).to.be.greaterThan(Number(userZoroBefore.amount));
    expect(Number(vaultG5After.amount)).to.be.greaterThan(Number(vaultG5Before.amount));
    expect(Number(vaultZoroAfter.amount)).to.be.lessThan(Number(vaultZoroBefore.amount));
  });

});
