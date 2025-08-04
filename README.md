# CredX

**CredX** is a decentralized credit protocol built on Solana that allows users to deposit **staked tokens (mSOL/jitoSOL)** as collateral to borrow **Credit Tokens**. The borrowed amount is based on live Oracle price feeds, and loan repayments are automated using the yield generated from the staked collateral.  

---

## ✨ Features

- Deposit **yield-bearing staked tokens** (mSOL / jitoSOL) as collateral  
- Borrow **Credit Tokens** based on live LTV ratio and Oracle prices  
- Daily/Weekly **cron job** to repay loans using staking yield  
- **Oracle integration** via [Pyth Network](https://pyth.network/)  
- Fully automated loan lifecycle with secure PDAs  

---


## 📚 How It Works

### 📊 [View Protocol Architecture Diagram](capstone_diagram.pdf)

### Protocol Flow

1. **Admin Initializes Protocol**  
   - Deploys and initializes the protocol state  
   - Creates the Credit Token mint account

2. **User Initializes Protocol (Loan Position)**  
   - A new user loan position is created  
   - Initializes:
     - Collateral Vault PDA  
     - Loan Account PDA  
     - Credit Token Account

3. **Deposit Collateral**  
   - User deposits mSOL/jitoSOL to the Collateral Vault PDA  
   - Deposit data is updated in the Loan Account PDA

4. **Get Oracle Price**  
   - Real-time price fetched from the Pyth Oracle for mSOL/jitoSOL

5. **Lend Credit Token**  
   - Borrowable Credit = `LTV * Oracle Price * Deposited Collateral`  
   - Credit Token is minted to user’s Credit Token Account

6. **Automated Repayment by Cron Job**  
   - Periodic job runs (daily/weekly)  
   - Fetches yield from staked tokens  
   - Calculates equivalent credit value using Oracle  
   - Repays by:
     - Burning user's Credit Token  
     - Reducing debt in Loan Account PDA  

7. **Withdraw Collateral**  
   - If outstanding debt = 0  
   - Transfers staked token back to user  
   - Closes Loan Account PDA  

---

## 🧠 Program Instructions

| # | Instruction               | Description |
|---|---------------------------|-------------|
| 1 | `initializeProtocol`      | Admin sets up the protocol, credit mint, and configuration |
| 2 | `initializeUserLoan`      | User sets up their vault, loan account, and credit account |
| 3 | `depositCollateral`       | Transfers mSOL/jitoSOL from user to Vault PDA |
| 4 | `getOraclePrice`          | Uses Pyth to fetch current staked token price |
| 5 | `lendCreditToken`         | Calculates LTV-based borrow amount and mints Credit Tokens |
| 6 | `cronRepayment`           | Uses yield from staked collateral to repay loan periodically |
| 7 | `withdrawCollateral`      | Allows withdrawal of collateral if loan is fully repaid |

---

## 🧾 Program Accounts (State)

| State Account         | Description |
|-----------------------|-------------|
| `ProtocolState`       | Stores protocol-level config (admin, LTV ratio, credit mint, etc.) |
| `CollateralVaultPDA`  | Stores user's deposited staked tokens |
| `LoanAccountPDA`      | Stores user’s loan data (collateral amount, borrowed, repaid, etc.) |
| `CreditMintPDA`       | Mint account for the Credit Token |
| `CreditAccount`       | User’s token account holding minted Credit Tokens |

---

## 🔗 Dependencies

- Solana
- Anchor
- Pyth Oracle
- SPL Token Program

---

## 🛠️ Development

To run and test locally:

```bash
anchor build
anchor test
