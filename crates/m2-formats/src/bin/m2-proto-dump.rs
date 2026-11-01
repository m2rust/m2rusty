//! Rust counterpart of `m2diff_proto` (`tools/differential/proto/ProtoDump.cpp`).
//!
//!   m2-proto-dump tables <client dir> <index file> <ymir 0|1> <cvt prefix>
//!                 <item_list> <item_proto> <itemdesc> <mob_proto>
//!                 <skilldesc> <skilltable> <insult>
//!   m2-proto-dump lz <file> [k0 k1 k2 k3]
//!
//! Prints the same line-based dump as the C++ tool: one `step` line per
//! loader `LoadLocaleData` ran, then every item, mob, skill, insult and
//! conversion table.

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use m2_formats::crypt;
use m2_formats::pack::{PackManager, SearchMode, load_index};
use m2_formats::proto::{LocaleData, Step, load_locale_files};

fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

/// Printable ASCII as is, everything else (and `\`) as `\xNN`.
fn esc(s: &[u8]) -> String {
    s.iter()
        .map(|&c| {
            if (0x20..0x7f).contains(&c) && c != b'\\' {
                char::from(c).to_string()
            } else {
                format!("\\x{c:02x}")
            }
        })
        .collect()
}

fn hex(s: &[u8]) -> String {
    s.iter().map(|b| format!("{b:02x}")).collect()
}

fn dump(out: &mut impl Write, data: &LocaleData, file_name: impl Fn(Step) -> String) -> io::Result<()> {
    for (step, result) in &data.steps {
        writeln!(
            out,
            "step {} {}",
            esc(file_name(*step).as_bytes()),
            if result.is_ok() { "ok" } else { "fail" }
        )?;
    }

    for (vnum, item) in &data.items.items {
        writeln!(
            out,
            "item {vnum} icon={} model={} sub={} drop={} desc={} summ={}",
            esc(&item.icon_file_name),
            esc(&item.model_file_name),
            esc(&item.sub_model_file_name),
            esc(&item.drop_model_file_name),
            esc(&item.description),
            esc(&item.summary)
        )?;
        let t = &item.table;
        write!(
            out,
            "itemtable {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            t.vnum,
            t.vnum_range,
            hex(&t.name),
            hex(&t.locale_name),
            t.item_type,
            t.sub_type,
            t.weight,
            t.size,
            t.anti_flags,
            t.flags,
            t.wear_flags,
            t.immune_flag,
            t.ibuy_item_price,
            t.isell_item_price
        )?;
        for l in &t.limits {
            write!(out, " {}:{}", l.limit_type, l.value)?;
        }
        for a in &t.applies {
            write!(out, " {}:{}", a.apply_type, a.value)?;
        }
        for v in t.values.iter().chain(&t.sockets) {
            write!(out, " {v}")?;
        }
        writeln!(
            out,
            " {} {} {} {} {}",
            t.refined_vnum, t.refine_set, t.alter_to_magic_item_pct, t.specular, t.gain_socket_pct
        )?;
    }
    for vnum in &data.items.ranges {
        writeln!(out, "range {vnum}")?;
    }

    for m in data.mobs.mobs.values() {
        write!(out, "mob {} {} {}", m.vnum, hex(&m.name), hex(&m.locale_name))?;
        for v in [
            u32::from(m.mob_type),
            u32::from(m.rank),
            u32::from(m.battle_type),
            u32::from(m.level),
            u32::from(m.size),
            m.gold_min,
            m.gold_max,
            m.exp,
            m.max_hp,
            u32::from(m.regen_cycle),
            u32::from(m.regen_percent),
            u32::from(m.def),
            m.ai_flag,
            m.race_flag,
            m.immune_flag,
            u32::from(m.str),
            u32::from(m.dex),
            u32::from(m.con),
            u32::from(m.int),
            m.damage_range[0],
            m.damage_range[1],
        ] {
            write!(out, " {v}")?;
        }
        write!(
            out,
            " {} {} {} {} {}",
            m.attack_speed, m.moving_speed, m.aggressive_hp_pct, m.aggressive_sight, m.attack_range
        )?;
        for v in m.enchants.iter().chain(&m.resists) {
            write!(out, " {v}")?;
        }
        write!(
            out,
            " {} {} {} {} {} {} {:08x} {} {} {} {}",
            m.resurrection_vnum,
            m.drop_item_vnum,
            m.mount_capacity,
            m.on_click_type,
            m.empire,
            hex(&m.folder),
            m.dam_multiply.to_bits(),
            m.summon_vnum,
            m.drain_sp,
            m.monster_color,
            m.polymorph_item_vnum
        )?;
        for s in &m.skills {
            write!(out, " {}:{}", s.vnum, s.level)?;
        }
        writeln!(
            out,
            " {} {} {} {} {}",
            m.berserk_point, m.stone_skin_point, m.god_speed_point, m.death_blow_point, m.revive_point
        )?;
    }

    for (index, s) in &data.skills.skills {
        writeln!(
            out,
            "skill {index} type={} index={} max={} up={} limit={} nomotion={} motion={} attr={} weapon={} range={} \
             name={} desc={} cool={} tc={} lc={} sp={} cont={} dur={}",
            s.skill_type,
            s.skill_index,
            s.max_level,
            s.level_up_point,
            s.level_limit,
            u8::from(s.no_motion),
            s.motion_index,
            s.skill_attribute,
            s.need_weapon,
            s.target_range,
            esc(&s.name),
            esc(&s.description),
            esc(&s.cool_time_formula),
            esc(&s.target_count_formula),
            esc(&s.motion_loop_count_formula),
            esc(&s.need_sp_formula),
            esc(&s.continuation_sp_formula),
            esc(&s.duration)
        )?;
        for (g, grade) in s.grades.iter().enumerate() {
            writeln!(
                out,
                "skillgrade {index} {g} {} name={}",
                grade.motion_index,
                esc(&grade.name)
            )?;
        }
        for c in &s.conditions {
            writeln!(out, "skillcond {index} c={}", esc(c))?;
        }
        for a in &s.affects {
            writeln!(
                out,
                "skillaffect {index} desc={} min={} max={}",
                esc(&a.description),
                esc(&a.min_formula),
                esc(&a.max_formula)
            )?;
        }
    }

    for insult in &data.insults {
        // As C strings, like the C++ dump (see LoadInsultList there).
        writeln!(out, "insult {}", esc(m2_formats::proto::cstr(insult)))?;
    }
    for (i, table) in data.convert_tables.iter().enumerate() {
        if let Some(t) = table {
            writeln!(
                out,
                "cvt {} {} {} {:016x}",
                i + 1,
                hex(&t.upper),
                hex(&t.lower),
                fnv1a64(&t.han.concat())
            )?;
        }
    }
    Ok(())
}

fn tables(args: &[String]) -> ExitCode {
    let [client, index, ymir, cvt_prefix, files @ ..] = args else {
        eprintln!("tables: expected 11 arguments");
        return ExitCode::from(2);
    };
    let [
        item_list,
        item_proto,
        item_desc,
        mob_proto,
        skill_desc,
        skill_table,
        insult,
    ] = files
    else {
        eprintln!("tables: expected 11 arguments");
        return ExitCode::from(2);
    };

    let mut packs = PackManager::new(client);
    packs.set_search_mode(SearchMode::PackFirst);
    if let Err(e) = load_index(&mut packs, "pack", index) {
        eprintln!("{index}: {e}");
        return ExitCode::FAILURE;
    }

    let file_name = |step: Step| match step {
        Step::ItemList => item_list.clone(),
        Step::ItemProto => item_proto.clone(),
        Step::ItemDesc => item_desc.clone(),
        Step::MobProto => mob_proto.clone(),
        Step::SkillDesc => skill_desc.clone(),
        Step::SkillTable => skill_table.clone(),
        Step::InsultList => insult.clone(),
        Step::ConvertTable(_) => format!("{cvt_prefix}/{}", step.file_name()),
    };
    let data = load_locale_files(&packs, file_name, ymir == "1");

    let mut out = BufWriter::new(io::stdout().lock());
    if dump(&mut out, &data, file_name).and_then(|()| out.flush()).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn lz(args: &[String]) -> ExitCode {
    let Some(data) = args.first().and_then(|path| std::fs::read(path).ok()) else {
        return ExitCode::from(2);
    };
    let key: Vec<u32> = args[1..].iter().filter_map(|k| k.parse().ok()).collect();
    let key: Option<[u32; 4]> = key.try_into().ok();
    match crypt::lz_decompress(&data, key.as_ref()) {
        Ok(out) => println!("lz ok {} {:016x}", out.len(), fnv1a64(&out)),
        Err(_) => println!("lz fail"),
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("tables") => tables(&args[2..]),
        Some("lz") => lz(&args[2..]),
        _ => {
            eprintln!("usage: {} tables|lz ...", args[0]);
            ExitCode::from(2)
        }
    }
}
