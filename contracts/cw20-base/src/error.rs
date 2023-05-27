use cosmwasm_std::StdError;
use cw20_base::ContractError as Cw20BaseError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    StdError(#[from] StdError),
    #[error("Cw20 base error: {0}")]
    Cw20BaseError(#[from] Cw20BaseError),
}
