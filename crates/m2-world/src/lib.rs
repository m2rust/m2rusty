//! CPU-side world logic of the outdoor maps, ported from
//! `client/src/game/{AreaTerrain,TerrainPatch,MapOutdoor*,Area}.cpp` and
//! `neoeter/src/terrain`. No rendering and no Bevy: the renderer uploads the
//! plain vectors produced here.
//!
//! Skipped on purpose (culling or device only): the patch proxy list and
//! terrain quadtree (`MapOutdoorQuadtree.cpp`, `TerrainQuadtree.cpp`, view
//! frustum code), the software-transform render pipeline (fog, projection),
//! character shadow textures, marked-area splats, `WE_GetHeightMapValue`
//! (world editor), and object heights in `CMapOutdoor::GetHeight` (they need
//! the model collision data of the objects; use [`OutdoorMap::terrain_height`]).

pub mod area;
pub mod lighting;
pub mod load;
pub mod mesh;
pub mod outdoor;
pub mod sort;
pub mod splat;

pub use load::{LoadedMap, load_map};
pub use outdoor::{OutdoorMap, TerrainCell, float_to_long};

/// `CTerrainImpl::TERRAIN_XSIZE`: world units per terrain side.
pub const TERRAIN_WORLD: i32 = 25600;
/// `CTerrainImpl::CELLSCALE`.
pub const CELLSCALE: i32 = 200;
/// `AROUND_AREA_NUM`.
pub const AROUND_AREA_NUM: i32 = 9;
