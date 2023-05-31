use crate::{
    error::ContractError,
    msg::*,
    state::{CW20_ADDRESS, MINTABLE_BLOCK_HEIGHT, UDODOKWAN_UUSD},
};

use cosmwasm_std::{
    to_binary, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw20_base::ContractError as Cw20BaseError;
use terra_bindings::{TerraQuerier, TerraQuery};

const AVG_BLOCKS_PER_DAY: u64 = 24 * 60 * 60 / 5; // 1 block per 5 seconds

pub fn instantiate(
    deps: DepsMut<TerraQuery>,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let current_block_height = env.block.height;
    let mintable_block_height =
        current_block_height + msg.mintable_period_days * AVG_BLOCKS_PER_DAY;
    MINTABLE_BLOCK_HEIGHT.save(deps.storage, &mintable_block_height)?;

    CW20_ADDRESS.save(deps.storage, &msg.cw20_address)?;
    UDODOKWAN_UUSD.save(deps.storage, &msg.udodokwan_per_uusd)?;

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
        let querier = TerraQuerier::new(&deps.querier);
        let exchange_rates = querier.query_exchange_rates(base_denom, vec![quote_denom])?;

        let uluna_uusd = exchange_rates.exchange_rates[0].exchange_rate;
        let udodokwan_uusd = UDODOKWAN_UUSD.load(deps.storage).unwrap();
        let udodokwan_uluna = udodokwan_uusd.checked_div(uluna_uusd).unwrap();

        let uluna_amount = cw_utils::must_pay(&info, &base_denom)?;
        let uluna_amount = Decimal::from_atomics(uluna_amount, 0).unwrap();
        let udodokwan_amount = udodokwan_uluna.checked_mul(uluna_amount).unwrap();

        let amount = udodokwan_amount.to_uint_floor();
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

pub fn query(deps: Deps<TerraQuery>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Cw20Address {} => to_binary(&query::cw20_address(deps)?),
        QueryMsg::MintableBlockHeight {} => to_binary(&query::mintable_block_height(deps)?),
        QueryMsg::UdodokwanPerUusd {} => to_binary(&query::udodokwan_per_uusd(deps)?),
        QueryMsg::UdodokwanToUluna { udodokwan_amount } => {
            to_binary(&query::udodokwan_to_uluna(deps, udodokwan_amount)?)
        }
    }
}

mod query {
    use super::*;

    pub fn cw20_address(deps: Deps<TerraQuery>) -> StdResult<Cw20AddressResp> {
        let cw20_address = CW20_ADDRESS.load(deps.storage)?;
        Ok(Cw20AddressResp { cw20_address })
    }

    pub fn mintable_block_height(deps: Deps<TerraQuery>) -> StdResult<MintableBlockHeightResp> {
        let mintable_block_height = MINTABLE_BLOCK_HEIGHT.load(deps.storage)?;
        Ok(MintableBlockHeightResp {
            mintable_block_height,
        })
    }

    pub fn udodokwan_per_uusd(deps: Deps<TerraQuery>) -> StdResult<UdodokwanPerUusdResp> {
        let udodokwan_uusd = UDODOKWAN_UUSD.load(deps.storage)?;
        Ok(UdodokwanPerUusdResp {
            uusd: udodokwan_uusd,
        })
    }

    pub fn udodokwan_to_uluna(
        deps: Deps<TerraQuery>,
        udodokwan_amount: Uint128,
    ) -> StdResult<UdodokwanToUlunaResp> {
        let querier = TerraQuerier::new(&deps.querier);
        let exchange_rates = querier.query_exchange_rates("uluna", vec!["uusd"])?;

        let uluna_uusd = exchange_rates.exchange_rates[0].exchange_rate;
        let udodokwan_uusd = UDODOKWAN_UUSD.load(deps.storage)?;
        let uluna_per_udodokwan = uluna_uusd.checked_div(udodokwan_uusd).unwrap();

        let udodokwan_amount = Decimal::from_atomics(udodokwan_amount, 0).unwrap();
        let uluna_amount = uluna_per_udodokwan.checked_mul(udodokwan_amount).unwrap();

        Ok(UdodokwanToUlunaResp { uluna_amount })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use cosmwasm_std::{
        testing::{mock_env, mock_info},
        Addr,
    };
    use terra_bindings::{ExchangeRateItem, ExchangeRatesResponse};

    mod unit_test {
        use super::*;

        use std::{marker::PhantomData, str::FromStr};

        use cosmwasm_std::{
            from_binary,
            testing::{MockApi, MockQuerier, MockStorage},
            Coin, OwnedDeps, SystemResult,
        };

        fn mock_deps_with_terra_query(
        ) -> OwnedDeps<MockStorage, MockApi, MockQuerier<TerraQuery>, TerraQuery> {
            let mock_querier = MockQuerier::<TerraQuery>::new(&[]);
            OwnedDeps {
                storage: MockStorage::default(),
                api: MockApi::default(),
                querier: mock_querier.with_custom_handler(|query| match query {
                    TerraQuery::ExchangeRates {
                        base_denom,
                        quote_denoms,
                    } => {
                        assert_eq!(base_denom, "uluna");
                        assert_eq!(quote_denoms[0], "uusd".to_string());
                        let response = ExchangeRatesResponse {
                            base_denom: base_denom.to_string(),
                            exchange_rates: vec![ExchangeRateItem {
                                quote_denom: quote_denoms[0].to_string(),
                                exchange_rate: Decimal::from_ratio(
                                    Uint128::from(1u128),
                                    Uint128::from(11_500u128),
                                ),
                            }],
                        };
                        let bin_response = to_binary(&response);

                        SystemResult::Ok((bin_response).into())
                    }
                    _ => panic!("DO NOT ENTER HERE"),
                }),
                custom_query_type: PhantomData,
            }
        }

        #[test]
        fn proper_instantiate() {
            let mut deps = mock_deps_with_terra_query();
            let env = mock_env();
            let owner = Addr::unchecked("owner");
            let info = mock_info(&owner.to_string(), &[]);

            let udodokwan_per_uusd = Decimal::from_str("0.0000000001").unwrap();
            let cw20_address = Addr::unchecked("cw20_address");
            let msg = InstantiateMsg {
                cw20_address: cw20_address.clone(),
                mintable_period_days: 30,
                udodokwan_per_uusd,
            };

            let res = instantiate(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();
            assert_eq!(0, res.messages.len());

            let bin_res = query(deps.as_ref(), env.clone(), QueryMsg::Cw20Address {}).unwrap();
            let res: Cw20AddressResp = from_binary(&bin_res).unwrap();
            assert_eq!(res.cw20_address, cw20_address);

            let bin_res =
                query(deps.as_ref(), mock_env(), QueryMsg::MintableBlockHeight {}).unwrap();
            let res: MintableBlockHeightResp = from_binary(&bin_res).unwrap();
            assert_eq!(
                res.mintable_block_height,
                env.block.height + msg.mintable_period_days * AVG_BLOCKS_PER_DAY
            );

            let bin_res = query(deps.as_ref(), mock_env(), QueryMsg::UdodokwanPerUusd {}).unwrap();
            let res: UdodokwanPerUusdResp = from_binary(&bin_res).unwrap();
            assert_eq!(res.uusd, udodokwan_per_uusd);
        }

        #[test]
        fn can_mint() {
            let mut deps = mock_deps_with_terra_query();
            let env = mock_env();
            let owner = Addr::unchecked("owner");

            let udodokwan_per_uusd = Decimal::from_str("0.0000000001").unwrap();

            let cw20_address = Addr::unchecked("cw20_address");
            let msg = InstantiateMsg {
                cw20_address: cw20_address.clone(),
                mintable_period_days: 30,
                udodokwan_per_uusd,
            };
            let info = mock_info(&owner.to_string(), &[]);

            let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
            assert_eq!(0, res.messages.len());

            let msg = ExecuteMsg::Mint {};
            let buyer = Addr::unchecked("buyer");
            let uluna_amount = 100_000_000;
            let info = mock_info(&buyer.to_string(), &[Coin::new(uluna_amount, "uluna")]);
            let res = execute(deps.as_mut(), env, info, msg).unwrap();

            let udodokwan_minted_amount_option =
                res.attributes.iter().find(|attr| attr.key == "amount");
            assert!(udodokwan_minted_amount_option.is_some());
            let udodokwan_minted_amount = &udodokwan_minted_amount_option.unwrap().value;
            let udodokwan_amount = Uint128::from_str(udodokwan_minted_amount).unwrap();

            let bin_res = query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::UdodokwanToUluna { udodokwan_amount },
            );
            let res: UdodokwanToUlunaResp = from_binary(&bin_res.unwrap()).unwrap();
            let uluna_amount_expect = res.uluna_amount.to_uint_ceil();

            assert_eq!(uluna_amount, uluna_amount_expect.u128());
        }

        #[test]
        fn error_exceed_mintable_block_height() {
            let mut deps = mock_deps_with_terra_query();
            let mut env = mock_env();
            let owner = Addr::unchecked("owner");

            let udodokwan_per_uusd = Decimal::from_str("0.0000000001").unwrap();

            let cw20_address = Addr::unchecked("cw20_address");
            let msg = InstantiateMsg {
                cw20_address: cw20_address.clone(),
                mintable_period_days: 30,
                udodokwan_per_uusd,
            };
            let info = mock_info(&owner.to_string(), &[]);
            instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

            let msg = ExecuteMsg::Mint {};
            let buyer = Addr::unchecked("buyer");
            let uluna_amount = 100_000_000;
            let info = mock_info(&buyer.to_string(), &[Coin::new(uluna_amount, "uluna")]);
            env.block.height += 30 * AVG_BLOCKS_PER_DAY + 1;
            let res = execute(deps.as_mut(), env, info, msg);
            assert!(res.is_err());
            assert_eq!(
                Err(ContractError::ExceedMintableBlock {}),
                res.map_err(Into::into)
            );
        }
    }
}
