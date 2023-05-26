use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw20_base::state::MinterData;

#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub max_supply: Uint128,
    pub mint: Option<MinterData>,
    pub foundation: Addr,
}

#[cw_serde]
pub enum ExecuteMsg {
    Mint {},
}
