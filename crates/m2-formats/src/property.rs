//! Map object properties: `CProperty` (`client/src/game/Property.cpp`) and
//! `CPropertyManager` (`PropertyManager.cpp`).
//!
//! Every file under `property/` (`.prb` buildings, `.prt` trees, `.prd`
//! dungeon blocks, `.pre` effects, `.pra` ambiences) that starts with the
//! FourCC `YPRT` is a property:
//!
//! ```text
//! YPRT
//! 944313422
//! buildingfile    "d:/ymir work/.../house3.gr2"
//! propertyname    "house3"
//! propertytype    "Building"
//! ```
//!
//! The FourCC is followed by one line break, then a line with the decimal
//! CRC the maps refer to the property by, then `key values…` lines. The
//! manager keeps them by CRC; `property/reserve` lists CRCs (one per line)
//! that new properties must not take.

use std::collections::{BTreeMap, BTreeSet};

use crate::pack::PackManager;
use crate::script::{TokenVectorMap, atoi};
use crate::text::{self, DEFAULT_DELIMITERS};

/// One loaded property file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// `m_stFileName`: the name it was registered under, through
    /// `StringPath`.
    pub file_name: Vec<u8>,
    /// `m_stCRC`: the first line after the FourCC, as written.
    pub crc_string: Vec<u8>,
    /// `m_dwCRC`: `atoi` of [`crc_string`](Self::crc_string).
    pub crc: u32,
    /// `m_stTokenMap`: lowercased key to the rest of its line; the first
    /// line with a key wins.
    pub tokens: TokenVectorMap,
    /// Where `mc_pFileName` points into [`file_name`](Self::file_name).
    short_name_start: usize,
}

impl Property {
    /// `CProperty::CProperty` + `CProperty::ReadFromMemory`. `None` when
    /// `data` does not start with `YPRT` and a line break (`\r\n`, `\n` or
    /// `\r`).
    ///
    /// Deviation: C++ reads the FourCC and the byte after it without
    /// checking the size; here shorter data is not a property.
    pub fn read_from_memory(file_name: &[u8], data: &[u8]) -> Option<Self> {
        let body = data.strip_prefix(b"YPRT")?;
        let body = match body {
            [b'\r', b'\n', rest @ ..] | [b'\n' | b'\r', rest @ ..] => rest,
            _ => return None,
        };

        let lines = text::lines(body);
        let crc_string = lines[0].to_vec();
        let crc = atoi(&crc_string) as u32;

        let mut tokens = TokenVectorMap::new();
        for line in &lines[1..] {
            let Some(split) = text::split_line(line, DEFAULT_DELIMITERS) else {
                continue;
            };
            // `PutVector`: `std::map::insert` keeps the first.
            tokens
                .entry(split[0].to_ascii_lowercase())
                .or_insert_with(|| split[1..].iter().map(|t| t.to_vec()).collect());
        }

        // `mc_pFileName` is found before `StringPath`, so only `/` counts.
        let short_name_start = file_name.iter().rposition(|&b| b == b'/').map_or(0, |slash| slash + 1);
        Some(Self {
            file_name: crate::pack::normalize(file_name),
            crc_string,
            crc,
            tokens,
            short_name_start,
        })
    }

    /// `mc_pFileName`: the file name after its last `/`.
    pub fn short_name(&self) -> &[u8] {
        &self.file_name[self.short_name_start..]
    }

    /// `CProperty::GetVector`: the tokens of `key` (lowercased first).
    pub fn get_vector(&self, key: impl AsRef<[u8]>) -> Option<&[Vec<u8>]> {
        self.tokens.get(&key.as_ref().to_ascii_lowercase()).map(Vec::as_slice)
    }

    /// `CProperty::GetString`: the first token of `key`.
    ///
    /// Deviation: for a key with no values C++ reads `[0]` of an empty
    /// vector (undefined); here that is `None`.
    pub fn get_string(&self, key: impl AsRef<[u8]>) -> Option<&[u8]> {
        self.get_vector(key)?.first().map(Vec::as_slice)
    }
}

/// `CPropertyManager`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyManager {
    /// `m_PropertyByCRCMap`.
    pub by_crc: BTreeMap<u32, Property>,
    /// `m_ReservedCRCSet`.
    pub reserved_crcs: BTreeSet<u32>,
}

impl PropertyManager {
    /// `CPropertyManager::Initialize` over the registered files in name
    /// order. C++ walks its `std::unordered_map`, whose order decides which
    /// of two files with the same CRC is kept (the later one); use
    /// [`initialize_in_order`](Self::initialize_in_order) to replay a given
    /// order.
    pub fn initialize(&mut self, manager: &PackManager) {
        let mut names: Vec<&[u8]> = manager.file_names().collect();
        names.sort_unstable();
        self.initialize_in_order(manager, names);
    }

    /// `CPropertyManager::Initialize` over `names` in the given order:
    /// `property/reserve…` files are CRC lists, every other `property/…`
    /// file is registered, the rest is ignored.
    pub fn initialize_in_order<'n>(&mut self, manager: &PackManager, names: impl IntoIterator<Item = &'n [u8]>) {
        for name in names {
            if name.starts_with(b"property/reserve") {
                self.load_reserved_crc(manager, name);
            } else if name.starts_with(b"property/") {
                self.register(manager, name);
            }
        }
    }

    /// `CPropertyManager::LoadReservedCRC`: `atoi` of every non-empty line.
    /// `false` for a missing file.
    pub fn load_reserved_crc(&mut self, manager: &PackManager, name: impl AsRef<[u8]>) -> bool {
        let Ok(data) = manager.get(name) else {
            return false;
        };
        for line in text::lines(&data) {
            if !line.is_empty() && line[0] != 0 {
                self.reserve_crc(atoi(line) as u32);
            }
        }
        true
    }

    /// `CPropertyManager::ReserveCRC`.
    pub fn reserve_crc(&mut self, crc: u32) {
        self.reserved_crcs.insert(crc);
    }

    /// `CPropertyManager::Register` (and `Get(const char *)`): loads `name`
    /// and stores it by CRC, replacing a property with the same CRC.
    pub fn register(&mut self, manager: &PackManager, name: impl AsRef<[u8]>) -> Option<&Property> {
        let name = name.as_ref();
        let property = Property::read_from_memory(name, &manager.get(name).ok()?)?;
        let crc = property.crc;
        self.by_crc.insert(crc, property);
        self.by_crc.get(&crc)
    }

    /// `CPropertyManager::Get(DWORD)`.
    pub fn get(&self, crc: u32) -> Option<&Property> {
        self.by_crc.get(&crc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_properties() {
        let p = Property::read_from_memory(
            b"Property\\Tree/A.PRT",
            b"YPRT\r\n12345\r\nTreeFile \"x.spt\"\r\nTreeSize 1 2\r\ntreefile other\r\n\r\nempty\r\n",
        )
        .unwrap();
        assert_eq!(p.file_name, b"property/tree/a.prt");
        assert_eq!(p.short_name(), b"a.prt");
        assert_eq!((p.crc, &p.crc_string[..]), (12345, &b"12345"[..]));
        assert_eq!(p.get_string("TREEFILE"), Some(&b"x.spt"[..]));
        assert_eq!(p.get_vector("treesize").unwrap().len(), 2);
        assert_eq!(p.get_vector("empty"), Some(&[][..]));
        assert_eq!(p.get_string("empty"), None);

        let short = Property::read_from_memory(b"Property\\a.prt", b"YPRT\n-1").unwrap();
        assert_eq!(short.crc, u32::MAX);
        assert_eq!(short.short_name(), b"property/a.prt", "no '/' before StringPath");
        assert!(short.tokens.is_empty());
        assert!(Property::read_from_memory(b"a", b"YPRT\r").is_some());
        assert!(Property::read_from_memory(b"a", b"YPRTx\n1").is_none());
        assert!(Property::read_from_memory(b"a", b"DDS \n").is_none());
        assert!(Property::read_from_memory(b"a", b"YPRT").is_none());
    }
}
