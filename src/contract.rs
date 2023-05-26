use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg},
    state::MINTABLE_BLOCK_HEIGHT,
};

use cosmwasm_std::{Decimal, DepsMut, Env, MessageInfo, Response, StdResult, Uint128};
use cw20_base::state::{MinterData, TokenInfo, BALANCES, TOKEN_INFO};
use cw20_base::ContractError as Cw20BaseError;
use terra_bindings::{TerraQuerier, TerraQuery};

pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let contract_addr = env.contract.address;

    let max_supply = msg.max_supply;
    let mint_data = MinterData {
        minter: contract_addr,
        cap: Some(max_supply),
    };

    // foundation holds 7.86% of max supply.
    let foundation = msg.foundation;
    let numerator: u128 = 7_86;
    let denominator: u128 = 100_00;
    let foundation_token_amount = max_supply
        .checked_multiply_ratio(numerator, denominator)
        .unwrap();
    BALANCES.save(deps.storage, &foundation, &foundation_token_amount)?;

    // store token info
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply: foundation_token_amount,
        mint: Some(mint_data),
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    let avg_seconds_per_block = 5;
    let blocks_per_minute = 60 / avg_seconds_per_block;
    let blocks_per_day = blocks_per_minute * 60 * 24;
    let mintable_day_range = 30;
    let current_block_height = env.block.height;
    let mintable_block_height = current_block_height + mintable_day_range * blocks_per_day;
    MINTABLE_BLOCK_HEIGHT.save(deps.storage, &mintable_block_height)?;

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
        let token_amount = token_amount.to_uint_floor();

        if token_amount == Uint128::zero() {
            return Err((Cw20BaseError::InvalidZeroAmount {}).into());
        }

        let recipient = &info.sender;
        let contract_addr = env.contract.address;

        let mut config = TOKEN_INFO
            .may_load(deps.storage)?
            .ok_or(Cw20BaseError::InvalidZeroAmount {})?;

        if config
            .mint
            .as_ref()
            .ok_or(Cw20BaseError::InvalidZeroAmount {})?
            .minter
            != contract_addr
        {
            return Err((Cw20BaseError::InvalidZeroAmount {}).into());
        }

        // update supply and enforce cap
        config.total_supply += token_amount;
        if let Some(limit) = config.get_cap() {
            if config.total_supply > limit {
                return Err((Cw20BaseError::CannotExceedCap {}).into());
            }
        }
        TOKEN_INFO.save(deps.storage, &config)?;

        // add amount to recipient balance
        BALANCES.update(
            deps.storage,
            recipient,
            |balance: Option<Uint128>| -> StdResult<_> {
                Ok(balance.unwrap_or_default() + token_amount)
            },
        )?;

        let res = Response::new()
            .add_attribute("action", "mint")
            .add_attribute("to", recipient)
            .add_attribute("amount", token_amount);
        Ok(res)
    }
}
