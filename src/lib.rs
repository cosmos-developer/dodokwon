use cosmwasm_std::{entry_point, DepsMut, Env, MessageInfo, Response, StdResult};
use error::ContractError;
use msg::InstantiateMsg;
use terra_bindings::TerraQuery;

pub mod contract;
pub mod error;
pub mod msg;
pub mod state;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    contract::instantiate(deps, env, info, msg)
}

// #[cfg_attr(not(feature = "library"), entry_point)]
// pub fn query(deps: Deps, env: Env, msg: msg::QueryMsg) -> StdResult<Binary> {
//     contract::query(deps, env, msg)
// }

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut<TerraQuery>,
    env: Env,
    info: MessageInfo,
    msg: msg::ExecuteMsg,
) -> Result<Response, ContractError> {
    contract::execute(deps, env, info, msg)
}
