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
            ProposalType::Send { to, amount } => {
                res.clone()
                    .add_attribute("action", "send")
                    .add_attribute("send_to", to)
                    .add_attribute("send_amount", amount);
                Ok(res)
            }
            ProposalType::AddVoter {
                address,
                vote_weight,
                info,
            } => {
                res.clone()
                    .add_attribute("action", "add_voter")
                    .add_attribute("voter", address)
                    .add_attribute("vote_weight", vote_weight.to_string())
                    .add_attribute("voter_info", info);
                Ok(res)
            }
            ProposalType::RemoveVoter {
                address,
                vote_weight,
            } => {
                res.clone()
                    .add_attribute("action", "remove_voter")
                    .add_attribute("vote_weight", vote_weight.to_string())
                    .add_attribute("voter", address);
                Ok(res)
            }
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

                res.clone()
                    .add_message(wasm_msg)
                    .add_attribute("action", "send")
                    .add_attribute("send_to", to)
                    .add_attribute("send_amount", amount);
                Ok(res)
            }
            ProposalType::AddVoter {
                address,
                vote_weight,
                info,
            } => {
                VOTERS.save(deps.storage, &address, &vote_weight)?;
                CONFIG.update(deps.storage, |mut config| -> StdResult<_> {
                    let new_total_weight = config.total_weight + vote_weight;
                    config.total_weight = new_total_weight;
                    Ok(config)
                })?;

                res.clone()
                    .add_attribute("action", "add_voter")
                    .add_attribute("voter", address)
                    .add_attribute("vote_weight", vote_weight.to_string())
                    .add_attribute("voter_info", info);
                Ok(res)
            }
            ProposalType::RemoveVoter {
                address,
                vote_weight,
            } => {
                VOTERS.remove(deps.storage, &address);
                CONFIG.update(deps.storage, |mut config| -> StdResult<_> {
                    let new_total_weight = config.total_weight - vote_weight;
                    config.total_weight = new_total_weight;
                    Ok(config)
                })?;

                res.clone()
                    .add_attribute("action", "remove_voter")
                    .add_attribute("vote_weight", vote_weight.to_string())
                    .add_attribute("voter", address);
                Ok(res)
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
