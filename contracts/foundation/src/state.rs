use crate::msg::ProposalType;

use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

pub const PROPOSAL_ID_TO_TYPE: Map<u64, ProposalType> = Map::new("proposal_id_to_type");
pub const CW20_ADDRESS: Item<Addr> = Item::new("cw20_address");
