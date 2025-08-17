import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import { CredX } from '../target/types/cred_x';
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  SendTransactionError,
  Connection,
} from '@solana/web3.js';
import {
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  createMint,
  mintTo,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from '@solana/spl-token';

import chai from 'chai';
import chaiBN from 'chai-bn';

describe('CredX Protocol - Complete Integration Test', () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.CredX as Program<CredX>;
  const connection = provider.connection;

  chai.use(chaiBN(anchor.BN));
  const { expect } = chai;

  let admin: Keypair;
  let user: Keypair;
  let collateralMint: PublicKey;
  let creditMint: PublicKey;
  let protocolPda: PublicKey;
  let programAuthorityPda: PublicKey;
  let collateralVaultPda: PublicKey;
  let loanAccountPda: PublicKey;
  let oraclePriceAccount: PublicKey;

  let stakedTokenMint: PublicKey;
  let stakedTokenAuthority: Keypair;
  let yieldGeneratorKeypair: Keypair;
  let mockOracleKeypair: Keypair;

  class MockStakedTokenManager {
    private connection: anchor.web3.Connection;
    private mint: PublicKey;
    private authority: Keypair;
    private yieldRate: number;
    private lastYieldTime: Map<string, number>;

    constructor(
      connection: anchor.web3.Connection,
      mint: PublicKey,
      authority: Keypair,
      annualYieldRate: number = 0.05
    ) {
      this.connection = connection;
      this.mint = mint;
      this.authority = authority;
      this.yieldRate = annualYieldRate;
      this.lastYieldTime = new Map();
    }

    async createStakedPosition(
      userWallet: PublicKey,
      initialAmount: number
    ): Promise<PublicKey> {
      const userAta = await getOrCreateAssociatedTokenAccount(
        this.connection,
        this.authority,
        this.mint,
        userWallet
      );
      await mintTo(
        this.connection,
        this.authority,
        this.mint,
        userAta.address,
        this.authority,
        initialAmount
      );
      this.lastYieldTime.set(userAta.address.toString(), Date.now());
      console.log(
        `✅ Created staked position: ${
          initialAmount / 1e9
        } tokens for ${userWallet.toString().slice(0, 8)}...`
      );
      return userAta.address;
    }

    async generateYieldForPeriod(
      accountAddress: PublicKey,
      periodInDays: number
    ): Promise<number> {
      const account = await getAccount(this.connection, accountAddress);
      const currentBalance = Number(account.amount);
      if (currentBalance === 0) return 0;

      const timeElapsedYears = periodInDays / 365;
      const yieldAmount = Math.floor(
        currentBalance * this.yieldRate * timeElapsedYears
      );

      if (yieldAmount > 0) {
        await mintTo(
          this.connection,
          this.authority,
          this.mint,
          accountAddress,
          this.authority,
          yieldAmount
        );
        console.log(
          `✅ Generated ${
            yieldAmount / 1e9
          } yield tokens for ${periodInDays} days (${(
            this.yieldRate * 100
          ).toFixed(1)}% APY)`
        );
      }
      return yieldAmount;
    }

    setYieldRate(newRate: number): void {
      this.yieldRate = newRate;
      console.log(
        `📊 Updated yield rate to ${(newRate * 100).toFixed(2)}% APY`
      );
    }

    getYieldRate(): number {
      return this.yieldRate;
    }
  }

  class MockOracleManager {
    private connection: Connection;
    private priceKeypair: Keypair;
    private currentPrice: number = 150;

    constructor(connection: Connection, priceKeypair: Keypair) {
      this.connection = connection;
      this.priceKeypair = priceKeypair;
    }

    async createPriceAccount(payer: Keypair, initialPrice: number = 150) {
      try {
        await program.methods
          .createSimpleOracle(
            new anchor.BN(Math.floor(initialPrice * 1_000_000))
          )
          .accounts({
            authority: payer.publicKey,
            priceAccount: this.priceKeypair.publicKey,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([payer, this.priceKeypair])
          .rpc();

        this.currentPrice = initialPrice;
        console.log(`✅ Simple oracle created with price $${initialPrice}`);
      } catch (error) {
        console.error('Failed to create simple oracle:', error);
        throw error;
      }
    }

    async updatePrice(payer: Keypair, newPrice: number) {
      this.currentPrice = newPrice;

      try {
        await program.methods
          .updateSimpleOracle(new anchor.BN(Math.floor(newPrice * 1_000_000)))
          .accounts({
            authority: payer.publicKey,
            priceAccount: this.priceKeypair.publicKey,
          })
          .signers([payer])
          .rpc();

        console.log(`📈 Updated oracle price to $${newPrice}`);
      } catch (error) {
        console.error(`❌ Failed to update oracle price: ${error}`);
        throw error;
      }
    }

    async getCurrentPrice(): Promise<number> {
      return this.currentPrice;
    }

    getPriceAccount(): PublicKey {
      return this.priceKeypair.publicKey;
    }
  }
  let mockStakedTokenManager: MockStakedTokenManager;
  let mockOracleManager: MockOracleManager;

  before(async () => {
    console.log('\n🚀 Setting up CredX Protocol Test Environment...\n');

    admin = Keypair.generate();
    user = Keypair.generate();
    mockOracleKeypair = Keypair.generate();

    console.log('💰 Airdropping SOL...');
    await connection.requestAirdrop(
      admin.publicKey,
      10 * anchor.web3.LAMPORTS_PER_SOL
    );
    await connection.requestAirdrop(
      user.publicKey,
      10 * anchor.web3.LAMPORTS_PER_SOL
    );
    await new Promise((r) => setTimeout(r, 2000));

    stakedTokenAuthority = Keypair.generate();
    yieldGeneratorKeypair = Keypair.generate();
    await connection.requestAirdrop(
      stakedTokenAuthority.publicKey,
      5 * anchor.web3.LAMPORTS_PER_SOL
    );
    await connection.requestAirdrop(
      yieldGeneratorKeypair.publicKey,
      5 * anchor.web3.LAMPORTS_PER_SOL
    );
    await new Promise((r) => setTimeout(r, 1000));

    console.log('🪙 Creating mock staked SOL token...');
    const mintKeypair = Keypair.generate();
    stakedTokenMint = await createMint(
      connection,
      stakedTokenAuthority,
      stakedTokenAuthority.publicKey,
      null,
      9,
      mintKeypair
    );
    collateralMint = stakedTokenMint;

    mockStakedTokenManager = new MockStakedTokenManager(
      connection,
      stakedTokenMint,
      stakedTokenAuthority,
      0.06
    );
    mockOracleManager = new MockOracleManager(connection, mockOracleKeypair);

    console.log('🔍 Finding Program Derived Addresses...');
    [programAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('program_authority')],
      program.programId
    );
    [protocolPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('protocol'), admin.publicKey.toBuffer()],
      program.programId
    );
    [creditMint] = PublicKey.findProgramAddressSync(
      [Buffer.from('credit'), admin.publicKey.toBuffer()],
      program.programId
    );
    [collateralVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from('collateral_vault'), user.publicKey.toBuffer()],
      program.programId
    );
    [loanAccountPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from('loan'),
        user.publicKey.toBuffer(),
        collateralVaultPda.toBuffer(),
      ],
      program.programId
    );

    oraclePriceAccount = mockOracleManager.getPriceAccount();

    console.log('✅ Test environment setup complete!\n');
    console.log('PDAs:');
    console.log(' protocolPda:', protocolPda.toBase58());
    console.log(' creditMint (PDA):', creditMint.toBase58());
  });

  describe('🏛️  Protocol Initialization', () => {
    it('Should initialize protocol successfully', async () => {
      console.log('🔧 Initializing protocol...');

      try {
        const existingProtocol = await connection.getAccountInfo(protocolPda);
        if (existingProtocol) {
          console.log(
            '⚠️  Protocol account already exists, attempting to fetch...'
          );
          try {
            const protocolAccount = await program.account.protocolState.fetch(
              protocolPda
            );
            console.log('✅ Existing protocol account is valid');
            expect(protocolAccount.admin.toString()).to.equal(
              admin.publicKey.toString()
            );
            return;
          } catch {
            console.log('❌ Existing account corrupted, reinitializing...');
          }
        }
      } catch {
        console.log('📝 No existing protocol account, proceeding with init');
      }

      try {
        const tx = await program.methods
          .initializeProtocol()
          .accounts({
            admin: admin.publicKey,
            creditMint,
            protocol: protocolPda,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          } as any)
          .signers([admin])
          .rpc();
        console.log(`✅ Protocol initialized! TX: ${tx.slice(0, 16)}...`);
      } catch (err: any) {
        if (err instanceof SendTransactionError) {
          console.error('❌ initializeProtocol failed, logs:');
          console.error(await err.getLogs(connection));
        }
        throw err;
      }

      await new Promise((r) => setTimeout(r, 800));
      const protocolAccount = await program.account.protocolState.fetch(
        protocolPda
      );
      expect(protocolAccount.admin.toString()).to.equal(
        admin.publicKey.toString()
      );
      expect(protocolAccount.ltvRatioBps).to.equal(6000);
      expect(protocolAccount.isLocked).to.be.false;
      expect(protocolAccount.creditMint.toString()).to.equal(
        creditMint.toString()
      );
    });
  });

  describe('🪙 Mock Staked Token Setup', () => {
    it('Should create mock staked token and setup oracle', async () => {
      await mockOracleManager.createPriceAccount(admin);

      const mintInfo = await connection.getAccountInfo(stakedTokenMint);
      expect(mintInfo).to.not.be.null;

      console.log(`✅ Mock staked token mint: ${stakedTokenMint.toString()}`);
      console.log(`✅ Oracle price account: ${oraclePriceAccount.toString()}`);
    });

    it('Should generate initial staked positions with yield', async () => {
      const initialAmount = 5_000_000_000;

      const userStakedAta = await mockStakedTokenManager.createStakedPosition(
        user.publicKey,
        initialAmount
      );
      const account = await getAccount(connection, userStakedAta);

      expect(Number(account.amount)).to.equal(initialAmount);

      const yieldGenerated =
        await mockStakedTokenManager.generateYieldForPeriod(userStakedAta, 30);

      const finalBalance = await getAccount(connection, userStakedAta);

      console.log(
        `💰 User staked balance: ${Number(finalBalance.amount) / 1e9} tokens`
      );
      console.log(`📈 Initial yield generated: ${yieldGenerated / 1e9} tokens`);

      expect(Number(finalBalance.amount)).to.be.greaterThan(initialAmount);
      expect(yieldGenerated).to.be.greaterThan(0);
    });
  });

  describe('💼 Loan Initialization', () => {
    it('Should initialize loan account', async () => {
      console.log('📋 Initializing loan account...');

      const tx = await program.methods
        .initializeLoan(collateralMint)
        .accounts({
          user: user.publicKey,
          protocol: protocolPda,
          creditMint: creditMint,
          collateralVault: collateralVaultPda,
          loanAccount: loanAccountPda,
          oraclePriceAccount: oraclePriceAccount,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([user])
        .rpc();

      console.log(`✅ Loan initialized! TX: ${tx.slice(0, 16)}...`);

      const loanAccount = await program.account.loanAccount.fetch(
        loanAccountPda
      );
      expect(loanAccount.user.toString()).to.equal(user.publicKey.toString());
      expect(loanAccount.collateralAmount.toNumber()).to.equal(0);
      expect(loanAccount.remainingDebt.toNumber()).to.equal(0);
      expect(loanAccount.yieldEarned.toNumber()).to.equal(0);

      const collateralVault = await program.account.collateralVault.fetch(
        collateralVaultPda
      );
      expect(collateralVault.mint.toString()).to.equal(
        collateralMint.toString()
      );
    });
  });

  describe('🏦 Complete Protocol Flow', () => {
    const depositAmount = 2_000_000_000;
    let userStakedAta: PublicKey;
    let collateralVaultAta: PublicKey;
    let userCreditAta: PublicKey;

    before(async () => {
      userStakedAta = await getAssociatedTokenAddress(
        stakedTokenMint,
        user.publicKey
      );

      collateralVaultAta = await getAssociatedTokenAddress(
        stakedTokenMint,
        collateralVaultPda,
        true
      );

      userCreditAta = await getAssociatedTokenAddress(
        creditMint,
        user.publicKey
      );
    });

    it('Step 1: Should deposit staked SOL as collateral', async () => {
      console.log('\n💎 STEP 1: Depositing staked SOL as collateral');

      const userBalanceBefore = await getAccount(connection, userStakedAta);
      console.log(
        `👤 User balance before: ${
          Number(userBalanceBefore.amount) / 1e9
        } stSOL`
      );

      const tx = await program.methods
        .depositCollateral(new anchor.BN(depositAmount))
        .accounts({
          user: user.publicKey,
          protocol: protocolPda,
          collateralMint: stakedTokenMint,
          userCollateralAta: userStakedAta,
          collateralVault: collateralVaultPda,
          collateralVaultAta: collateralVaultAta,
          programAuthority: programAuthorityPda,
          loanAccount: loanAccountPda,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([user])
        .rpc();

      console.log(`✅ Collateral deposited! TX: ${tx.slice(0, 16)}...`);

      const loanAccount = await program.account.loanAccount.fetch(
        loanAccountPda
      );
      const vaultBalance = await getAccount(connection, collateralVaultAta);

      expect(loanAccount.collateralAmount.toNumber()).to.equal(depositAmount);
      expect(Number(vaultBalance.amount)).to.equal(depositAmount);

      console.log(
        `🏦 Vault balance: ${Number(vaultBalance.amount) / 1e9} stSOL`
      );
      console.log(
        `📊 Collateral recorded: ${
          loanAccount.collateralAmount.toNumber() / 1e9
        } stSOL`
      );
    });

    it('Step 2: Should borrow credit tokens against collateral (Admin signs)', async () => {
      console.log('\n💳 STEP 2: Borrowing credit tokens');

      mockOracleManager.updatePrice(admin, 150);

      const tx = await program.methods
        .lendCreditToken()
        .accounts({
          user: user.publicKey,
          admin: admin.publicKey,
          protocol: protocolPda,
          creditMint: creditMint,
          userCreditAta: userCreditAta,
          collateralVault: collateralVaultPda,
          loanAccount: loanAccountPda,
          oraclePriceAccount: oraclePriceAccount,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([user, admin])
        .rpc();

      console.log(`✅ Credit tokens borrowed! TX: ${tx.slice(0, 16)}...`);

      const loanAccount = await program.account.loanAccount.fetch(
        loanAccountPda
      );
      const userCreditBalance = await getAccount(connection, userCreditAta);

      const borrowedAmount = new anchor.BN(
        loanAccount.remainingDebt.toString()
      );
      const userAmount: any = new anchor.BN(
        userCreditBalance.amount.toString()
      );

      expect(userAmount).to.be.bignumber.greaterThan(new anchor.BN(0));
      expect(userAmount).to.be.bignumber.equal(borrowedAmount);

      console.log(
        `💰 Credit tokens borrowed: ${borrowedAmount
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );
      console.log(
        `💳 User credit balance: ${userAmount
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );
      console.log(
        `📊 Remaining debt: ${borrowedAmount
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );
    });

    it('Step 3: Should generate yield and auto-repay via cron', async () => {
      console.log('\n🔄 STEP 3: Generating yield and auto-repayment');

      let loanAccountBefore = await program.account.loanAccount.fetch(
        loanAccountPda
      );
      const initialDebtBN = new anchor.BN(
        loanAccountBefore.remainingDebt.toString()
      );

      console.log(
        `💳 Initial debt: ${initialDebtBN
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );

      console.log('⏰ Simulating 60 days of staking rewards...');
      const yieldGenerated =
        await mockStakedTokenManager.generateYieldForPeriod(
          collateralVaultAta,
          60
        );

      const vaultBalanceAfterYield = await getAccount(
        connection,
        collateralVaultAta
      );

      console.log(
        `🏦 Vault balance after yield: ${
          Number(vaultBalanceAfterYield.amount) / 1e9
        } stSOL`
      );
      console.log(`📈 Yield generated: ${yieldGenerated / 1e9} stSOL`);

      mockOracleManager.updatePrice(admin, 160);

      console.log('🤖 Executing cron job repayment...');
      const cronTx = await program.methods
        .cronRepayment()
        .accounts({
          authority: user.publicKey,
          protocol: protocolPda,
          collateralVault: collateralVaultPda,
          collateralVaultAta: collateralVaultAta,
          loanAccount: loanAccountPda,
          creditMint: creditMint,
          programAuthority: programAuthorityPda,
          userCreditAta: userCreditAta,
          oraclePriceAccount: oraclePriceAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        } as any)
        .rpc();

      console.log(`✅ Cron repayment executed! TX: ${cronTx.slice(0, 16)}...`);

      const loanAccountAfter = await program.account.loanAccount.fetch(
        loanAccountPda
      );
      const finalDebtBN = new anchor.BN(
        loanAccountAfter.remainingDebt.toString()
      );
      const yieldEarnedBN = new anchor.BN(
        loanAccountAfter.yieldEarned.toString()
      );
      const debtReductionBN = initialDebtBN.sub(finalDebtBN);

      console.log(
        `💳 Final debt: ${finalDebtBN
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );
      console.log(
        `📊 Yield earned (tracked): ${yieldEarnedBN
          .div(new anchor.BN(1_000_000_000))
          .toString()} stSOL`
      );
      console.log(
        `📉 Debt reduction: ${debtReductionBN
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );
      expect(finalDebtBN).to.be.bignumber.lessThan(initialDebtBN);
      expect(yieldEarnedBN).to.be.bignumber.greaterThan(new anchor.BN(0));
      expect(debtReductionBN).to.be.bignumber.greaterThan(new anchor.BN(0));
    });

    it('Step 4: Should handle multiple yield cycles', async () => {
      console.log('\n🔁 STEP 4: Multiple yield and repayment cycles');

      for (let cycle = 1; cycle <= 3; cycle++) {
        console.log(`\n--- Cycle ${cycle} ---`);

        let loanAccount = await program.account.loanAccount.fetch(
          loanAccountPda
        );
        const debtBeforeBN = new anchor.BN(
          loanAccount.remainingDebt.toString()
        );

        if (debtBeforeBN.isZero()) {
          console.log('🎉 Debt fully repaid! No more cycles needed.');
          break;
        }

        console.log(
          `💳 Debt before cycle: ${debtBeforeBN
            .div(new anchor.BN(1_000_000))
            .toString()} CRED`
        );

        const cycleYield = await mockStakedTokenManager.generateYieldForPeriod(
          collateralVaultAta,
          30
        );
        console.log(
          `📈 Cycle ${cycle} yield: ${new anchor.BN(cycleYield.toString())
            .div(new anchor.BN(1_000_000_000))
            .toString()} stSOL`
        );

        const newPrice = 150 + cycle * 5;
        mockOracleManager.updatePrice(admin, newPrice);

        await program.methods
          .cronRepayment()
          .accounts({
            authority: admin.publicKey,
            protocol: protocolPda,
            collateralVault: collateralVaultPda,
            collateralVaultAta: collateralVaultAta,
            loanAccount: loanAccountPda,
            creditMint: creditMint,
            programAuthority: programAuthorityPda,
            userCreditAta: userCreditAta,
            oraclePriceAccount: oraclePriceAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          } as any)
          .rpc();

        loanAccount = await program.account.loanAccount.fetch(loanAccountPda);
        const debtAfterBN = new anchor.BN(loanAccount.remainingDebt.toString());
        const reductionBN = debtBeforeBN.sub(debtAfterBN);

        console.log(
          `💳 Debt after cycle: ${debtAfterBN
            .div(new anchor.BN(1_000_000))
            .toString()} CRED`
        );
        console.log(
          `📉 Debt reduced by: ${reductionBN
            .div(new anchor.BN(1_000_000))
            .toString()} CRED`
        );

        expect(debtAfterBN.lt(debtBeforeBN)).to.be.true;
      }
    });

    it('Step 5: Should allow withdrawal after repayment', async () => {
      console.log('\n💸 STEP 5: Testing withdrawal functionality');

      const loanAccount = await program.account.loanAccount.fetch(
        loanAccountPda
      );
      const remainingDebtBN = new anchor.BN(
        loanAccount.remainingDebt.toString()
      );

      console.log(
        `💳 Remaining debt before withdrawal: ${remainingDebtBN
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );

      const vaultBalanceBefore = await getAccount(
        connection,
        collateralVaultAta
      );
      const userBalanceBefore = await getAccount(connection, userStakedAta);

      console.log(
        `🏦 Vault balance before: ${new anchor.BN(
          vaultBalanceBefore.amount.toString()
        )
          .div(new anchor.BN(1_000_000_000))
          .toString()} stSOL`
      );
      console.log(
        `👤 User balance before: ${new anchor.BN(
          userBalanceBefore.amount.toString()
        )
          .div(new anchor.BN(1_000_000_000))
          .toString()} stSOL`
      );

      const withdrawTx = await program.methods
        .withdrawCollateral()
        .accounts({
          user: user.publicKey,
          protocol: protocolPda,
          creditMint: creditMint,
          programAuthority: programAuthorityPda,
          collateralVault: collateralVaultPda,
          collateralVaultAta: collateralVaultAta,
          userCollateralAta: userStakedAta,
          loanAccount: loanAccountPda,
          userCreditAta: userCreditAta,
          oraclePriceAccount: oraclePriceAccount,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        } as any)
        .signers([user])
        .rpc();

      console.log(`✅ Withdrawal executed! TX: ${withdrawTx.slice(0, 16)}...`);

      const userBalanceAfter = await getAccount(connection, userStakedAta);
      const loanAccountAfter = await program.account.loanAccount.fetch(
        loanAccountPda
      );

      const userBalanceBeforeBN = new anchor.BN(
        userBalanceBefore.amount.toString()
      );
      const userBalanceAfterBN = new anchor.BN(
        userBalanceAfter.amount.toString()
      );

      console.log(
        `👤 User balance after: ${userBalanceAfterBN
          .div(new anchor.BN(1_000_000_000))
          .toString()} stSOL`
      );
      console.log(
        `📊 Final yield earned: ${new anchor.BN(
          loanAccountAfter.yieldEarned.toString()
        )
          .div(new anchor.BN(1_000_000_000))
          .toString()} stSOL`
      );
      console.log(
        `💳 Final debt: ${new anchor.BN(
          loanAccountAfter.remainingDebt.toString()
        )
          .div(new anchor.BN(1_000_000))
          .toString()} CRED`
      );

      expect(userBalanceAfterBN.gt(userBalanceBeforeBN)).to.be.true;

      expect(
        new anchor.BN(loanAccountAfter.collateralAmount.toString()).isZero()
      ).to.be.true;
      expect(new anchor.BN(loanAccountAfter.remainingDebt.toString()).isZero())
        .to.be.true;
    });
  });

  after(async () => {
    console.log('\n🏁 Test Suite Completed Successfully!');
    console.log('  ✅ Protocol initialization');
    console.log('  ✅ Loan account creation and management');
    console.log('  ✅ Collateral deposit and withdrawal');
    console.log('  ✅ Credit token borrowing with admin signatures');
    console.log('  ✅ Automated yield-based repayment via cron jobs');
  });
});
