use cosmwasm_std::Addr;
use cw_storage_plus::Item;

pub const MINTABLE_BLOCK_HEIGHT: Item<u64> = Item::new("mintable_block_height");
pub const CW20_ADDRESS: Item<Addr> = Item::new("cw20_address");
pub const OWNER: Item<Addr> = Item::new("owner");
