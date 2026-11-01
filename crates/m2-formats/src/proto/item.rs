//! `CItemManager::LoadItemList`, `LoadItemTable` and `LoadItemDesc`
//! (`client/src/game/ItemManager.cpp`), with the parts of `CItemData` they
//! set (`client/src/game/ItemData.cpp`).

use std::collections::BTreeMap;

use super::{Error, Reader, atoi, cstr, le32, split_line};
use crate::crypt::{self, ITEM_PROTO_KEY};
use crate::text;

/// `sizeof(CItemData::TItemTable)`, packed.
pub const ITEM_TABLE_SIZE: usize = 156;

/// The icon `LoadItemTable` falls back to when none is found.
pub const EMPTY_BOWL_VNUM: u32 = 27995;

const ITEM_NAME_LEN: usize = 24 + 1;

/// `CItemData::TItemLimit`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemLimit {
    pub limit_type: u8,
    pub value: i32,
}

/// `CItemData::TItemApply`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemApply {
    pub apply_type: u8,
    pub value: i32,
}

/// `CItemData::TItemTable` (`client/include/Client/Game/ItemData.h`), one
/// item_proto record. Names are the raw `char[25]` buffers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemTable {
    pub vnum: u32,
    pub vnum_range: u32,
    pub name: [u8; ITEM_NAME_LEN],
    pub locale_name: [u8; ITEM_NAME_LEN],
    pub item_type: u8,
    pub sub_type: u8,
    pub weight: u8,
    pub size: u8,
    pub anti_flags: u32,
    pub flags: u32,
    pub wear_flags: u32,
    pub immune_flag: u32,
    pub ibuy_item_price: u32,
    pub isell_item_price: u32,
    pub limits: [ItemLimit; 2],
    pub applies: [ItemApply; 3],
    pub values: [i32; 6],
    pub sockets: [i32; 3],
    pub refined_vnum: u32,
    pub refine_set: u16,
    pub alter_to_magic_item_pct: u8,
    pub specular: u8,
    pub gain_socket_pct: u8,
}

impl ItemTable {
    /// Decodes one packed record.
    ///
    /// # Panics
    /// If `record` is shorter than [`ITEM_TABLE_SIZE`].
    pub fn parse(record: &[u8]) -> Self {
        assert!(record.len() >= ITEM_TABLE_SIZE);
        let mut r = Reader::new(record);
        Self {
            vnum: r.u32(),
            vnum_range: r.u32(),
            name: r.bytes(),
            locale_name: r.bytes(),
            item_type: r.u8(),
            sub_type: r.u8(),
            weight: r.u8(),
            size: r.u8(),
            anti_flags: r.u32(),
            flags: r.u32(),
            wear_flags: r.u32(),
            immune_flag: r.u32(),
            ibuy_item_price: r.u32(),
            isell_item_price: r.u32(),
            limits: std::array::from_fn(|_| ItemLimit {
                limit_type: r.u8(),
                value: r.i32(),
            }),
            applies: std::array::from_fn(|_| ItemApply {
                apply_type: r.u8(),
                value: r.i32(),
            }),
            values: std::array::from_fn(|_| r.i32()),
            sockets: std::array::from_fn(|_| r.i32()),
            refined_vnum: r.u32(),
            refine_set: r.u16(),
            alter_to_magic_item_pct: r.u8(),
            specular: r.u8(),
            gain_socket_pct: r.u8(),
        }
    }
}

/// `GetHashCode` (djb2) over the record's `szName`.
///
/// C++ hashes up to the first NUL with `strlen`, which runs on into
/// `szLocaleName` when all 25 name bytes are set; that is mirrored. If the
/// locale name has no NUL either, C++ goes on into the numeric fields; here
/// the hash stops at the end of the locale name. Bytes are sign-extended
/// like a signed `char` (`unsigned long` arithmetic truncated to `DWORD`).
pub fn item_name_hash(table: &ItemTable) -> u32 {
    let joined = [&table.name[..], &table.locale_name[..]].concat();
    cstr(&joined).iter().fold(5381u32, |h, &b| {
        (h << 5).wrapping_add(h).wrapping_add(b as i8 as i32 as u32)
    })
}

/// Decodes an item_proto file (`LoadItemTable` up to the record loop).
///
/// Two layouts: `MIPX` (version, stride, elements, data size) and the older
/// `MIPT` (elements, data size); the data is a `CLZObject` encrypted with
/// [`ITEM_PROTO_KEY`]. C++ reads `elements` records whatever the decompressed
/// size; here a file claiming more records than it holds is rejected.
pub fn parse_item_proto(data: &[u8]) -> Result<Vec<ItemTable>, Error> {
    let fourcc = le32(data, 0).ok_or(Error::Truncated)?;
    let mut pos = 4;
    if fourcc == u32::from_le_bytes(*b"MIPX") {
        let version = le32(data, 4).ok_or(Error::Truncated)?;
        let stride = le32(data, 8).ok_or(Error::Truncated)?;
        pos = 12;
        if version != 1 {
            return Err(Error::BadVersion(version));
        }
        if stride as usize != ITEM_TABLE_SIZE {
            return Err(Error::BadStride(stride));
        }
    } else if fourcc != u32::from_le_bytes(*b"MIPT") {
        return Err(Error::BadFourCC(fourcc));
    }

    let elements = le32(data, pos).ok_or(Error::Truncated)? as usize;
    let data_size = le32(data, pos + 4).ok_or(Error::Truncated)? as usize;
    let blob = data.get(pos + 8..pos + 8 + data_size).ok_or(Error::Truncated)?;

    let records = crypt::lz_decompress(blob, Some(&ITEM_PROTO_KEY))?;
    if elements.checked_mul(ITEM_TABLE_SIZE).is_none_or(|n| n > records.len()) {
        return Err(Error::Truncated);
    }
    Ok(records
        .as_chunks::<ITEM_TABLE_SIZE>()
        .0
        .iter()
        .take(elements)
        .map(|r| ItemTable::parse(r))
        .collect())
}

/// What `CItemManager` keeps per item (`CItemData`, without the loaded
/// graphics). File names are as stored, not normalised.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemData {
    pub icon_file_name: Vec<u8>,
    pub model_file_name: Vec<u8>,
    pub sub_model_file_name: Vec<u8>,
    pub drop_model_file_name: Vec<u8>,
    pub description: Vec<u8>,
    pub summary: Vec<u8>,
    /// All zero until an item_proto record is set.
    pub table: ItemTable,
}

impl ItemData {
    /// `CItemData::SetDefaultItemData`. Takes C strings, so the names end at
    /// the first NUL.
    fn set_default(&mut self, icon: &[u8], model: Option<&[u8]>) {
        match model {
            Some(model) => {
                self.model_file_name = cstr(model).to_vec();
                self.drop_model_file_name = cstr(model).to_vec();
            }
            None => {
                self.model_file_name.clear();
                self.drop_model_file_name = b"d:/ymir work/item/etc/item_bag.gr2".to_vec();
            }
        }
        self.icon_file_name = cstr(icon).to_vec();
        self.sub_model_file_name.clear();
        self.description.clear();
        self.summary.clear();
        self.table.sockets = [0; 3];
    }
}

/// `CItemManager`'s tables: `m_ItemMap` and `m_vec_ItemRange`.
#[derive(Debug, Clone, Default)]
pub struct ItemTables {
    pub items: BTreeMap<u32, ItemData>,
    /// Vnums of items whose record has a `vnum_range`, in load order; an item
    /// appears once per such record (C++ keeps pointers).
    pub ranges: Vec<u32>,
}

impl ItemTables {
    /// `CItemManager::LoadItemList`: `vnum<TAB>type<TAB>icon[<TAB>model]`
    /// lines, split with `SplitLine` (quotes allowed); other lines are
    /// skipped. Ignores `USE_VIETNAM_CONVERT_WEAPON_VNUM`, which only the
    /// Vietnamese build sets.
    pub fn load_item_list(&mut self, data: &[u8]) {
        for line in text::lines(data) {
            let (tokens, ok) = split_line(line, b"\t");
            if !ok || !(tokens.len() == 3 || tokens.len() == 4) {
                continue;
            }
            let vnum = atoi(tokens[0]) as u32;
            self.items
                .entry(vnum)
                .or_default()
                .set_default(tokens[2], tokens.get(3).copied());
        }
    }

    /// `CItemManager::LoadItemTable` after decoding: adds or updates one item
    /// per record. A new item's icon is `icon/item/<vnum>.tga`, else that of
    /// the first earlier record with the same name hash, else that of the
    /// vnum rounded down to 10, else [`EMPTY_BOWL_VNUM`]'s; `exists` is
    /// `CResourceManager::IsFileExist` (the pack manager's `isExist`).
    pub fn load_item_table(&mut self, tables: &[ItemTable], exists: impl Fn(&[u8]) -> bool) {
        let icon = |vnum: u32| format!("icon/item/{:05}.tga", vnum as i32).into_bytes();
        let mut name_map: BTreeMap<u32, u32> = BTreeMap::new();

        for table in tables {
            let vnum = table.vnum;
            let hash = item_name_hash(table);
            let item = match self.items.entry(vnum) {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => {
                    let mut name = icon(vnum);
                    if !exists(&name) {
                        name = match name_map.get(&hash) {
                            Some(&same_name) => icon(same_name),
                            None => icon(vnum - vnum % 10),
                        };
                        if !exists(&name) {
                            name = icon(EMPTY_BOWL_VNUM);
                        }
                    }
                    let item = e.insert(ItemData::default());
                    item.set_default(&name, None);
                    item
                }
            };
            name_map.entry(hash).or_insert(vnum);
            item.table = table.clone();
            if table.vnum_range != 0 {
                self.ranges.push(vnum);
            }
        }
    }

    /// `CItemManager::LoadItemDesc`: `vnum<TAB>name<TAB>description<TAB>
    /// summary` lines, split by tab; missing columns are empty, unknown vnums
    /// skipped. A description or summary in double quotes loses them.
    pub fn load_item_desc(&mut self, data: &[u8]) {
        for line in text::lines(data) {
            let tokens = text::split_by_tab(line);
            if tokens.is_empty() {
                continue;
            }
            let column = |i: usize| tokens.get(i).copied().unwrap_or(b"");
            let vnum = atoi(column(0)) as u32;
            if let Some(item) = self.items.get_mut(&vnum) {
                item.description = snap_string(column(2)).to_vec();
                item.summary = snap_string(column(3)).to_vec();
            }
        }
    }
}

/// `__SnapString`: strips a leading `"` and, if there is one, a trailing `"`
/// from strings of two bytes or more.
fn snap_string(s: &[u8]) -> &[u8] {
    if s.len() < 2 || s[0] != b'"' {
        return s;
    }
    let right = if s[s.len() - 1] == b'"' { s.len() - 1 } else { s.len() };
    &s[1..right]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap() {
        assert_eq!(snap_string(b"\"abc\""), b"abc");
        assert_eq!(snap_string(b"\"abc"), b"abc");
        assert_eq!(snap_string(b"\"\""), b"");
        assert_eq!(snap_string(b"\""), b"\"");
        assert_eq!(snap_string(b"abc\""), b"abc\"");
    }

    #[test]
    fn name_hash() {
        let mut t = ItemTable::default();
        assert_eq!(item_name_hash(&t), 5381);
        t.name[..2].copy_from_slice(b"ab");
        assert_eq!(item_name_hash(&t), (5381u32 * 33 + 97) * 33 + 98);
        // Signed char: 0xFF adds -1.
        t.name[..2].copy_from_slice(b"\xff\0");
        assert_eq!(item_name_hash(&t), 5381u32 * 33 - 1);
        // An unterminated name runs into the locale name.
        t.name = [b'a'; ITEM_NAME_LEN];
        t.locale_name[0] = b'b';
        let expected = (0..ITEM_NAME_LEN).fold(5381u32, |h, _| h.wrapping_mul(33).wrapping_add(97));
        assert_eq!(item_name_hash(&t), expected.wrapping_mul(33).wrapping_add(98));
    }

    #[test]
    fn item_list_and_desc() {
        let mut items = ItemTables::default();
        items.load_item_list(b"10\tWEAPON\ticon/a.tga\tmodel.gr2\n20\tETC\ticon/b.tga\nbad line\n30\tx\n\n");
        assert_eq!(items.items.len(), 2);
        assert_eq!(items.items[&10].drop_model_file_name, b"model.gr2");
        assert_eq!(
            items.items[&20].drop_model_file_name,
            b"d:/ymir work/item/etc/item_bag.gr2"
        );
        items.load_item_desc(b"10\tname\t\"desc\"\tsum\n20\tname\n99\tx\ty\tz\n");
        assert_eq!(items.items[&10].description, b"desc");
        assert_eq!(items.items[&10].summary, b"sum");
        assert_eq!(items.items[&20].description, b"");
        assert!(!items.items.contains_key(&99));
    }

    fn table(vnum: u32, name: &[u8], range: u32) -> ItemTable {
        let mut t = ItemTable {
            vnum,
            vnum_range: range,
            ..Default::default()
        };
        t.name[..name.len()].copy_from_slice(name);
        t
    }

    #[test]
    fn item_table_icons() {
        let mut items = ItemTables::default();
        items.load_item_list(b"5\tx\tlisted.tga\n");
        let tables = [
            table(5, b"listed", 0),
            table(100, b"sword", 0),
            table(103, b"sword", 9),
            table(117, b"other", 0),
            table(125, b"none", 0),
            table(103, b"again", 1),
        ];
        let exists = |n: &[u8]| n == b"icon/item/00100.tga" || n == b"icon/item/00110.tga";
        items.load_item_table(&tables, exists);
        assert_eq!(items.items[&5].icon_file_name, b"listed.tga");
        assert_eq!(items.items[&5].table.vnum, 5);
        assert_eq!(items.items[&100].icon_file_name, b"icon/item/00100.tga");
        // Same name as 100.
        assert_eq!(items.items[&103].icon_file_name, b"icon/item/00100.tga");
        // Rounded down to 110.
        assert_eq!(items.items[&117].icon_file_name, b"icon/item/00110.tga");
        assert_eq!(items.items[&125].icon_file_name, b"icon/item/27995.tga");
        // The later record replaces the table; both ranges are kept.
        assert_eq!(items.items[&103].table.vnum_range, 1);
        assert_eq!(items.ranges, [103, 103]);
    }
}
