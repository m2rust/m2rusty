//! Unit tests for the map formats: failure paths and C++ quirks the real
//! data does not exercise.

use m2_formats::map::*;

fn le16(v: &[u16]) -> Vec<u8> {
    v.iter().flat_map(|w| w.to_le_bytes()).collect()
}

#[test]
fn ids_and_paths() {
    assert_eq!(terrain_id(1, 2), 1002);
    assert_eq!(terrain_file_name_untruncated(b"m", 1, 2, "a.txt"), b"m\\001002\\a.txt");
    let long = vec![b'x'; 100];
    assert_eq!(terrain_file_name(&long, 0, 0, "height.raw").len(), 64);
    assert_eq!(atol(b" -12x"), -12);
    assert_eq!(atol(b"4294967297"), 4_294_967_297);
}

#[test]
fn texture_set_name_quirk() {
    // find_first_of("textureset") != 0: any first byte in the set counts.
    assert_eq!(texture_set_file_name(b"textureset\\a.txt"), b"textureset\\a.txt");
    assert_eq!(texture_set_file_name(b"eter.txt"), b"eter.txt");
    assert_eq!(texture_set_file_name(b"a.txt"), b"textureset\\a.txt");
}

#[test]
fn setting_errors_keep_partial_state() {
    let s = MapSetting::parse(b"ScriptType MapSetting\nViewRadius 0\nCellScale 1\nHeightScale 1\nMapSize 1 1\nTextureSet a\n", |_| {
        unreachable!()
    });
    assert!(matches!(s.error, Some(SettingError::ViewRadius)));
    let s = MapSetting::parse(b"ScriptType MapSetting\n", |_| unreachable!());
    assert!(matches!(s.error, Some(SettingError::MissingKey("viewradius"))));
    let ok = MapSetting::parse(
        b"ScriptType MapSetting\nViewRadius 64\nCellScale 1\nHeightScale 0.5\nMapSize 0 3\nBasePosition 5 6\nTextureSet a\nTerrainVisible 0\nEnvironment x.msenv\n",
        |name| {
            assert_eq!(name, b"textureset\\a");
            Ok(TextureSet::default())
        },
    );
    assert!(ok.error.is_none());
    assert_eq!(ok.terrain_count, (0, 0));
    assert_eq!(ok.base, (5, 6));
    assert!(!ok.terrain_visible);
    assert_eq!(ok.environment, b"x.msenv");
}

#[test]
fn map_property() {
    let p = MapProperty::parse(b"ScriptType MapProperty\nMapType \"Indoor\"\nParentMapName parent\n");
    assert!(p.error.is_none());
    assert_eq!(p.map_type, MapType::Indoor);
    assert_eq!(p.data_directory(b"me"), b"parent");
    let p = MapProperty::parse(b"ScriptType Other\nMapType Outdoor\nParentMapName parent\n");
    assert!(matches!(p.error, Some(PropertyError::ScriptType)));
    assert_eq!(p.parent_map_name, b"parent");
}

#[test]
fn texture_set_defaults() {
    let data = b"TextureSet\nTextureCount 2\nStart Texture002\n a.dds\n 2\n 3\n 0.5\n 0.25\n 0\n 7\n 9\nEnd Texture002\n";
    let set = TextureSet::parse(data, b"ts", TERRAIN_TEX_COORD_BASE).unwrap();
    assert_eq!(set.textures.len(), 3);
    assert_eq!(set.textures[1], TerrainTexture::default());
    let t = &set.textures[2];
    assert_eq!((t.u_scale, t.begin, t.end, t.splat), (2.0, 7, 9, false));
    assert_eq!(t.transform[13], -0.25);
    assert!(TextureSet::parse(b"TextureSet\n", b"ts", 1.0).is_err());
}

#[test]
fn area_rotation_and_filter() {
    let data = b"AreaDataFile\nObjectCount 3\n\
        Start Object000\n 1 2 3\n 10\n 50#6#7\n 1.5\n 1 2\nEnd Object\n\
        Start Object002\n 1 2 3\n 20\n 90\nEnd Object\n\
        Start Object001\n 0 0 0\n 99\nEnd Object\n";
    let objects = parse_area_data(data, |crc| crc != 99).unwrap();
    assert_eq!(objects.len(), 2);
    // C++ drops the byte before the first '#': "50" reads as 5.
    assert_eq!((objects[0].yaw, objects[0].pitch, objects[0].roll), (5.0, 6.0, 7.0));
    assert_eq!(objects[0].height_bias, 1.5);
    assert_eq!(objects[0].portal_ids[..3], [1, 2, 0]);
    assert_eq!((objects[1].yaw, objects[1].roll), (0.0, 90.0));
    assert!(parse_area_data(b"ObjectCount 1\n", |_| true).is_err());
}

#[test]
fn ambience() {
    let data = b"AreaAmbienceDataFile\nObjectCount 1\nStart Object000\n 1 2 3\n 4\n 500\n 12.5\nEnd Object\n";
    let o = parse_area_ambience_data(data, |_| true).unwrap();
    assert_eq!((o[0].crc, o[0].range, o[0].max_volume_area_percentage), (4, 500, 12.5));
}

#[test]
fn monsters() {
    let data = b"//c\nM 1 2 3 4 0 5 0 100 7 112\nG 1 2 3 4 0 0 0 100 7 3\nm 1 2\nx 1 2 3 4 5 6 7 8 9 10\n";
    let a = parse_monster_areas(data);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].kind, MonsterAreaKind::Monster);
    assert_eq!((a[0].id, a[0].count, a[0].direction), (112, 7, 5));
    assert_eq!(a[0].bounds(), (-2, -2, 4, 6));
    assert_eq!(a[1].kind, MonsterAreaKind::Group);
}

#[test]
fn atlas_repeats_previous_values() {
    let e = parse_atlas_info(b"a 1 2 3 4\nb 5\n\n");
    assert_eq!(e.len(), 3);
    assert_eq!((e[1].name.as_slice(), e[1].base_x, e[1].base_y, e[1].size_x), (b"b".as_slice(), 5, 2, 3));
    assert_eq!(e[2].name, b"b");
    assert_eq!(e[0].end_x, 1 + 3 * 25600);
}

#[test]
fn terrain_loaders() {
    let mut t = Terrain::default();
    assert!(!t.load_height_map(&[0; 10]));
    assert!(!t.load_attr_map(&le16(&[2634, 256, 255])));
    let mut attr = le16(&[2634, 256, 256]);
    attr.extend(vec![3; 256 * 256]);
    assert!(t.load_attr_map(&attr));
    assert_eq!(t.attr_map[100], 3);
    attr.pop();
    assert!(!t.load_attr_map(&attr));

    assert!(!t.load_shadow_map(&[0; 4]));
    assert!(t.load_shadow_map(&vec![1; 256 * 256 * 2]));
    assert_eq!(t.shadow_map[0], 0x0101);

    // Water: i32 heights load; u16 heights and bad sizes fail and reset.
    let mut w = le16(&[5426, 128, 128]);
    w.push(2);
    w.extend(vec![1; 128 * 128]);
    w.extend(10i32.to_le_bytes());
    w.extend((-4i32).to_le_bytes());
    assert!(t.load_water_map(Some(&w)));
    assert_eq!((t.num_water, t.water_heights[0], t.water_heights[1], t.water_heights[2]), (2, 10, -4, -1));
    let mut w16 = le16(&[5426, 128, 128]);
    w16.push(1);
    w16.extend(vec![1; 128 * 128]);
    w16.extend([1, 0]);
    assert!(!t.load_water_map(Some(&w16)));
    assert_eq!((t.num_water, t.water_map[0], t.water_heights[0]), (0, 0xFF, -1));
    assert!(!t.load_water_map(None));
}

#[test]
fn normals_and_tile_counts() {
    let mut t = Terrain { height_scale: 0.5, ..Terrain::default() };
    t.calculate_normals();
    // Flat terrain: straight up, 127 long.
    assert_eq!(&t.normal_map[..3], &[0, 0, 127]);
    t.tile_map.fill(2);
    t.count_tiles();
    assert_eq!(t.tile_count[2], 258 * 258);
    assert_eq!(t.tile_count[0], 0);
    // Every tile lands in at least one patch.
    let total: u32 = t.patch_tile_count.iter().map(|p| p[2]).sum();
    assert!(total >= 258 * 258);
}

#[test]
fn area_property() {
    assert_eq!(parse_area_property(b"ScriptType AreaProperty\nAreaName \"x y\"\n").unwrap().area_name, b"x y");
    assert!(parse_area_property(b"ScriptType Other\nAreaName a\n").is_err());
    assert!(parse_area_property(b"ScriptType AreaProperty\n").is_err());
}
