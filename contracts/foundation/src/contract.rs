use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw3_fixed_multisig::contract::{
    execute_vote as cw3_execute_vote, instantiate as cw3_instantiate,
};
use cw3_fixed_multisig::msg::InstantiateMsg as Cw3InstantiateMsg;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::CW20_ADDRESS;

pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    CW20_ADDRESS.save(deps.storage, &msg.cw20_address)?;
    let cw3_instantiate_msg = Cw3InstantiateMsg {
        voters: msg.voters,
        threshold: msg.threshold,
        max_voting_period: msg.max_voting_period,
    };

    cw3_instantiate(deps, env, info, cw3_instantiate_msg).map_err(Into::into)
}

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Propose {
            title,
            description,
            proposal_type,
            msgs,
            latest,
        } => execute::propose(
            deps,
            env,
            info,
            proposal_type,
            title,
            description,
            msgs,
            latest,
        ),
        ExecuteMsg::Vote { proposal_id, vote } => {
            cw3_execute_vote(deps, env, info, proposal_id, vote).map_err(Into::into)
        }
        ExecuteMsg::Execute { proposal_id } => execute::execute(deps, env, info, proposal_id),

        ExecuteMsg::Close { proposal_id } => {
            cw3_fixed_multisig::contract::execute_close(deps, env, info, proposal_id)
                .map_err(Into::into)
        }
    }
}

mod execute {
    use std::cmp::Ordering;

    use cosmwasm_std::{CosmosMsg, Empty, WasmMsg};
    use cw20::Cw20ExecuteMsg;
    use cw3::{Ballot, Proposal, Status, Vote, Votes};
    use cw3_fixed_multisig::state::{next_id, BALLOTS, CONFIG, PROPOSALS, VOTERS};
    use cw_utils::Expiration;

    use crate::{msg::ProposalType, state::PROPOSAL_ID_TO_TYPE};

    use super::*;

    pub fn propose(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        proposal_type: ProposalType,
        title: String,
        description: String,
        msgs: Vec<CosmosMsg<Empty>>,
        latest: Option<Expiration>,
    ) -> Result<Response, ContractError> {
        // only members of the multisig can create a proposal
        let vote_power = VOTERS
            .may_load(deps.storage, &info.sender)?
            .ok_or(ContractError::Unauthorized {})?;

        let cfg = CONFIG.load(deps.storage)?;

        // max expires also used as default
        let max_expires = cfg.max_voting_period.after(&env.block);
        let mut expires = latest.unwrap_or(max_expires);
        let comp = expires.partial_cmp(&max_expires);
        if let Some(Ordering::Greater) = comp {
            expires = max_expires;
        } else if comp.is_none() {
            return Err(ContractError::WrongExpiration {});
        }

        // create a proposal
        let mut prop = Proposal {
            title,
            description,
            start_height: env.block.height,
            expires,
            msgs,
            status: Status::Open,
            votes: Votes::yes(vote_power),
            threshold: cfg.threshold,
            total_weight: cfg.total_weight,
            proposer: info.sender.clone(),
            deposit: None,
        };
        prop.update_status(&env.block);
        let id = next_id(deps.storage)?;
        PROPOSALS.save(deps.storage, id, &prop)?;
        PROPOSAL_ID_TO_TYPE
            .save(deps.storage, id, &proposal_type)
            .unwrap();

        // add the first yes vote from voter
        let ballot = Ballot {
            weight: vote_power,
            vote: Vote::Yes,
        };
        BALLOTS.save(deps.storage, (id, &info.sender), &ballot)?;

        let res = Response::new()
            .add_attribute("action", "propose")
            .add_attribute("sender", info.sender)
            .add_attribute("proposal_id", id.to_string())
            .add_attribute("status", format!("{:?}", prop.status));

        match proposal_type {
            ProposalType::Send { to, amount } => Ok(res
                .add_attribute("type", "send")
                .add_attribute("send_to", to)
                .add_attribute("send_amount", amount)),
            ProposalType::AddVoter {
                address,
                vote_weight,
            } => Ok(res
                .add_attribute("type", "add_voter")
                .add_attribute("voter", address)
                .add_attribute("vote_weight", vote_weight.to_string())),
            ProposalType::RemoveVoter { address } => Ok(res
                .add_attribute("type", "remove_voter")
                .add_attribute("voter", address)),
        }
    }

    pub fn execute(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        proposal_id: u64,
    ) -> Result<Response, ContractError> {
        // anyone can trigger this if the vote passed
        let mut prop = PROPOSALS.load(deps.storage, proposal_id)?;
        // we allow execution even after the proposal "expiration" as long as all vote come in before
        // that point. If it was approved on time, it can be executed any time.
        prop.update_status(&env.block);
        if prop.status != Status::Passed {
            return Err(ContractError::WrongExecuteStatus {});
        }

        // set it to executed
        prop.status = Status::Executed;
        PROPOSALS.save(deps.storage, proposal_id, &prop)?;
        let proposal_type = PROPOSAL_ID_TO_TYPE.load(deps.storage, proposal_id).unwrap();

        // dispatch all proposed messages
        let res = Response::new()
            .add_messages(prop.msgs)
            .add_attribute("action", "execute")
            .add_attribute("sender", info.sender)
            .add_attribute("proposal_id", proposal_id.to_string());

        match proposal_type {
            ProposalType::Send { to, amount } => {
                let cw20_address = CW20_ADDRESS.load(deps.storage)?;
                let execute_msg = Cw20ExecuteMsg::Transfer {
                    recipient: to.clone().into(),
                    amount,
                };
                let bin_exec_msg = to_binary(&execute_msg).unwrap();
                let wasm_msg = WasmMsg::Execute {
                    contract_addr: cw20_address.into(),
                    msg: bin_exec_msg,
                    funds: vec![],
                };

                Ok(res
                    .add_message(wasm_msg)
                    .add_attribute("action", "send")
                    .add_attribute("send_to", to)
                    .add_attribute("send_amount", amount))
            }
            ProposalType::AddVoter {
                address,
                vote_weight,
            } => {
                VOTERS.save(deps.storage, &address, &vote_weight)?;
                CONFIG.update(deps.storage, |mut config| -> StdResult<_> {
                    let new_total_weight = config.total_weight + vote_weight;
                    config.total_weight = new_total_weight;
                    Ok(config)
                })?;

                Ok(res
                    .add_attribute("action", "add_voter")
                    .add_attribute("voter", address)
                    .add_attribute("vote_weight", vote_weight.to_string()))
            }
            ProposalType::RemoveVoter { address } => {
                let vote_weight = VOTERS.load(deps.storage, &address)?; // check it exists
                VOTERS.remove(deps.storage, &address);
                CONFIG.update(deps.storage, |mut config| -> StdResult<_> {
                    let new_total_weight = config.total_weight - vote_weight;
                    config.total_weight = new_total_weight;
                    Ok(config)
                })?;

                Ok(res
                    .add_attribute("action", "remove_voter")
                    .add_attribute("voter", address))
            }
        }
    }
}

pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Threshold {} => to_binary(&query::threshold(deps)?),
        QueryMsg::Proposal { proposal_id } => to_binary(&query::proposal(deps, env, proposal_id)?),
        QueryMsg::Vote { proposal_id, voter } => to_binary(&query::vote(deps, proposal_id, voter)?),
        QueryMsg::ListProposals { start_after, limit } => {
            to_binary(&query::list_proposals(deps, env, start_after, limit)?)
        }
        QueryMsg::ReverseProposals {
            start_before,
            limit,
        } => to_binary(&query::reverse_proposals(deps, env, start_before, limit)?),
        QueryMsg::ListVotes {
            proposal_id,
            start_after,
            limit,
        } => to_binary(&query::list_votes(deps, proposal_id, start_after, limit)?),
        QueryMsg::Voter { address } => to_binary(&query::voter(deps, address)?),
        QueryMsg::ListVoters { start_after, limit } => {
            to_binary(&query::list_voters(deps, start_after, limit)?)
        }
    }
}

pub mod query {
    use cosmwasm_std::{BlockInfo, Order};
    use cw3::{
        Proposal, VoteInfo, VoteListResponse, VoteResponse, VoterDetail, VoterListResponse,
        VoterResponse,
    };
    use cw3_fixed_multisig::state::{BALLOTS, CONFIG, PROPOSALS, VOTERS};
    use cw_storage_plus::Bound;
    use cw_utils::ThresholdResponse;

    use crate::{
        msg::{ProposalListResponse, ProposalResponse},
        state::PROPOSAL_ID_TO_TYPE,
    };

    use super::*;

    pub fn threshold(deps: Deps) -> StdResult<ThresholdResponse> {
        let cfg = CONFIG.load(deps.storage)?;
        Ok(cfg.threshold.to_response(cfg.total_weight))
    }

    pub fn proposal(deps: Deps, env: Env, id: u64) -> StdResult<ProposalResponse> {
        let prop = PROPOSALS.load(deps.storage, id)?;
        let proposal_type = PROPOSAL_ID_TO_TYPE.load(deps.storage, id)?;
        let status = prop.current_status(&env.block);
        let threshold = prop.threshold.to_response(prop.total_weight);
        Ok(ProposalResponse {
            id,
            title: prop.title,
            description: prop.description,
            proposal_type,
            msgs: prop.msgs,
            status,
            expires: prop.expires,
            deposit: prop.deposit,
            proposer: prop.proposer,
            threshold,
        })
    }

    // settings for pagination
    const MAX_LIMIT: u32 = 30;
    const DEFAULT_LIMIT: u32 = 10;

    pub fn list_proposals(
        deps: Deps,
        env: Env,
        start_after: Option<u64>,
        limit: Option<u32>,
    ) -> StdResult<ProposalListResponse> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let start = start_after.map(Bound::exclusive);
        let proposals = PROPOSALS
            .range(deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|p| map_proposal(deps, &env.block, p))
            .collect::<StdResult<_>>()?;

        Ok(ProposalListResponse { proposals })
    }

    pub fn reverse_proposals(
        deps: Deps,
        env: Env,
        start_before: Option<u64>,
        limit: Option<u32>,
    ) -> StdResult<ProposalListResponse> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let end = start_before.map(Bound::exclusive);
        let props: StdResult<Vec<_>> = PROPOSALS
            .range(deps.storage, None, end, Order::Descending)
            .take(limit)
            .map(|p| map_proposal(deps, &env.block, p))
            .collect();

        Ok(ProposalListResponse { proposals: props? })
    }

    fn map_proposal(
        deps: Deps,
        block: &BlockInfo,
        item: StdResult<(u64, Proposal)>,
    ) -> StdResult<ProposalResponse> {
        item.map(|(id, prop)| {
            let status = prop.current_status(block);
            let threshold = prop.threshold.to_response(prop.total_weight);
            let proposal_type = PROPOSAL_ID_TO_TYPE.load(deps.storage, id).unwrap();

            ProposalResponse {
                id,
                title: prop.title,
                description: prop.description,
                proposal_type,
                msgs: prop.msgs,
                status,
                deposit: prop.deposit,
                proposer: prop.proposer,
                expires: prop.expires,
                threshold,
            }
        })
    }

    pub fn vote(deps: Deps, proposal_id: u64, voter: String) -> StdResult<VoteResponse> {
        let voter = deps.api.addr_validate(&voter)?;
        let ballot = BALLOTS.may_load(deps.storage, (proposal_id, &voter))?;
        let vote = ballot.map(|b| VoteInfo {
            proposal_id,
            voter: voter.into(),
            vote: b.vote,
            weight: b.weight,
        });
        Ok(VoteResponse { vote })
    }

    pub fn list_votes(
        deps: Deps,
        proposal_id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<VoteListResponse> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let start = start_after.map(|s| Bound::ExclusiveRaw(s.into()));

        let votes = BALLOTS
            .prefix(proposal_id)
            .range(deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| {
                item.map(|(addr, ballot)| VoteInfo {
                    proposal_id,
                    voter: addr.into(),
                    vote: ballot.vote,
                    weight: ballot.weight,
                })
            })
            .collect::<StdResult<_>>()?;

        Ok(VoteListResponse { votes })
    }

    pub fn voter(deps: Deps, voter: String) -> StdResult<VoterResponse> {
        let voter = deps.api.addr_validate(&voter)?;
        let weight = VOTERS.may_load(deps.storage, &voter)?;
        Ok(VoterResponse { weight })
    }

    pub fn list_voters(
        deps: Deps,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<VoterListResponse> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let start = start_after.map(|s| Bound::ExclusiveRaw(s.into()));

        let voters = VOTERS
            .range(deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| {
                item.map(|(addr, weight)| VoterDetail {
                    addr: addr.into(),
                    weight,
                })
            })
            .collect::<StdResult<_>>()?;

        Ok(VoterListResponse { voters })
    }
}

#[cfg(test)]
mod test {
    use crate::{
        msg::{ProposalListResponse, ProposalResponse},
        *,
    };

    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, Decimal, Uint128,
    };
    use cw3::{
        Status, VoteInfo, VoteListResponse, VoteResponse, VoterDetail, VoterListResponse,
        VoterResponse,
    };
    use cw3_fixed_multisig::msg::Voter;
    use cw_utils::{Duration, Threshold, ThresholdResponse};

    #[test]
    fn proper_instantiate() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("sender", &[]);

        let voters = vec![
            Voter {
                addr: "voter1".into(),
                weight: 1,
            },
            Voter {
                addr: "voter2".into(),
                weight: 2,
            },
        ];
        let threshold_percentage = 60;
        let msg = InstantiateMsg {
            cw20_address: Addr::unchecked("cw20_address"),
            max_voting_period: Duration::Height(10),
            voters: voters.clone(),
            threshold: Threshold::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
            },
        };
        let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // check the state
        let query_threshold_msg = QueryMsg::Threshold {};
        let bin_res = query(deps.as_ref(), env.clone(), query_threshold_msg).unwrap();
        let res: ThresholdResponse = from_binary(&bin_res).unwrap();
        assert_eq!(
            res,
            ThresholdResponse::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
                total_weight: voters.iter().map(|v| v.weight).sum(),
            }
        );

        let query_voters_msg = QueryMsg::ListVoters {
            start_after: None,
            limit: None,
        };
        let bin_res = query(deps.as_ref(), env.clone(), query_voters_msg).unwrap();
        let res: VoterListResponse = from_binary(&bin_res).unwrap();
        assert_eq!(
            res,
            VoterListResponse {
                voters: voters
                    .iter()
                    .map(|v| VoterDetail {
                        addr: v.addr.clone(),
                        weight: v.weight
                    })
                    .collect()
            }
        );
    }

    #[test]
    fn only_voter_propose() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("sender", &[]);

        let max_voting_period = 10;
        let threshold_percentage = 100;
        let voters = vec![
            Voter {
                addr: "voter1".into(),
                weight: 1,
            },
            Voter {
                addr: "voter2".into(),
                weight: 1,
            },
        ];
        let msg = InstantiateMsg {
            cw20_address: Addr::unchecked("cw20_address"),
            max_voting_period: Duration::Height(max_voting_period),
            voters: voters.clone(),
            threshold: Threshold::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
            },
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let proposal_title = "title";
        let proposal_description = "description";
        let proposal_type = msg::ProposalType::AddVoter {
            address: Addr::unchecked("new_voter"),
            vote_weight: 1,
        };
        let propose_msg = ExecuteMsg::Propose {
            title: proposal_title.to_string(),
            description: proposal_description.to_string(),
            proposal_type: proposal_type.clone(),
            msgs: vec![],
            latest: None,
        };

        let invalid_info = mock_info("invalid", &[]);
        let res = execute(
            deps.as_mut(),
            env.clone(),
            invalid_info,
            propose_msg.clone(),
        );
        assert!(res.is_err());

        let voter1_info = mock_info(&voters[0].addr, &[]);
        let res = execute(deps.as_mut(), env.clone(), voter1_info, propose_msg.clone());
        assert!(res.is_ok());

        // query the proposal state
        let query_list_proposals_msg = QueryMsg::ListProposals {
            start_after: None,
            limit: None,
        };
        let bin_res = query(deps.as_ref(), env.clone(), query_list_proposals_msg).unwrap();
        let res: ProposalListResponse = from_binary(&bin_res).unwrap();
        assert_eq!(res.proposals.len(), 1);

        let query_proposal_msg = QueryMsg::Proposal { proposal_id: 1 };
        let bin_res = query(deps.as_ref(), env.clone(), query_proposal_msg).unwrap();
        let res: ProposalResponse = from_binary(&bin_res).unwrap();
        assert_eq!(
            res,
            ProposalResponse {
                id: 1,
                title: proposal_title.to_string(),
                description: proposal_description.to_string(),
                proposal_type: proposal_type.clone(),
                msgs: vec![],
                status: Status::Open,
                expires: cw_utils::Expiration::AtHeight(env.block.height + max_voting_period),
                threshold: ThresholdResponse::AbsolutePercentage {
                    percentage: Decimal::percent(threshold_percentage),
                    total_weight: voters.iter().map(|v| v.weight).sum()
                },
                proposer: Addr::unchecked(voters[0].addr.clone()),
                deposit: None
            }
        );
    }

    #[test]
    fn only_voter_vote() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("sender", &[]);

        let max_voting_period = 10;
        let threshold_percentage = 100;
        let voters = vec![
            Voter {
                addr: "voter1".into(),
                weight: 1,
            },
            Voter {
                addr: "voter2".into(),
                weight: 1,
            },
        ];
        let msg = InstantiateMsg {
            cw20_address: Addr::unchecked("cw20_address"),
            max_voting_period: Duration::Height(max_voting_period),
            voters: voters.clone(),
            threshold: Threshold::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
            },
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let proposal_title = "title";
        let proposal_description = "description";
        let proposal_type = msg::ProposalType::AddVoter {
            address: Addr::unchecked("new_voter"),
            vote_weight: 1,
        };
        let propose_msg = ExecuteMsg::Propose {
            title: proposal_title.to_string(),
            description: proposal_description.to_string(),
            proposal_type: proposal_type.clone(),
            msgs: vec![],
            latest: None,
        };
        let voter1_info = mock_info(&voters[0].addr, &[]);
        execute(deps.as_mut(), env.clone(), voter1_info, propose_msg.clone()).unwrap();

        let proposal_id = 1;
        let vote = cw3::Vote::Yes;
        let vote_msg = ExecuteMsg::Vote {
            proposal_id,
            vote: vote.clone(),
        };

        let invalid_info = mock_info("invalid", &[]);
        let res = execute(deps.as_mut(), env.clone(), invalid_info, vote_msg.clone());
        assert!(res.is_err());

        let voter2_info = mock_info(&voters[1].addr, &[]);
        let res = execute(deps.as_mut(), env.clone(), voter2_info, vote_msg.clone());
        assert!(res.is_ok());

        // check the state
        let query_list_votes_msg = QueryMsg::ListVotes {
            proposal_id,
            start_after: None,
            limit: None,
        };
        let bin_res = query(deps.as_ref(), env.clone(), query_list_votes_msg).unwrap();
        let res: VoteListResponse = from_binary(&bin_res).unwrap();
        assert_eq!(res.votes.len(), 2); // of voter1 and voter2

        let query_proposal_msg = QueryMsg::Vote {
            proposal_id,
            voter: voters[1].addr.to_string(),
        };
        let bin_res = query(deps.as_ref(), env.clone(), query_proposal_msg).unwrap();
        let res: VoteResponse = from_binary(&bin_res).unwrap();
        assert!(res.vote.is_some());
        assert_eq!(
            res,
            VoteResponse {
                vote: Some(VoteInfo {
                    proposal_id,
                    voter: voters[1].addr.to_string(),
                    vote: vote.clone(),
                    weight: voters[1].weight,
                })
            }
        );
    }

    #[test]
    fn execute_after_proposal_passed() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("sender", &[]);

        let max_voting_period = 10;
        let threshold_percentage = 100;
        let voters = vec![Voter {
            addr: "voter1".into(),
            weight: 1,
        }];
        let msg = InstantiateMsg {
            cw20_address: Addr::unchecked("cw20_address"),
            max_voting_period: Duration::Height(max_voting_period),
            voters: voters.clone(),
            threshold: Threshold::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
            },
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let propose_msg = ExecuteMsg::Propose {
            title: "title".to_string(),
            description: "description".to_string(),
            proposal_type: msg::ProposalType::AddVoter {
                address: Addr::unchecked("new_voter"),
                vote_weight: 1,
            },
            msgs: vec![],
            latest: None,
        };
        let voter1_info = mock_info(&voters[0].addr, &[]);
        execute(deps.as_mut(), env.clone(), voter1_info, propose_msg.clone()).unwrap();

        let proposal_id = 1;
        let execute_proposal_msg = ExecuteMsg::Execute { proposal_id };
        let any_info = mock_info("any", &[]);
        let res = execute(deps.as_mut(), env.clone(), any_info, execute_proposal_msg);
        assert!(res.is_ok());
    }

    #[test]
    fn add_voter_after_proposal_passed() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("sender", &[]);

        let max_voting_period = 10;
        let threshold_percentage = 100;
        let voters = vec![Voter {
            addr: "voter1".into(),
            weight: 1,
        }];
        let msg = InstantiateMsg {
            cw20_address: Addr::unchecked("cw20_address"),
            max_voting_period: Duration::Height(max_voting_period),
            voters: voters.clone(),
            threshold: Threshold::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
            },
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let new_voter = "new_voter";
        let proposal_type = msg::ProposalType::AddVoter {
            address: Addr::unchecked(new_voter),
            vote_weight: 1,
        };
        let propose_msg = ExecuteMsg::Propose {
            title: "title".to_string(),
            description: "description".to_string(),
            proposal_type: proposal_type.clone(),
            msgs: vec![],
            latest: None,
        };
        let voter1_info = mock_info(&voters[0].addr, &[]);
        execute(deps.as_mut(), env.clone(), voter1_info, propose_msg.clone()).unwrap();

        let proposal_id = 1;
        let execute_proposal_msg = ExecuteMsg::Execute { proposal_id };
        let any_info = mock_info("any", &[]);
        let res = execute(deps.as_mut(), env.clone(), any_info, execute_proposal_msg);
        assert!(res.is_ok());

        // check the state
        let query_voter_msg = QueryMsg::Voter {
            address: new_voter.to_string(),
        };
        let bin_res = query(deps.as_ref(), env.clone(), query_voter_msg).unwrap();
        let res: VoterResponse = from_binary(&bin_res).unwrap();
        assert_eq!(res, VoterResponse { weight: Some(1) });
    }

    #[test]
    fn remove_voter_after_proposal_passed() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("sender", &[]);

        let max_voting_period = 10;
        let threshold_percentage = 50;
        let voters = vec![
            Voter {
                addr: "voter1".into(),
                weight: 1,
            },
            Voter {
                addr: "voter2".into(),
                weight: 1,
            },
        ];
        let total_weight = voters.iter().map(|v| v.weight).sum::<u64>();
        let msg = InstantiateMsg {
            cw20_address: Addr::unchecked("cw20_address"),
            max_voting_period: Duration::Height(max_voting_period),
            voters: voters.clone(),
            threshold: Threshold::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
            },
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let remove_voter = voters[1].clone();
        let proposal_type = msg::ProposalType::RemoveVoter {
            address: Addr::unchecked(&remove_voter.addr),
        };
        let propose_msg = ExecuteMsg::Propose {
            title: "title".to_string(),
            description: "description".to_string(),
            proposal_type: proposal_type.clone(),
            msgs: vec![],
            latest: None,
        };
        let voter1_info = mock_info(&voters[0].addr, &[]);
        execute(deps.as_mut(), env.clone(), voter1_info, propose_msg.clone()).unwrap();

        let proposal_id = 1;
        let execute_proposal_msg = ExecuteMsg::Execute { proposal_id };
        let any_info = mock_info("any", &[]);
        let res = execute(deps.as_mut(), env.clone(), any_info, execute_proposal_msg);
        assert!(res.is_ok());

        // check the state
        let query_voter_msg = QueryMsg::Voter {
            address: remove_voter.addr.to_string(),
        };
        let bin_res = query(deps.as_ref(), env.clone(), query_voter_msg).unwrap();
        let res: VoterResponse = from_binary(&bin_res).unwrap();
        assert_eq!(res, VoterResponse { weight: None });

        let query_vote_list_msg = QueryMsg::ListVoters {
            start_after: None,
            limit: None,
        };
        let bin_res = query(deps.as_ref(), env.clone(), query_vote_list_msg).unwrap();
        let res: VoterListResponse = from_binary(&bin_res).unwrap();
        assert_eq!(
            res,
            VoterListResponse {
                voters: vec![VoterDetail {
                    addr: voters[0].addr.clone(),
                    weight: voters[0].weight,
                }],
            }
        );

        let query_threshold_msg = QueryMsg::Threshold {};
        let bin_res = query(deps.as_ref(), env.clone(), query_threshold_msg).unwrap();
        let res: ThresholdResponse = from_binary(&bin_res).unwrap();
        assert_eq!(
            res,
            ThresholdResponse::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
                total_weight: total_weight - remove_voter.weight,
            }
        );
    }

    #[test]
    fn send_cw20_after_proposal_passed() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("sender", &[]);

        let max_voting_period = 10;
        let threshold_percentage = 100;
        let voters = vec![Voter {
            addr: "voter1".into(),
            weight: 1,
        }];
        let msg = InstantiateMsg {
            cw20_address: Addr::unchecked("cw20_address"),
            max_voting_period: Duration::Height(max_voting_period),
            voters: voters.clone(),
            threshold: Threshold::AbsolutePercentage {
                percentage: Decimal::percent(threshold_percentage),
            },
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let receiver = "receiver";
        let proposal_type = msg::ProposalType::Send {
            to: Addr::unchecked(receiver),
            amount: Uint128::from(100u128),
        };
        let propose_msg = ExecuteMsg::Propose {
            title: "title".to_string(),
            description: "description".to_string(),
            proposal_type: proposal_type.clone(),
            msgs: vec![],
            latest: None,
        };
        let voter1_info = mock_info(&voters[0].addr, &[]);
        execute(deps.as_mut(), env.clone(), voter1_info, propose_msg.clone()).unwrap();

        let proposal_id = 1;
        let execute_proposal_msg = ExecuteMsg::Execute { proposal_id };
        let any_info = mock_info("any", &[]);
        let res = execute(deps.as_mut(), env.clone(), any_info, execute_proposal_msg);
        assert!(res.is_ok());
    }

    mod integration {
        use super::*;

        use cw20::{BalanceResponse, Cw20Coin};
        use cw20_base::{
            contract::{
                execute as cw20_execute, instantiate as cw20_instantiate, query as cw20_query,
            },
            msg::InstantiateMsg as Cw20InstantiateMsg,
        };
        use cw_multi_test::{App, ContractWrapper, Executor};

        #[test]
        fn propose_and_execute_and_send_cw20() {
            let mut app = App::default();

            let preditable_foundation_contract_address = Addr::unchecked("contract1");

            let cw20_code = ContractWrapper::new(cw20_execute, cw20_instantiate, cw20_query);
            let cw20_code_id = app.store_code(Box::new(cw20_code));
            let foundation_amount = Uint128::new(1_000_000);
            let cw20_address = app
                .instantiate_contract(
                    cw20_code_id,
                    Addr::unchecked("sender"),
                    &Cw20InstantiateMsg {
                        name: "name".to_string(),
                        symbol: "symbol".to_string(),
                        decimals: 6,
                        initial_balances: vec![Cw20Coin {
                            address: preditable_foundation_contract_address.to_string(),
                            amount: foundation_amount,
                        }],
                        mint: None,
                        marketing: None,
                    },
                    &[],
                    "Cw20Contract",
                    None,
                )
                .unwrap();

            let voter_address = Addr::unchecked("voter");
            let foundation_code = ContractWrapper::new(execute, instantiate, query);
            let foundation_code_id = app.store_code(Box::new(foundation_code));
            let foundation_address = app
                .instantiate_contract(
                    foundation_code_id,
                    Addr::unchecked("sender"),
                    &InstantiateMsg {
                        cw20_address: cw20_address.clone(),
                        max_voting_period: Duration::Height(10),
                        voters: vec![Voter {
                            addr: voter_address.to_string(),
                            weight: 1,
                        }],
                        threshold: Threshold::AbsolutePercentage {
                            percentage: Decimal::percent(50),
                        },
                    },
                    &[],
                    "FoundationContract",
                    None,
                )
                .unwrap();
            assert_eq!(preditable_foundation_contract_address, foundation_address);

            let send_to = Addr::unchecked("send_to");
            let send_amount = Uint128::from(500u128);

            // check balance before send proposal
            let query_msg = cw20_base::msg::QueryMsg::Balance {
                address: foundation_address.to_string(),
            };
            let res: BalanceResponse = app
                .wrap()
                .query_wasm_smart(&cw20_address, &query_msg)
                .unwrap();
            assert_eq!(res.balance, foundation_amount);

            let query_msg = cw20_base::msg::QueryMsg::Balance {
                address: send_to.to_string(),
            };
            let res: BalanceResponse = app
                .wrap()
                .query_wasm_smart(&cw20_address, &query_msg)
                .unwrap();
            assert_eq!(res.balance, Uint128::zero());

            // send proposal
            let proposal_type = msg::ProposalType::Send {
                to: send_to.clone(),
                amount: send_amount.clone(),
            };
            let propose_msg = ExecuteMsg::Propose {
                title: "title".to_string(),
                description: "description".to_string(),
                proposal_type: proposal_type.clone(),
                msgs: vec![],
                latest: None,
            };
            app.execute_contract(voter_address, foundation_address.clone(), &propose_msg, &[])
                .unwrap(); // vote is passed because of only one voter

            // execute proposal
            let proposal_id = 1;
            let execute_proposal_msg = ExecuteMsg::Execute { proposal_id };
            app.execute_contract(
                Addr::unchecked("any"),
                foundation_address.clone(),
                &execute_proposal_msg,
                &[],
            )
            .unwrap();

            // check balance before send proposal
            let query_msg = cw20_base::msg::QueryMsg::Balance {
                address: foundation_address.to_string(),
            };
            let res: BalanceResponse = app
                .wrap()
                .query_wasm_smart(&cw20_address, &query_msg)
                .unwrap();
            assert_eq!(
                res.balance,
                foundation_amount.checked_sub(send_amount).unwrap()
            );

            let query_msg = cw20_base::msg::QueryMsg::Balance {
                address: send_to.to_string(),
            };
            let res: BalanceResponse = app
                .wrap()
                .query_wasm_smart(&cw20_address, &query_msg)
                .unwrap();
            assert_eq!(res.balance, send_amount);
        }
    }
}
