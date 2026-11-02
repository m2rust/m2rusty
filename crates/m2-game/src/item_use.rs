//! Inventory item-on-item routing from uiinventory.py. The server owns the result.
use crate::player::{ItemCatalog, ItemStack};

pub fn applies_to_item(source: &ItemStack, catalog: &ItemCatalog) -> bool {
    source.flags & (1 << 14) != 0 || catalog.get(source.vnum).is_some_and(|data| {
        data.table.flags & (1 << 14) != 0 || matches!(data.table.item_type, 10 | 21)
            || data.table.item_type == 3 && matches!(data.table.sub_type, 2 | 14 | 17..=22 | 24..=30)
    })
}

pub fn can_apply(source: &ItemStack, target: &ItemStack, catalog: &ItemCatalog) -> Result<(), &'static str> {
    let src = catalog.get(source.vnum).ok_or("Item information is still loading.")?;
    let dst = catalog.get(target.vnum).ok_or("Item information is still loading.")?;
    if source.flags & (1 << 14) != 0 || src.table.flags & (1 << 14) != 0 { return Ok(()); }
    let equipment = matches!(dst.table.item_type, 1 | 2);
    let attrs = target.attributes.iter().take(5).filter(|attr| attr.bType != 0).count();
    let valid = match (src.table.item_type, src.table.sub_type) {
        (10, kind) => equipment && src.table.wear_flags & dst.table.wear_flags != 0 && target.sockets.iter().any(|socket| *socket == 2 || kind == 0 && *socket == 1),
        (21, _) => dst.table.item_type == 20,
        (3, 2) => dst.table.item_type != 8 && dst.table.refined_vnum != 0,
        (3, 14) => dst.table.flags & 1 != 0 && target.sockets.iter().any(|s| *s > 2),
        (3, 17) => dst.table.item_type == 1 && target.sockets.contains(&28960),
        (3, 18 | 24) => equipment && attrs > 0,
        (3, 19 | 22) => equipment && attrs < 5,
        (3, 20) => dst.table.item_type == 2 && matches!(dst.table.sub_type, 3..=5) && target.sockets[1] < 3,
        (3, 21) => dst.table.item_type == 2 && matches!(dst.table.sub_type, 3..=5) && target.sockets[0] < target.sockets[1] && source.vnum == accessory_material(target.vnum, dst.table.sub_type),
        (3, 29) => dst.table.item_type == 34,
        (3, 30) => dst.table.item_type == 33,
        (3, 25..=28) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err("This item cannot be used on that item.") }
}

/// constInfo.GET_ACCESSORY_MATERIAL_VNUM, including the newer jewel sets.
fn accessory_material(vnum: u32, subtype: u8) -> u32 {
    let base = vnum / 10 * 10;
    for [gem, wrist, neck, ear] in [[50634,14420,16220,17220], [50635,14500,16500,17500], [50636,14520,16520,17520], [50637,14540,16540,17540], [50638,14560,16560,17560]] {
        if base == match subtype { 3 => wrist, 4 => neck, 5 => ear, _ => return 0 } { return gem; }
    }
    if (16210..=16219).contains(&vnum) { return 50625; }
    let offset = i64::from(vnum) - match subtype {3 => 14000, 4 => 16000, 5 => 17000, _ => return 0};
    let mut kind = offset.div_euclid(20);
    if !(0..16).contains(&kind) { kind = (offset - 170).div_euclid(20); }
    if (0..16).contains(&kind) { 50623 + kind as u32 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_formats::proto::ItemData;
    fn stack(vnum: u32) -> ItemStack { ItemStack { vnum, count: 1, flags: 0, anti_flags: 0, sockets: [0;3], attributes: Default::default() } }
    fn fixture() -> ItemCatalog {
        let mut catalog = ItemCatalog::default();
        for (vnum, kind, subtype) in [(1,1,0), (2,3,18), (3,3,19), (4,10,1), (5,3,14), (6,3,17), (7,3,2), (8,3,21), (14000,2,3), (50623,3,21), (50624,3,21)] {
            let mut data = ItemData::default(); data.table.vnum = vnum; data.table.item_type = kind; data.table.sub_type = subtype; data.table.wear_flags = 1; data.table.flags = 1; data.table.refined_vnum = 10;
            catalog.0.items.insert(vnum, data);
        } catalog
    }
    #[test]
    fn bonus_changer_needs_existing_bonuses_and_adder_stops_at_five() {
        let catalog = fixture(); let mut target = stack(1);
        assert!(applies_to_item(&stack(2), &catalog));
        assert!(can_apply(&stack(2), &target, &catalog).is_err());
        assert!(can_apply(&stack(3), &target, &catalog).is_ok());
        target.attributes[0].bType = 1;
        assert!(can_apply(&stack(2), &target, &catalog).is_ok());
        for attr in target.attributes.iter_mut().take(5) { attr.bType = 1; }
        assert!(can_apply(&stack(3), &target, &catalog).is_err());
    }
    #[test]
    fn gold_metin_needs_gold_socket_and_detachment_accepts_real_stones() {
        let catalog = fixture(); let mut target = stack(1); target.sockets[0] = 1;
        assert!(can_apply(&stack(4), &target, &catalog).is_err());
        target.sockets[0] = 2; assert!(can_apply(&stack(4), &target, &catalog).is_ok());
        target.sockets[0] = 28030; assert!(can_apply(&stack(5), &target, &catalog).is_ok());
        assert!(can_apply(&stack(6), &target, &catalog).is_err());
        target.sockets[0] = 28960; assert!(can_apply(&stack(6), &target, &catalog).is_ok());
    }
    #[test]
    fn accessory_material_must_match_and_refining_needs_next_grade() {
        let mut catalog = fixture(); let mut target = stack(14000); target.sockets[1] = 1;
        assert!(can_apply(&stack(50623), &target, &catalog).is_ok());
        assert!(can_apply(&stack(50624), &target, &catalog).is_err());
        target.sockets[0] = 1; assert!(can_apply(&stack(50623), &target, &catalog).is_err());
        catalog.0.items.get_mut(&1).unwrap().table.refined_vnum = 0;
        assert!(can_apply(&stack(7), &stack(1), &catalog).is_err());
    }
}
