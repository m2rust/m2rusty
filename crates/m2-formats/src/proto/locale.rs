//! `LoadLocaleData` (`client/src/ui/PythonApplication.cpp`).

use super::{
    Error, ItemTables, MobTables, SkillTables, TextConvertTable, parse_insult_list, parse_item_proto, parse_mob_proto,
};
use crate::pack::PackManager;

/// One loader `LoadLocaleData` runs, with the file it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    ItemList,
    ItemProto,
    ItemDesc,
    MobProto,
    SkillDesc,
    SkillTable,
    InsultList,
    /// `lang<empire>.cvt`, empires 1..=3.
    ConvertTable(u32),
}

impl Step {
    /// The file under the locale path.
    pub fn file_name(self) -> String {
        match self {
            Step::ItemList => "item_list.txt".into(),
            Step::ItemProto => "item_proto".into(),
            Step::ItemDesc => "itemdesc.txt".into(),
            Step::MobProto => "mob_proto".into(),
            Step::SkillDesc => "SkillDesc.txt".into(),
            Step::SkillTable => "SkillTable.txt".into(),
            Step::InsultList => "insult.txt".into(),
            Step::ConvertTable(empire) => format!("lang{empire}.cvt"),
        }
    }

    /// Whether `LoadLocaleData` returns false when this step fails.
    pub fn is_required(self) -> bool {
        matches!(
            self,
            Step::ItemProto | Step::MobProto | Step::SkillDesc | Step::SkillTable
        )
    }
}

/// Everything `LoadLocaleData` loads into the managers.
#[derive(Debug, Default)]
pub struct LocaleData {
    pub items: ItemTables,
    pub mobs: MobTables,
    pub skills: SkillTables,
    pub insults: Vec<Vec<u8>>,
    /// Empires 1..=3; only loaded for the YMIR locale.
    pub convert_tables: [Option<TextConvertTable>; 3],
    /// The steps that ran, in order, with their result. A failed required
    /// step is the last one, as C++ returns there.
    pub steps: Vec<(Step, Result<(), Error>)>,
}

impl LocaleData {
    /// `LoadLocaleData`'s return value.
    pub fn ok(&self) -> bool {
        !self
            .steps
            .iter()
            .any(|(step, result)| step.is_required() && result.is_err())
    }
}

/// `LoadLocaleData(locale_path)` on fresh managers. `ymir` is
/// `LocaleService_IsYMIR()`: it selects the CHEONMA skill table rules and
/// loads the `lang%d.cvt` tables.
pub fn load_locale_data(packs: &PackManager, locale_path: &str, ymir: bool) -> LocaleData {
    load_locale_files(packs, |step| format!("{locale_path}/{}", step.file_name()), ymir)
}

/// [`load_locale_data`] with each step's virtual file name given by
/// `file_name` (e.g. the root pack's `936mob_proto`).
pub fn load_locale_files(packs: &PackManager, file_name: impl Fn(Step) -> String, ymir: bool) -> LocaleData {
    let mut data = LocaleData::default();
    let steps = [
        Step::ItemList,
        Step::ItemProto,
        Step::ItemDesc,
        Step::MobProto,
        Step::SkillDesc,
        Step::SkillTable,
        Step::InsultList,
    ];
    let convert = if ymir {
        (1..=3).map(Step::ConvertTable).collect()
    } else {
        Vec::new()
    };

    for step in steps.into_iter().chain(convert) {
        let result = packs
            .get(file_name(step))
            .map_err(Error::from)
            .and_then(|file| run(&mut data, step, &file, packs, ymir));
        let failed = result.is_err();
        data.steps.push((step, result));
        if failed && step.is_required() {
            break;
        }
    }
    data
}

fn run(data: &mut LocaleData, step: Step, file: &[u8], packs: &PackManager, ymir: bool) -> Result<(), Error> {
    match step {
        Step::ItemList => data.items.load_item_list(file),
        Step::ItemProto => data
            .items
            .load_item_table(&parse_item_proto(file)?, |name| packs.exists(name)),
        Step::ItemDesc => data.items.load_item_desc(file),
        Step::MobProto => data.mobs.insert_all(parse_mob_proto(file)?),
        Step::SkillDesc => data.skills.register_skill_desc(file)?,
        Step::SkillTable => data.skills.register_skill_table(file, ymir),
        Step::InsultList => data.insults = parse_insult_list(file),
        Step::ConvertTable(empire) => {
            let table = TextConvertTable::parse(file).ok_or(Error::Truncated)?;
            data.convert_tables[empire as usize - 1] = Some(table);
        }
    }
    Ok(())
}
