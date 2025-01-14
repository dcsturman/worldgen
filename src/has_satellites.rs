use crate::system::Star;
use crate::system_tables::ZoneTable;
use crate::world::{Satellites, World};

pub trait HasSatellites {
    fn get_num_satellites(&self) -> usize;
    fn get_satellite(&self, orbit: usize) -> Option<&World>;
    fn get_satellites_mut(&mut self) -> &mut Satellites;
    fn push_satellite(&mut self, satellite: World);
    fn sort_satellites(&mut self) {
        self.get_satellites_mut()
            .sats
            .sort_by(|a, b| a.orbit.cmp(&b.orbit));
    }

    fn clean_satellites(&mut self) {
        self.sort_satellites();
        let ring_indices: Vec<usize> = self
            .get_satellites_mut()
            .sats
            .iter()
            .enumerate()
            .filter(|(_, satellite)| satellite.size == 0)
            .map(|(index, _)| index)
            .collect();

        if ring_indices.is_empty() {
            return;
        }

        for ring in ring_indices.iter().skip(1) {
            self.get_satellites_mut().sats.remove(*ring);
        }
        self.get_satellites_mut().sats[ring_indices[0]].name = "Ring System".to_string();
    }

    fn determine_num_satellites(&self) -> i32;

    fn gen_satellite_orbit(&self, is_ring: bool) -> usize;

    fn gen_satellite(&mut self, system_zones: &ZoneTable, main_world: &World, star: &Star);
}
