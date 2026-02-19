use serde::{Deserialize, Serialize};

use crate::systems::world::World;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TradeState {
    pub origin_world: World,
    pub dest_world: Option<World>,
    pub available_goods: AvailableGoodsTable,
    pub available_passengers: Option<AvailablePassengers>,
    pub ship_manifest: ShipManifest,
    pub buyer_broker_skill: i16,
    pub seller_broker_skill: i16,
    pub steward_skill: i16,
    pub illegal_goods: bool,
}
