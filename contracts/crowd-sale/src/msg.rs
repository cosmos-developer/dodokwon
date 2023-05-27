use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

#[cw_serde]
pub struct InstantiateMsg {
    pub cw20_address: Addr,
    pub owner: Addr,
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
