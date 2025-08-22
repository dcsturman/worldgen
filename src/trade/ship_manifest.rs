#[derive(Debug, Clone, Default)]
pub struct ShipManifest {
    pub high_passengers: i32,
    pub medium_passengers: i32,
    pub basic_passengers: i32,
    pub low_passengers: i32,
    pub freight_lot_indices: Vec<usize>,
}

const HIGH_COST: [i32; 7] = [0, 9000, 14000, 21000, 34000, 60000, 210000];
const MEDIUM_COST: [i32; 7] = [0, 6500, 10000, 14000, 23000, 40000, 130000];
const BASIC_COST: [i32; 7] = [0, 2000, 3000, 5000, 8000, 14000, 55000];
const LOW_COST: [i32; 7] = [0, 700, 1300, 2200, 3900, 7200, 27000];
const FREIGHT_COST: [i32; 7] = [0, 1000, 1600, 2600, 4400, 8500, 32000];

impl ShipManifest {
    pub fn passenger_revenue(&self, distance: i32) -> i32 {
        let distance_index = distance.clamp(1, 6) as usize;
        let high_cost = HIGH_COST[distance_index] * self.high_passengers;
        let medium_cost = MEDIUM_COST[distance_index] * self.medium_passengers;
        let basic_cost = BASIC_COST[distance_index] * self.basic_passengers;
        let low_cost = LOW_COST[distance_index] * self.low_passengers;
        high_cost + medium_cost + basic_cost + low_cost
    }

    pub fn freight_revenue(&self, distance: i32) -> i32 {
        let distance_index = distance.clamp(1, 6) as usize;
        FREIGHT_COST[distance_index] * self.freight_lot_indices.len() as i32
    }
}
