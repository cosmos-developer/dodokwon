use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub cw20_address: Addr,
    pub mintable_period_days: u64,
    pub udodokwan_per_uusd: Decimal,
    pub maximum_mintable_per_uusd: Uint128,
}

#[cw_serde]
pub enum ExecuteMsg {
    Mint {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Cw20AddressResp)]
    Cw20Address {},
    #[returns(MintableBlockHeightResp)]
    MintableBlockHeight {},
    #[returns(UdodokwanPerUusdResp)]
    UdodokwanPerUusd {},
    #[returns(UdodokwanToUlunaResp)]
    UdodokwanToUluna { udodokwan_amount: Uint128 },
    // Add query burned uluna
    #[returns(BurnedUlunaResp)]
    BurnedUluna {},
    // Add query maximum mintable amount
    #[returns(MaximumMintableAmountResp)]
    MaximumMintableAmount {},
}

#[cw_serde]
pub struct OracleResp {
    pub exchange_rate: String,
}

#[cw_serde]
pub struct Cw20AddressResp {
    pub cw20_address: Addr,
}

#[cw_serde]
pub struct MintableBlockHeightResp {
    pub mintable_block_height: u64,
}

#[cw_serde]
pub struct UdodokwanPerUusdResp {
    pub uusd: Decimal,
}

#[cw_serde]
pub struct UdodokwanToUlunaResp {
    pub uluna_amount: Decimal,
}

#[cw_serde]
pub struct BurnedUlunaResp {
    pub burned_uluna: Uint128,
}

#[cw_serde]
pub struct MaximumMintableAmountResp {
    pub maximum_mintable_amount: Uint128,
}
