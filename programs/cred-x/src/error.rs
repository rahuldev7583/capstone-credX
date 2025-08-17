use anchor_lang::prelude::*;

#[error_code]
pub enum CredXError {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Invalid LTV ratio: must be between 1 and 9000 basis points")]
    InvalidLtvRatio,
    #[msg("Invalid mint authority")]
    InvalidMintAuthority,
    #[msg("Protocol already initialized")]
    ProtocolAlreadyInitialized,
    #[msg("Invalid collateral amount")]
    InvalidCollateralAmount,
    #[msg("Unsupported collateral mint")]
    UnsupportedCollateralMint,
    #[msg("Invalid user")]
    InvalidUser,
    #[msg("Invalid oracle account")]
    InvalidOracleAccount,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Mint mismatch")]
    MintMismatch,
    #[msg("Unauthorized user")]
    UnauthorizedUser,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Protocol locked")]
    ProtocolLocked,
    #[msg("Math overflow: amount too large to process")]
    MathOverflow,
    #[msg("Invalid credit mint")]
    InvalidCreditMint,
    #[msg("Invalid collateral mint")]
    InvalidCollateralMint,
    #[msg("No collateral deposited")]
    NoCollateralDeposited,
    #[msg("Empty oracle account")]
    EmptyOracleAccount,
    #[msg("Failed to borrow oracle data")]
    FailedToBorrowOracleData,
    #[msg("Invalid pyth account")]
    InvalidPythAccount,
    #[msg("Invalid price status of pyth account")]
    InvalidPriceStatus,
    #[msg("Invalid price of pyth account")]
    InvalidPrice,
    #[msg("Stale oracle data: price not updated within the last 5 minutes")]
    StalePrice,
    #[msg("Borrow value should be positive")]
    ZeroBorrowAmount,
    #[msg("Invalid borrow amount")]
    InvalidBorrowAmount,
    #[msg("Max borrow exceeded")]
    ExceedsMaxBorrow,
    #[msg("Max borrow limit reached")]
    MaxBorrowLimitReached,
    #[msg("User vault has less amount than loan account collateral")]
    InsufficientCollateral,
    #[msg("No outstanding debt")]
    NoOutstandingDebt,
    #[msg("No tokens to burn")]
    NoTokensToBurn,
    #[msg("Negative yield")]
    NegativeYield,
    #[msg("Zero repayment value")]
    ZeroRepaymentValue,
    #[msg("Insufficient credit tokens")]
    InsufficientCreditTokens,
    #[msg("Math under flow")]
    MathUnderflow,
    #[msg("Outstanding debt exists")]
    OutstandingDebtExists,
    #[msg("Insufficient collateral for debt")]
    InsufficientCollateralForDebt,
    #[msg("No withdrawable collateral")]
    NoWithdrawableCollateral,
    #[msg("Failed to load price account")]
    FailedToLoadPriceAccount,
    #[msg("Unauthorized admin")]
    UnauthorizedAdmin,
    #[msg("Account not enough keys")]
    AccountNotEnoughKeys,
    #[msg("No active loan")]
    NoActiveLoan,
    #[msg("Insufficient collateral value")]
    InsufficientCollateralValue,
}
