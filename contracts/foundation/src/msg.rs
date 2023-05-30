use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, CosmosMsg, Empty, Uint128};
use cw3::{DepositInfo, Status, Vote};
use cw3_fixed_multisig::msg::Voter;
use cw_utils::{Duration, Expiration, Threshold, ThresholdResponse};

pub use cw3_fixed_multisig::msg::ExecuteMsg as Cw3ExecuteMsg;

#[cw_serde]
pub struct InstantiateMsg {
    pub voters: Vec<Voter>,
    pub threshold: Threshold,
    pub max_voting_period: Duration,
    pub cw20_address: Addr,
}

#[cw_serde]
pub enum ProposalType {
    Send {
        to: Addr,
        amount: Uint128,
    },
    AddVoter {
        address: Addr,
        vote_weight: u64,
        info: String,
    },
    RemoveVoter {
        address: Addr,
        vote_weight: u64,
    },
}

#[cw_serde]
pub enum ExecuteMsg {
    Propose {
        title: String,
        description: String,
        proposal_type: ProposalType,
        msgs: Vec<CosmosMsg<Empty>>,
        latest: Option<Expiration>,
    },
    Vote {
        proposal_id: u64,
        vote: Vote,
    },
    Execute {
        proposal_id: u64,
    },
    Close {
        proposal_id: u64,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ProposalResponse)]
    Proposal { proposal_id: u64 },
    #[returns(cw_utils::ThresholdResponse)]
    Threshold {},
    #[returns(cw3::ProposalListResponse)]
    ListProposals {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    #[returns(cw3::ProposalListResponse)]
    ReverseProposals {
        start_before: Option<u64>,
        limit: Option<u32>,
    },
    #[returns(cw3::VoteResponse)]
    Vote { proposal_id: u64, voter: String },
    #[returns(cw3::VoteListResponse)]
    ListVotes {
        proposal_id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(cw3::VoterResponse)]
    Voter { address: String },
    #[returns(cw3::VoterListResponse)]
    ListVoters {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct ProposalResponse<T = Empty> {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub msgs: Vec<CosmosMsg<T>>,
    pub status: Status,
    pub expires: Expiration,
    /// This is the threshold that is applied to this proposal. Both
    /// the rules of the voting contract, as well as the total_weight
    /// of the voting group may have changed since this time. That
    /// means that the generic `Threshold{}` query does not provide
    /// valid information for existing proposals.
    pub threshold: ThresholdResponse,
    pub proposer: Addr,
    pub deposit: Option<DepositInfo>,
}

#[cw_serde]
pub struct ProposalListResponse<T = Empty> {
    pub proposals: Vec<ProposalResponse<T>>,
}
