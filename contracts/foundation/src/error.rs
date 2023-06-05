use cosmwasm_std::StdError;
use cw3_fixed_multisig::ContractError as Cw3FixedMultisigError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    StdError(#[from] StdError),
    #[error("Cw3 fixed multisig error: {0}")]
    Cw3FixedMultisigError(#[from] Cw3FixedMultisigError),
    #[error("Insufficient fund")]
    InsufficientFund {},
    #[error("Unauthorized")]
    Unauthorized {},
    #[error("Wrong expiration option")]
    WrongExpiration {},
    #[error("Proposal must have passed and not yet been executed")]
    WrongExecuteStatus {},
}
