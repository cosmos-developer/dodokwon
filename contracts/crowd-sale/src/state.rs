use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::Item;

pub const MINTABLE_BLOCK_HEIGHT: Item<u64> = Item::new("mintable_block_height");
pub const CW20_ADDRESS: Item<Addr> = Item::new("cw20_address");
pub const OWNER: Item<Addr> = Item::new("owner");
pub const UDODOKWAN_UUSD: Item<Decimal> = Item::new("udodokwan_per_uusd");
