use cosmwasm_std::{StdError, SystemError};
use cw20_base::ContractError as Cw20BaseError;
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    StdError(#[from] StdError),
    #[error("{0}")]
    Cw20BaseError(#[from] Cw20BaseError),
    #[error("{0}")]
    Payment(#[from] PaymentError),
    #[error("Exceed mintable block height")]
    ExceedMintableBlock {},
    #[error("System error: {0}")]
    SystemError(#[from] SystemError),
}
