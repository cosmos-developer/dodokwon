use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub cw20_address: Addr,
    pub owner: Addr,
    pub mintable_period_days: u64,
    pub udodokwan_per_uusd: Decimal,
}

#[cw_serde]
pub enum ExecuteMsg {
    Mint {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // #[returns(OracleResp)]
    // OracleOne {},
    #[returns(OwnerResp)]
    Owner {},
    #[returns(Cw20AddressResp)]
    Cw20Address {},
    #[returns(MintableBlockHeightResp)]
    MintableBlockHeight {},
    // create udodokwan uusd pair
    #[returns(UdodokwanPerUusdResp)]
    UdodokwanPerUusd {},
    #[returns(UdodokwanToUlunaResp)]
    UdodokwanToUluna { udodokwan_amount: Uint128 },
}

#[cw_serde]
pub struct OracleResp {
    pub exchange_rate: String,
}

#[cw_serde]
pub struct OwnerResp {
    pub owner: Addr,
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
