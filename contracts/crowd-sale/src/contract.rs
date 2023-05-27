use crate::{
    error::ContractError,
    msg::*,
    state::{CW20_ADDRESS, MINTABLE_BLOCK_HEIGHT, OWNER},
};

use cosmwasm_std::{
    from_binary, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};
use cw20_base::ContractError as Cw20BaseError;
use terra_bindings::{ExchangeRatesResponse, TerraQuerier, TerraQuery};

pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let avg_seconds_per_block = 5;
    let blocks_per_minute = 60 / avg_seconds_per_block;
    let blocks_per_day = blocks_per_minute * 60 * 24;
    let mintable_day_range = 30;
    let current_block_height = env.block.height;
    let mintable_block_height = current_block_height + mintable_day_range * blocks_per_day;
    MINTABLE_BLOCK_HEIGHT.save(deps.storage, &mintable_block_height)?;

    CW20_ADDRESS.save(deps.storage, &msg.cw20_address)?;
    OWNER.save(deps.storage, &msg.owner)?;

    // let contract_addr = env.contract.address;
    // let max_supply = msg.max_supply;
    // let mint_data = MinterData {
    //     minter: contract_addr,
    //     cap: Some(max_supply),
    // };

    // // foundation holds 7.86% of max supply.
    // let foundation = msg.foundation;
    // let numerator: u128 = 7_86;
    // let denominator: u128 = 100_00;
    // let foundation_token_amount = max_supply
    //     .checked_multiply_ratio(numerator, denominator)
    //     .unwrap();
    // BALANCES.save(deps.storage, &foundation, &foundation_token_amount)?;

    // // store token info
    // let data = TokenInfo {
    //     name: msg.name,
    //     symbol: msg.symbol,
    //     decimals: msg.decimals,
    //     total_supply: foundation_token_amount,
    //     mint: Some(mint_data),
    // };
    // TOKEN_INFO.save(deps.storage, &data)?;

    Ok(Response::default())
}

pub fn execute(
    deps: DepsMut<TerraQuery>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint {} => execute::mint(deps, env, info),
    }
}
mod execute {
    use cosmwasm_std::{CosmosMsg, WasmMsg};
    use cw20::Cw20ExecuteMsg;

    use super::*;

    pub fn mint(
        deps: DepsMut<TerraQuery>,
        env: Env,
        info: MessageInfo,
    ) -> Result<Response, ContractError> {
        let block_height = env.block.height;
        let mintable_block_height = MINTABLE_BLOCK_HEIGHT.load(deps.storage)?;
        if block_height > mintable_block_height {
            return Err(ContractError::ExceedMintableBlock {});
        }

        let base_denom = "uluna";
        let quote_denom = "uusd";
        let uluna_amount = cw_utils::must_pay(&info, &base_denom)?;

        // let token = TOKEN_INFO.load(deps.storage)?;
        // let _decimals = token.decimals;

        let querier = TerraQuerier::new(&deps.querier);
        let exchange_rates = querier.query_exchange_rates(base_denom, vec![quote_denom])?;
        let uluna_uusd = exchange_rates.exchange_rates[0].exchange_rate;

        let token_per_uusd =
            Decimal::from_ratio(Uint128::new(1u128), Uint128::new(1_000_000_000u128));
        let token_per_uluna = token_per_uusd.checked_div(uluna_uusd).unwrap();
        let token_amount = token_per_uluna
            .checked_mul(Decimal::new(uluna_amount))
            .unwrap();

        let amount = token_amount.to_uint_floor();
        if amount == Uint128::zero() {
            return Err((Cw20BaseError::InvalidZeroAmount {}).into());
        }

        let cw20_address = CW20_ADDRESS.load(deps.storage)?;
        let recipient = info.sender;
        let mint_cw20_msg = Cw20ExecuteMsg::Mint {
            recipient: recipient.to_string(),
            amount,
        };
        let exec_cw20_mint_msg = WasmMsg::Execute {
            contract_addr: cw20_address.into(),
            msg: to_binary(&mint_cw20_msg)?,
            funds: vec![],
        };
        let cw20_mint_cosmos_msg: CosmosMsg = exec_cw20_mint_msg.into();

        let res = Response::new()
            .add_message(cw20_mint_cosmos_msg)
            .add_attribute("to", recipient)
            .add_attribute("amount", amount);
        Ok(res)
    }
}

pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Owner {} => to_binary(&query::owner(deps)?),
        QueryMsg::Cw20Address {} => to_binary(&query::cw20_address(deps)?),
    }
}

mod query {

    use super::*;

    pub fn owner(deps: Deps) -> StdResult<OwnerResp> {
        let owner = OWNER.load(deps.storage)?;
        Ok(OwnerResp { owner })
    }

    pub fn cw20_address(deps: Deps) -> StdResult<Cw20AddressResp> {
        let cw20_address = CW20_ADDRESS.load(deps.storage)?;
        Ok(Cw20AddressResp { cw20_address })
    }

    pub fn _oracle(deps: Deps) -> Result<OracleResp, ContractError> {
        let base_denom = "uluna";
        let quote_denom = "uusd";
        let query = TerraQuery::ExchangeRates {
            base_denom: base_denom.to_string(),
            quote_denoms: vec![quote_denom.to_string()],
        };
        let bin_query = to_binary(&query)?;

        let system_res = deps.querier.raw_query(&bin_query.as_slice());

        match system_res {
            cosmwasm_std::SystemResult::Ok(contract_result) => {
                let bin_response = contract_result.unwrap();
                let exchange_rates: ExchangeRatesResponse = from_binary(&bin_response)?;
                let uluna_uusd = exchange_rates.exchange_rates[0].exchange_rate;
                return Ok(OracleResp {
                    exchange_rate: uluna_uusd.to_string(),
                });
            }
            cosmwasm_std::SystemResult::Err(err) => {
                return Err(err.into());
            }
        }
    }
}
