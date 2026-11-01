//! `CRaceData::LoadRaceData` (`client/src/game/RaceDataFile.cpp`) and the
//! `CRaceData` state it fills (`RaceData.cpp`).

use std::collections::BTreeMap;

use super::data::{AttachingData, load_attaching_data};
use super::{Cursor, c_str, case_crc32, change_dos_path, string_path};
use crate::pack::PackManager;
use crate::script::{self, Script, atoi};

/// `CRaceData::SMOKE_NUM`.
pub const SMOKE_NUM: usize = 4;

/// `CRaceData::SSkin`: a texture replaced on the model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Skin {
    /// `m_ePart` (always 0 from scripts).
    pub part: i32,
    /// `m_stSrcFileName`, with `/` turned into `\` (`ChangeDosPath`).
    pub src_file_name: Vec<u8>,
    /// `m_stDstFileName`, as written.
    pub dst_file_name: Vec<u8>,
}

/// `CRaceData::SShape`: an armor/costume model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shape {
    /// `m_stModelFileName`.
    pub model_file_name: Vec<u8>,
    /// `m_kVct_kSkin`.
    pub skins: Vec<Skin>,
}

/// `CRaceData::SHair`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hair {
    /// `m_stModelFileName`.
    pub model_file_name: Vec<u8>,
    /// `m_kVct_kSkin`.
    pub skins: Vec<Skin>,
}

/// What `CRaceData` holds after `LoadRaceData`.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceData {
    /// `m_strBaseModelFileName`.
    pub base_model_file_name: Vec<u8>,
    /// `m_strTreeFileName`, through `StringPath` when not empty.
    pub tree_file_name: Vec<u8>,
    /// `m_strAttributeFileName`.
    pub attribute_file_name: Vec<u8>,
    /// `m_strSmokeBoneName`.
    pub smoke_bone_name: Vec<u8>,
    /// `m_strMotionListFileName`; `motlist.txt` unless the script names one.
    pub motion_list_file_name: Vec<u8>,
    /// `m_adwSmokeEffectID`: per smoke slot, the `GetCaseCRC32` of the
    /// `StringPath`ed effect file name (0 when unset or not registered).
    pub smoke_effect_ids: [u32; SMOKE_NUM],
    /// `m_kMap_dwShapeKey_kShape`, by shape index.
    pub shapes: BTreeMap<u32, Shape>,
    /// `m_kMap_dwHairKey_kHair`, by hair index.
    pub hairs: BTreeMap<u32, Hair>,
    /// `m_AttachingDataVector`.
    pub attaching_data: Vec<AttachingData>,
}

impl Default for RaceData {
    /// `CRaceData::__Initialize`.
    fn default() -> Self {
        Self {
            base_model_file_name: Vec::new(),
            tree_file_name: Vec::new(),
            attribute_file_name: Vec::new(),
            smoke_bone_name: Vec::new(),
            motion_list_file_name: b"motlist.txt".to_vec(),
            smoke_effect_ids: [0; SMOKE_NUM],
            shapes: BTreeMap::new(),
            hairs: BTreeMap::new(),
            attaching_data: Vec::new(),
        }
    }
}

impl RaceData {
    /// `CRaceData::SetShapeModel`.
    pub fn set_shape_model(&mut self, shape: u32, model_file_name: &[u8]) {
        self.shapes.entry(shape).or_default().model_file_name = c_str(model_file_name).to_vec();
    }

    /// `CRaceData::AppendShapeSkin`.
    pub fn append_shape_skin(&mut self, shape: u32, part: i32, src: &[u8], dst: &[u8]) {
        self.shapes.entry(shape).or_default().skins.push(skin(part, src, dst));
    }

    /// `CRaceData::SetHairSkin`: appends the skin and sets the model.
    pub fn set_hair_skin(&mut self, hair: u32, part: i32, model_file_name: &[u8], src: &[u8], dst: &[u8]) {
        let entry = self.hairs.entry(hair).or_default();
        entry.skins.push(skin(part, src, dst));
        entry.model_file_name = c_str(model_file_name).to_vec();
    }

    /// `CRaceData::LoadRaceData`: reads `name` and loads it with
    /// [`load_script`](Self::load_script). A missing file is `false`.
    pub fn load(&mut self, manager: &PackManager, name: impl AsRef<[u8]>, register_effect: impl FnMut(&[u8]) -> bool) -> bool {
        match script::load(manager, name) {
            Ok(script) => self.load_script(&script, register_effect),
            Err(_) => false,
        }
    }

    /// `CRaceData::LoadRaceData` on a parsed script, statement for
    /// statement. Like C++ it adds to what `self` already holds, and what it
    /// loaded before returning `false` stays (`CRaceManager` retries other
    /// paths on the same object; Python ignores the result).
    ///
    /// `register_effect` stands for `CEffectManager::RegisterEffect2` on each
    /// smoke effect file name: whether the effect script loads. When it does
    /// not, loading stops with `false`.
    ///
    /// Deviation: in the C++ as built, `ShapeDataCount` and `HairDataCount`
    /// are read into `DWORD const` locals initialised to 0 (a clang-tidy
    /// change that also made `GetTokenDoubleWord` take `const DWORD *`); the
    /// compiler folds the constant, so both loops never run and no shape or
    /// hair is ever loaded. This port loads them as the code intends; the
    /// differential test checks it against the C++ with `const` removed.
    pub fn load_script(&mut self, script: &Script, mut register_effect: impl FnMut(&[u8]) -> bool) -> bool {
        // `Load` fails when the braces do not balance.
        if !script.complete {
            return false;
        }
        let mut cur = Cursor::new(&script.root);

        cur.string("basemodelfilename", &mut self.base_model_file_name);
        cur.string("treefilename", &mut self.tree_file_name);
        cur.string("attributefilename", &mut self.attribute_file_name);
        cur.string("smokebonename", &mut self.smoke_bone_name);
        cur.string("motionlistfilename", &mut self.motion_list_file_name);

        if !self.tree_file_name.is_empty() {
            self.tree_file_name = string_path(&self.tree_file_name);
        }

        if let Some(tokens) = cur.tokens("smokefilename") {
            if tokens.len() % 2 != 0 {
                return false;
            }
            for pair in tokens.as_chunks::<2>().0 {
                let smoke = atoi(&pair[0]);
                if !(0..SMOKE_NUM as i32).contains(&smoke) {
                    return false;
                }
                let file_name = c_str(&pair[1]);
                let id = &mut self.smoke_effect_ids[smoke as usize];
                *id = case_crc32(&string_path(file_name));
                if !register_effect(file_name) {
                    *id = 0;
                    return false;
                }
            }
        }

        if cur.set_child(b"shapedata") {
            let mut path = Vec::new();
            let mut count = 0u32;
            if cur.string("pathname", &mut path) && cur.dword("shapedatacount", &mut count) {
                for i in 0..count {
                    if !cur.set_child_indexed(b"shapedata", i) {
                        continue;
                    }
                    // "Temporary - for an event"; it changes the path for
                    // the following entries too.
                    cur.string("specialpath", &mut path);

                    // From here a `continue` leaves the cursor in this entry,
                    // so the following entries are looked up inside it.
                    let mut index = 0u32;
                    if !cur.dword("shapeindex", &mut index) {
                        continue;
                    }

                    let mut model = Vec::new();
                    if cur.string("model", &mut model) {
                        self.set_shape_model(index, &[&path[..], &model].concat());
                    } else {
                        if !cur.string("local_model", &mut model) {
                            continue;
                        }
                        self.set_shape_model(index, &model);
                    }

                    let mut src = Vec::new();
                    let mut dst = Vec::new();
                    if cur.string("local_sourceskin", &mut src) && cur.string("local_targetskin", &mut dst) {
                        self.append_shape_skin(index, 0, &src, &dst);
                    }
                    for (src_key, dst_key) in [("sourceskin", "targetskin"), ("sourceskin2", "targetskin2")] {
                        if cur.string(src_key, &mut src) && cur.string(dst_key, &mut dst) {
                            self.append_shape_skin(index, 0, &[&path[..], &src].concat(), &[&path[..], &dst].concat());
                        }
                    }

                    cur.set_parent();
                }
            }
            cur.set_parent();
        }

        if cur.set_child(b"hairdata") {
            let mut path = Vec::new();
            let mut count = 0u32;
            if cur.string("pathname", &mut path) && cur.dword("hairdatacount", &mut count) {
                for i in 0..count {
                    if !cur.set_child_indexed(b"hairdata", i) {
                        continue;
                    }
                    cur.string("specialpath", &mut path);

                    let mut index = 0u32;
                    if !cur.dword("hairindex", &mut index) {
                        continue;
                    }

                    let (mut model, mut src, mut dst) = (Vec::new(), Vec::new(), Vec::new());
                    if cur.string("model", &mut model) && cur.string("sourceskin", &mut src) && cur.string("targetskin", &mut dst) {
                        let with_path = |s: &[u8]| [&path[..], s].concat();
                        self.set_hair_skin(index, 0, &with_path(&model), &with_path(&src), &with_path(&dst));
                    }

                    cur.set_parent();
                }
            }
            cur.set_parent();
        }

        if cur.set_child(b"attachingdata") {
            if !load_attaching_data(&mut cur, &mut self.attaching_data) {
                return false;
            }
            cur.set_parent();
        }

        true
    }
}

fn skin(part: i32, src: &[u8], dst: &[u8]) -> Skin {
    Skin {
        part,
        src_file_name: change_dos_path(c_str(src)),
        dst_file_name: c_str(dst).to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::parse;

    fn load(text: &[u8]) -> (RaceData, bool) {
        let mut race = RaceData::default();
        let ok = race.load_script(&parse(text), |_| true);
        (race, ok)
    }

    #[test]
    fn shapes_and_hair_load() {
        // C++ as built never loads these (see `load_script`).
        let (race, ok) = load(
            b"BaseModelFileName \"a.gr2\"\nTreeFileName \"D:\\T\\X.SPT\"\nGroup ShapeData\n{\nPathName \"d:/p/\"\nShapeDataCount 2\n\
              Group ShapeData00\n{\nShapeIndex 3\nModel \"m.gr2\"\nSourceSkin \"s/a.dds\"\nTargetSkin \"t.dds\"\n}\n\
              Group ShapeData01\n{\nSpecialPath \"e/\"\nShapeIndex 4\nLocal_Model \"l.gr2\"\n}\n}\n\
              Group HairData\n{\nPathName \"h/\"\nHairDataCount 1\nGroup HairData00\n{\nHairIndex 1\nModel \"m\"\nSourceSkin \"s\"\nTargetSkin \"t\"\n}\n}\n",
        );
        assert!(ok);
        assert_eq!(race.tree_file_name, b"d:/t/x.spt");
        assert_eq!(race.motion_list_file_name, b"motlist.txt");
        assert_eq!(race.shapes[&3].model_file_name, b"d:/p/m.gr2");
        assert_eq!(race.shapes[&3].skins[0].src_file_name, b"d:\\p\\s\\a.dds");
        assert_eq!(race.shapes[&3].skins[0].dst_file_name, b"d:/p/t.dds");
        assert_eq!(race.shapes[&4].model_file_name, b"l.gr2");
        assert_eq!(race.hairs[&1].model_file_name, b"h/m");
    }

    #[test]
    fn bad_shape_entry_strands_the_cursor() {
        // ShapeData00 has no ShapeIndex: the cursor stays in it, ShapeData01
        // is not found inside it, and the final SetParentNode only gets
        // back to ShapeData, so AttachingData is not found either.
        let (race, ok) = load(
            b"Group ShapeData\n{\nPathName \"\"\nShapeDataCount 2\nGroup ShapeData00\n{\nModel \"a\"\n}\n\
              Group ShapeData01\n{\nShapeIndex 1\nModel \"b\"\n}\n}\nGroup AttachingData\n{\nAttachingDataCount 0\n}\n",
        );
        assert!(ok);
        assert!(race.shapes.is_empty());
    }

    #[test]
    fn smoke_effects() {
        let text = b"List SmokeFileName\n{\n1 \"D:\\E\\A.mse\"\n0 b.mse\n}\nBaseModelFileName x\n";
        let mut race = RaceData::default();
        let mut seen = Vec::new();
        assert!(race.load_script(&parse(text), |n| {
            seen.push(n.to_vec());
            true
        }));
        assert_eq!(seen, [b"D:\\E\\A.mse".to_vec(), b"b.mse".to_vec()]);
        assert_eq!(race.smoke_effect_ids, [crc32fast::hash(b"b.mse"), crc32fast::hash(b"d:/e/a.mse"), 0, 0]);

        let mut race = RaceData::default();
        assert!(!race.load_script(&parse(text), |n| n != b"b.mse"));
        assert_eq!(race.smoke_effect_ids[0], 0);
        assert_ne!(race.smoke_effect_ids[1], 0);

        let (race, ok) = load(b"List SmokeFileName\n{\n4 a\n}\n");
        assert!(!ok);
        assert_eq!(race.smoke_effect_ids, [0; 4]);
        assert!(!load(b"List SmokeFileName\n{\n0\n}\n").1);
        assert!(!load(b"Group A\n{\n").1, "unbalanced braces fail Load");
    }
}
