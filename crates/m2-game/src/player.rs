//! Server-owned player data and UI requests. Widgets never predict item moves.
use std::collections::{BTreeMap, HashSet, VecDeque};
use bevy::{prelude::*, tasks::{IoTaskPool, Task, futures::check_ready}};
use m2_assets::{M2Packs, paths};
use m2_formats::proto::{ItemData, ItemTables, parse_item_proto};
use m2_net::{phase::{NetEvent, Phase}, wire::{self, GcPacket, TItemPos, TPlayerItemAttribute, TQuickSlot, Wire, cg}};
use crate::combat::{self, ActorCombat, CombatWorld};
use crate::{ingame::LOCALE_PATH, net::{Net, NetMessage}};

/// `c_Equipment_Start` (`client/include/Client/UI/GameType.h`): equipment cells
/// are 90.., the first 90 cells are the inventory (`c_ItemSlot_Count`).
pub const EQUIPMENT_START: u16 = 90;
/// `c_Equipment_Body` (`c_Equipment_Start + 0`).
pub const EQUIPMENT_BODY: u16 = EQUIPMENT_START;
/// `c_Equipment_Weapon` (`c_Equipment_Start + 4`).
pub const EQUIPMENT_WEAPON: u16 = EQUIPMENT_START + 4;

/// Inventory, equipment and belt share the same underlying array in the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ItemPosition { pub window: u8, pub cell: u16 }
impl ItemPosition {
    pub fn inventory(cell: u16) -> Self { Self { window: 1, cell } }
    /// An equipment cell of the shared item array (`TItemPos(EQUIPMENT, cell)`).
    pub fn equipment(cell: u16) -> Self { Self { window: 1, cell } }
    pub fn from_wire(pos: TItemPos) -> Self { Self { window: if pos.window_type == 2 {1} else {pos.window_type}, cell: pos.cell } }
    pub fn wire(self) -> TItemPos { TItemPos { window_type: self.window, cell: self.cell } }
    pub fn valid(self) -> bool { match self.window { 1 => self.cell < 168, 5 => self.cell < 960, _ => false } }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ItemStack {
    pub vnum: u32, pub count: u8, pub flags: u32, pub anti_flags: u32,
    pub sockets: [i32; 3], pub attributes: [TPlayerItemAttribute; 7],
}
#[derive(Clone, Debug)]
pub struct ChatLine { pub kind: u8, pub text: String }
/// `GcPacket::PartyLink`/`Unlink` key: the player id behind a party member.
#[derive(Clone, Debug, Default)]
pub struct PartyMember {
    pub pid: u32,
    pub vid: u32,
    pub name: String,
    pub state: u8,
    pub percent_hp: u8,
    pub affects: [i16; 7],
}

/// `PVP_MODE_*` (`Packet.h`).
pub const PVP_MODE_NONE: u8 = 0;
pub const PVP_MODE_AGREE: u8 = 1;
pub const PVP_MODE_FIGHT: u8 = 2;
pub const PVP_MODE_REVENGE: u8 = 3;

/// `CInstanceBase::g_kSet_dwPVPKey` and `CPythonPlayer`'s challenge,
/// revenge and cannot-fight instance sets.
#[derive(Clone, Debug, Default)]
pub struct Pvp {
    pub keys: HashSet<(u32, u32)>,
    /// `g_kSet_dwGVGKey`: guilds at war (`GUILD_SUBHEADER_GC_GUILD_WAR_LIST`).
    pub guild_keys: HashSet<(u32, u32)>,
    pub challenge: HashSet<u32>,
    pub revenge: HashSet<u32>,
    pub cant_fight: HashSet<u32>,
}

impl Pvp {
    /// `CPythonNetworkStream::RecvPVPPacket`; `me` is the main character's VID.
    pub fn apply(&mut self, me: u32, src: u32, dst: u32, mode: u8) {
        match mode {
            PVP_MODE_AGREE => {
                self.remove_key(src, dst);
                // The other side asked me (dst) to fight.
                if dst == me { self.revenge.remove(&src); self.challenge.insert(src); }
                // I cannot fight while my request is pending.
                if src == me { self.cant_fight.insert(dst); }
            }
            PVP_MODE_REVENGE => {
                self.remove_key(src, dst);
                // src killed me (dst): I may take revenge.
                if dst == me { self.challenge.remove(&src); self.revenge.insert(src); }
                if src == me { self.cant_fight.insert(dst); }
            }
            PVP_MODE_FIGHT => { self.keys.insert((src, dst)); self.forget(src); self.forget(dst); }
            PVP_MODE_NONE => { self.remove_key(src, dst); self.forget(src); self.forget(dst); }
            _ => {}
        }
    }
    fn remove_key(&mut self, a: u32, b: u32) { self.keys.remove(&(a, b)); self.keys.remove(&(b, a)); }
    /// `CPythonPlayer::ForgetInstance`.
    fn forget(&mut self, vid: u32) { self.challenge.remove(&vid); self.revenge.remove(&vid); self.cant_fight.remove(&vid); }
}

#[derive(Clone, Debug, Default)]
pub struct Quest {
    pub title: String,
    pub clock_name: String,
    pub clock_value: i32,
    pub counter_name: String,
    pub counter_value: i32,
    pub icon: String,
}

#[derive(Resource)]
pub struct PlayerState {
    pub vid: u32, pub name: String, pub race: u16, pub skill_group: u8, pub points: [i32; 255],
    pub items: BTreeMap<ItemPosition, ItemStack>, pub quickslots: [TQuickSlot; 32],
    pub chat: VecDeque<ChatLine>,
    /// The main actor's `CharacterAdditionalInfo` (`CInstanceBase` fields the
    /// combat predicates read).
    pub kind: u8, pub empire: u8, pub alignment: i16, pub pk_mode: u8, pub guild_id: u32,
    pub level: u32, pub state_flags: u8, pub affect_flags: u64,
    /// Armor vnum and hair part from the main actor's appearance packets.
    pub appearance: Option<(u16, u16)>,
    /// The monster race the main character is polymorphed into, 0 when not
    /// (its own `GC_CHARACTER_ADD` carries a race of `MAIN_RACE_MAX_NUM` and up).
    pub poly_race: u16,
    /// `TPacketGCMount.mount_vid`; 0 when not mounted.
    pub mount_vnum: u32,
    /// `CPythonPlayer::m_dwDuelMode` and the `InsertDUELKey` pairs.
    pub duel_mode: u8, pub duel_keys: HashSet<(u32, u32)>,
    /// `GC_PVP` state: the fight keys and the target board's duel buttons.
    pub pvp: Pvp,
    /// Party members by player id (`CPythonPlayer::m_PartyMemberMap`).
    pub party: BTreeMap<u32, PartyMember>,
    pub quests: BTreeMap<u16, Quest>,
    pub observer: bool,
}

/// Contents and open state of `CPythonSafeBox` (bank safe box and item mall).
#[derive(Resource, Default)]
pub struct StorageState {
    pub safebox: Vec<Option<ItemStack>>,
    pub mall: Vec<Option<ItemStack>>,
    pub money: u32,
    pub safebox_open: bool,
    pub mall_open: bool,
    pub password_error: bool,
    pub safebox_serial: u64,
    pub mall_serial: u64,
}

/// One server-provided NPC shop tab (`CPythonShop::ShopTab`).
#[derive(Clone, Debug)]
pub struct ShopTab {
    pub name: String,
    pub coin_type: u8,
    pub items: Vec<wire::TShopItemData>,
}

/// Server-owned contents of the current `CPythonShop` window.
#[derive(Resource, Default)]
pub struct ShopState {
    pub open: bool,
    pub generation: u64,
    pub owner_vid: u32,
    pub tabs: Vec<ShopTab>,
    pub selling_price: i32,
    pub message: Option<String>,
}

impl ShopState {
    fn clear(&mut self) {
        let generation = self.generation.wrapping_add(1);
        *self = Self { generation, ..Self::default() };
    }

    fn set_item(&mut self, index: usize, item: wire::TShopItemData) {
        let tab = index / wire::consts::SHOP_HOST_ITEM_MAX_NUM;
        let cell = index % wire::consts::SHOP_HOST_ITEM_MAX_NUM;
        if let Some(tab) = self.tabs.get_mut(tab)
            && let Some(slot) = tab.items.get_mut(cell)
        {
            *slot = item;
        }
    }

    fn apply_packet(&mut self, head: &wire::TPacketGCShop, tail: &[u8]) {
        const START: u8 = 0;
        const END: u8 = 1;
        const UPDATE_ITEM: u8 = 2;
        const UPDATE_PRICE: u8 = 3;
        const NOT_ENOUGH_MONEY: u8 = 5;
        const SOLD_OUT: u8 = 6;
        const INVENTORY_FULL: u8 = 7;
        const INVALID_POS: u8 = 8;
        const START_EX: u8 = 10;
        const NOT_ENOUGH_MONEY_EX: u8 = 11;

        match head.subheader {
            START if tail.len() >= 4 + wire::TPacketGCShopStart::SIZE => {
                self.clear();
                self.owner_vid = u32::from_le_bytes(tail[..4].try_into().unwrap_or_default());
                let start = wire::TPacketGCShopStart::decode(&tail[4..]).unwrap_or_default();
                self.tabs.push(ShopTab { name: "Shop".into(), coin_type: 0, items: start.items.to_vec() });
                self.open = true;
            }
            START_EX if tail.len() >= wire::TPacketGCShopStartEx::SIZE => {
                self.clear();
                let start = wire::TPacketGCShopStartEx::decode(tail).unwrap_or_default();
                self.owner_vid = start.owner_vid;
                let mut offset = wire::TPacketGCShopStartEx::SIZE;
                for _ in 0..usize::from(start.shop_tab_count.min(3)) {
                    let Some(bytes) = tail.get(offset..offset + wire::TSubPacketShopTab::SIZE) else { break };
                    let Ok(tab) = wire::TSubPacketShopTab::decode(bytes) else { break };
                    self.tabs.push(ShopTab { name: tab.name_str(), coin_type: tab.coin_type, items: tab.items.to_vec() });
                    offset += wire::TSubPacketShopTab::SIZE;
                }
                self.open = !self.tabs.is_empty();
            }
            END => self.clear(),
            UPDATE_ITEM if tail.len() >= wire::TPacketGCShopUpdateItem::SIZE => {
                if let Ok(update) = wire::TPacketGCShopUpdateItem::decode(tail) {
                    self.set_item(usize::from(update.pos), update.item);
                }
            }
            UPDATE_PRICE if tail.len() >= 4 => {
                self.selling_price = i32::from_le_bytes(tail[..4].try_into().unwrap_or_default());
            }
            NOT_ENOUGH_MONEY => self.message = Some("Not enough yang.".into()),
            NOT_ENOUGH_MONEY_EX => self.message = Some("Not enough currency.".into()),
            SOLD_OUT => self.message = Some("This item is sold out.".into()),
            INVENTORY_FULL => self.message = Some("Your inventory is full.".into()),
            INVALID_POS => self.message = Some("This shop item is unavailable.".into()),
            _ => {}
        }
    }
}

impl StorageState {
    fn apply(&mut self, event: &NetEvent) {
        match event {
            NetEvent::SafeboxSet(packet) => set_storage_item(&mut self.safebox, packet.Cell.cell, item_stack(packet)),
            NetEvent::MallSet(packet) => set_storage_item(&mut self.mall, packet.Cell.cell, item_stack(packet)),
            NetEvent::Packet(GcPacket::SafeboxSize(packet)) => {
                self.safebox = vec![None; usize::from(packet.bSize).saturating_mul(5)];
                self.safebox_open = true;
                self.safebox_serial = self.safebox_serial.wrapping_add(1);
                self.password_error = false;
            }
            NetEvent::Packet(GcPacket::SafeboxDel(packet)) => {
                if let Some(slot) = self.safebox.get_mut(usize::from(packet.pos)) { *slot = None; }
            }
            NetEvent::Packet(GcPacket::SafeboxMoneyChange(packet)) => self.money = packet.dwMoney,
            NetEvent::Packet(GcPacket::SafeboxWrongPassword(_)) => self.password_error = true,
            NetEvent::Packet(GcPacket::MallOpen(packet)) => {
                self.mall = vec![None; usize::from(packet.bSize).saturating_mul(5)];
                self.mall_open = true;
                self.mall_serial = self.mall_serial.wrapping_add(1);
            }
            NetEvent::Packet(GcPacket::MallDel(packet)) => {
                if let Some(slot) = self.mall.get_mut(usize::from(packet.pos)) { *slot = None; }
            }
            NetEvent::PhaseChanged { to: Phase::Login | Phase::Select | Phase::Loading, .. } => {
                self.safebox.clear();
                self.mall.clear();
                self.money = 0;
                self.safebox_open = false;
                self.mall_open = false;
                self.password_error = false;
            }
            _ => {}
        }
    }
}

impl ShopState {
    fn apply(&mut self, event: &NetEvent) {
        match event {
            NetEvent::Packet(GcPacket::Shop { head, tail }) => self.apply_packet(head, tail),
            NetEvent::PhaseChanged { to: Phase::Login | Phase::Select | Phase::Loading, .. } => self.clear(),
            _ => {}
        }
    }
}

fn item_stack(packet: &wire::TPacketGCItemSet2) -> ItemStack {
    ItemStack {
        vnum: packet.vnum,
        count: packet.count,
        flags: packet.flags,
        anti_flags: packet.anti_flags,
        sockets: packet.alSockets,
        attributes: packet.aAttr,
    }
}

fn set_storage_item(items: &mut [Option<ItemStack>], index: u16, item: ItemStack) {
    if let Some(slot) = items.get_mut(usize::from(index)) {
        *slot = (item.vnum != 0).then_some(item);
    }
}
impl Default for PlayerState {
    fn default() -> Self { Self { vid:0, name:String::new(), points:[0;255], items:BTreeMap::new(), quickslots:[TQuickSlot::default();32], chat:VecDeque::new(), race:0, skill_group:0, kind:0, empire:0, alignment:0, pk_mode:0, guild_id:0, level:0, state_flags:0, affect_flags:0, appearance:None, poly_race:0, mount_vnum:0, duel_mode:0, duel_keys:HashSet::new(), pvp:Pvp::default(), party:BTreeMap::new(), quests:BTreeMap::new(), observer:false } }
}
impl PlayerState {
    pub fn line(&mut self, kind:u8, text:String) {
        self.chat.push_back(ChatLine{kind,text});
        while self.chat.len()>500 { self.chat.pop_front(); }
    }
    pub fn apply(&mut self, event:&NetEvent) {
        match event {
            // `CPythonPlayer::SetMainCharacterIndex` only stores the index; it does
            // not clear the item array. Equipment packets may arrive before
            // `MainCharacter`, so keep them (`MainCharacter` is not a reset).
            NetEvent::MainCharacter(main) => {
                self.vid=main.vid; self.name=main.name.clone(); self.race=main.race; self.skill_group=main.skill_group;
                self.poly_race=0;
                self.kind=m2_net::phase::ACTOR_TYPE_PC;
                self.appearance=None;
            }
            NetEvent::Empire(empire) => self.empire = *empire,
            NetEvent::ActorAppend(actor) if actor.vid==self.vid => {
                self.kind=actor.kind;
                self.empire=actor.empire;
                self.alignment=actor.alignment;
                self.pk_mode=actor.pk_mode;
                self.guild_id=actor.guild_id;
                self.level=actor.level;
                self.state_flags=actor.state_flags;
                self.affect_flags=actor.affect_flags;
                self.appearance=Some((actor.armor,actor.hair));
                self.mount_vnum=actor.mount_vnum;
                self.poly_race=if actor.race>=8 { actor.race } else { 0 };
            }
            NetEvent::ActorUpdate(update) if update.vid==self.vid => {
                self.alignment=update.alignment;
                self.pk_mode=update.pk_mode;
                self.guild_id=update.guild_id;
                self.state_flags=update.state_flags;
                self.affect_flags=update.affect_flags;
                self.appearance=Some((update.armor,update.hair));
                self.mount_vnum=update.mount_vnum;
            }
            NetEvent::Chat{kind,text,..} if *kind != 5 => self.line(*kind,text.clone()),
            NetEvent::Packet(packet) => match packet {
                GcPacket::PlayerPoints(p) => self.points=p.points,
                GcPacket::PlayerPointChange(p) if p.dwVID==self.vid => {
                    if let Some(point)=self.points.get_mut(p.Type as usize) { *point=p.value; }
                }
                GcPacket::Mount(p) if p.vid==self.vid => self.mount_vnum=p.mount_vid,
                GcPacket::QuestInfo { head, tail } => {
                    if let Ok(info) = wire::tails::quest_info(head.flag, tail) {
                        if info.begin == Some(false) {
                            self.quests.remove(&head.index);
                        } else {
                            if info.begin == Some(true) { self.quests.insert(head.index, Quest::default()); }
                            let quest = self.quests.entry(head.index).or_default();
                            if let Some(value) = info.title.filter(|s| !s.is_empty()) { quest.title = value; }
                            if let Some(value) = info.clock_name.filter(|s| !s.is_empty()) { quest.clock_name = value; }
                            if let Some(value) = info.clock_value { quest.clock_value = value; }
                            if let Some(value) = info.counter_name.filter(|s| !s.is_empty()) { quest.counter_name = value; }
                            if let Some(value) = info.counter_value { quest.counter_value = value; }
                            if let Some(value) = info.icon.filter(|s| !s.is_empty()) { quest.icon = value; }
                        }
                    }
                }
                // `GC_AFFECT_ADD`/`_REMOVE` name the affect's type (a skill
                // or item buff id), not an `AFFECT_*` flag bit; the visible
                // flags come only with `GC_CHARACTER_ADD`/`_UPDATE`
                // (`SetAffectFlagContainer`).
                GcPacket::DuelStart { tail, .. } => {
                    self.duel_keys.clear();
                    let target=self.vid;
                    for chunk in tail.as_chunks::<4>().0 {
                        let vid=u32::from_le_bytes([chunk[0],chunk[1],chunk[2],chunk[3]]);
                        if vid!=0 { self.duel_keys.insert((target,vid)); }
                    }
                    self.duel_mode=if self.duel_keys.is_empty() { combat::DUEL_CANNOTATTACK } else { combat::DUEL_START };
                }
                // `RecvChangeSkillGroupPacket`: `m_dwMainActorSkillGroup`.
                GcPacket::ChangeSkillGroup(p) => self.skill_group=p.skill_group,
                GcPacket::Pvp(p) => self.pvp.apply(self.vid, p.dwVIDSrc, p.dwVIDDst, p.bMode),
                GcPacket::Guild { head, tail } if matches!(head.subheader, crate::social::guild::GUILD_WAR_LIST | crate::social::guild::GUILD_WAR_END_LIST) => {
                    for (a, b) in crate::social::guild_war_pairs(tail).unwrap_or_default() {
                        if head.subheader == crate::social::guild::GUILD_WAR_LIST { self.pvp.guild_keys.insert((a, b)); }
                        else { self.pvp.guild_keys.remove(&(a, b)); self.pvp.guild_keys.remove(&(b, a)); }
                    }
                }
                GcPacket::PartyAdd(p) => {
                    let member = self.party.entry(p.pid).or_default();
                    member.pid = p.pid;
                    member.name = p.name_str();
                }
                GcPacket::PartyUpdate(p) => {
                    if let Some(member) = self.party.get_mut(&p.pid) {
                        member.state = p.state;
                        member.percent_hp = p.percent_hp;
                        member.affects = p.affects;
                    }
                }
                GcPacket::PartyLink(p) => {
                    let member = self.party.entry(p.pid).or_default();
                    member.pid = p.pid;
                    member.vid = p.vid;
                }
                GcPacket::PartyUnlink(p) => {
                    if let Some(member) = self.party.get_mut(&p.pid) { member.vid = 0; }
                }
                GcPacket::PartyRemove(p) => {
                    self.party.retain(|_, member| member.pid!=p.pid);
                }
                GcPacket::ItemSet(p) => {
                    debug!("item set cell {:?} vnum {} count {}", p.Cell, p.vnum, p.count);
                    self.set_item(p.Cell, ItemStack{vnum:p.vnum,count:p.count,flags:0,anti_flags:0,sockets:p.alSockets,attributes:p.aAttr})
                }
                GcPacket::ItemSet2(p) => {
                    debug!("item set2 cell {:?} vnum {} count {} flags {}", p.Cell, p.vnum, p.count, p.flags);
                    self.set_item(p.Cell, ItemStack{vnum:p.vnum,count:p.count,flags:p.flags,anti_flags:p.anti_flags,sockets:p.alSockets,attributes:p.aAttr})
                }
                GcPacket::ItemUpdate(p) => {
                    debug!("item update cell {:?} count {}", p.Cell, p.count);
                    let pos=ItemPosition::from_wire(p.Cell);
                    if p.count==0 { self.items.remove(&pos); }
                    else if let Some(item)=self.items.get_mut(&pos) {item.count=p.count;item.sockets=p.alSockets;item.attributes=p.aAttr;}
                }
                GcPacket::QuickslotAdd(p) if (p.pos as usize)<32 => self.quickslots[p.pos as usize]=p.slot,
                GcPacket::QuickslotDel(p) if (p.pos as usize)<32 => self.quickslots[p.pos as usize]=TQuickSlot::default(),
                GcPacket::QuickslotSwap(p) if p.pos<32 && p.change_pos<32 => self.quickslots.swap(p.pos as usize,p.change_pos as usize),
                _=>{}
            },
            _=>{}
        }
    }

    /// The main actor as [`combat`] sees it.
    pub fn combat_self(&self) -> ActorCombat {
        ActorCombat {
            vid:self.vid, kind:self.kind, race:self.race, empire:self.empire, pk_mode:self.pk_mode,
            guild_id:self.guild_id, alignment:self.alignment, dead:self.state_flags&combat::STATE_DEAD!=0,
            state_flags:self.state_flags, affect_flags:self.affect_flags, in_safe:false, x:0, y:0,
        }
    }

    /// The party, duel and war state `IsAttackableInstance` reads.
    pub fn combat_world(&self) -> CombatWorld {
        CombatWorld {
            observer:self.observer,
            duel_mode:self.duel_mode,
            party:self.party.values().map(|member| member.vid).collect(),
            duel_keys:self.duel_keys.clone(),
            pvp_keys:self.pvp.keys.clone(),
            gvg_keys:self.pvp.guild_keys.clone(),
        }
    }

    /// The number of arrows in the arrow equipment cell (`c_Equipment_Arrow`,
    /// `c_Equipment_Start + 6`).
    pub fn arrow_count(&self) -> u32 {
        self.items.get(&ItemPosition::equipment(EQUIPMENT_START+9)).map_or(0, |item| u32::from(item.count))
    }

    /// `CPythonPlayer::__HasItem`.
    pub fn has_item(&self, vnum: u32) -> bool {
        self.items.values().any(|item| item.vnum==vnum)
    }
    fn set_item(&mut self,pos:TItemPos,item:ItemStack) {
        let pos=ItemPosition::from_wire(pos);
        if !pos.valid(){return;}
        if item.vnum==0 || item.count==0 { self.items.remove(&pos); } else { self.items.insert(pos,item); }
    }
    /// The weapon in equipment cell [`EQUIPMENT_WEAPON`], if any.
    pub fn weapon(&self) -> Option<&ItemStack> {
        self.items.get(&ItemPosition::equipment(EQUIPMENT_WEAPON))
    }
}

/// The render data of an equipped weapon, resolved from `item_proto`/`item_list`
/// (`CItemManager::GetItemDataPointer`) and the item's data script
/// (`CItemData::GetModelThing`/`GetSubModelThing`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponVisual {
    pub vnum: u32,
    /// `CItemData::GetModelThing` (the main hand model).
    pub model: String,
    /// `CItemData::GetSubModelThing`; the item's `SubModelFileName`, or the
    /// main model when it names none (the C++ getter's fallback).
    pub sub_model: String,
    /// `CItemData::GetWeaponType` (`TItemTable::bSubType`).
    pub weapon_type: u8,
    /// `TItemTable::bSpecular` (`GetSpecularPowerf` is this over 100).
    pub specular: u8,
}

#[derive(Resource,Default)]
pub struct ItemCatalog(pub ItemTables);
impl ItemCatalog {
    pub fn get(&self,vnum:u32)->Option<&ItemData> {
        self.0.items.get(&vnum).or_else(||self.0.ranges.iter().filter_map(|id|self.0.items.get(id)).find(|i|vnum>i.table.vnum && vnum<i.table.vnum.saturating_add(i.table.vnum_range)))
    }
    /// The weapon model of `vnum` (`CItemManager::GetItemDataPointer` plus
    /// `CItemData::GetModelThing`/`GetSubModelThing`). The sub-model comes from
    /// `ItemData::sub_model_file_name`, else the item data script
    /// (`d:/ymir work/item/<vnum>.msm`), else `<model>_1.gr2` when that file
    /// exists; `GetSubModelThing` itself falls back to the main model.
    pub fn weapon_visual(&self, packs:&m2_formats::pack::PackManager, vnum:u32)->Option<WeaponVisual> {
        let item=self.get(vnum)?;
        let model=paths::canonical_asset_path(&String::from_utf8_lossy(&item.model_file_name));
        if model.is_empty() { return None; }
        let sub_model=if item.sub_model_file_name.is_empty() {
            item_sub_model(packs,vnum,&model).unwrap_or_else(||model.clone())
        } else {
            paths::canonical_asset_path(&String::from_utf8_lossy(&item.sub_model_file_name))
        };
        Some(WeaponVisual{vnum,model,sub_model,weapon_type:item.table.sub_type,specular:item.table.specular})
    }
}

/// `CItemData::m_strSubModelFileName` is set from the item data script
/// (`ItemDataScript`'s `SubModelFileName`); the item list itself only names the
/// main model. The shipped data only has the per-vnum script for some items,
/// so the sibling `<model>_1.gr2` is used when the script is absent.
fn item_sub_model(packs:&m2_formats::pack::PackManager,vnum:u32,model:&str)->Option<String> {
    for candidate in [vnum,vnum-vnum%10] {
        let file=format!("d:/ymir work/item/{candidate:05}.msm");
        let Ok(data)=packs.get(paths::virtual_name(&file)) else { continue };
        let script=m2_formats::script::parse(&data);
        if let Some(name)=script.root.token_string(b"submodelfilename")
            && !name.is_empty()
        {
            return Some(paths::canonical_asset_path(&String::from_utf8_lossy(name)));
        }
    }
    let sibling=format!("{}_1.gr2",model.strip_suffix(".gr2")?);
    packs.exists(paths::virtual_name(&sibling)).then_some(sibling)
}
#[derive(Resource)] struct CatalogTask(Task<Result<ItemTables,String>>);

#[derive(Message,Clone,Debug)]
pub enum PlayerAction {
    Chat {text:String,kind:u8},
    /// Selects a server-provided NPC quest dialog answer (`254` continues).
    QuestAnswer(u8),
    /// Clicks a quest letter/button offered by an NPC script.
    QuestButton(u32),
    UseItem(ItemPosition),
    MoveItem {source:ItemPosition,destination:ItemPosition,count:u8},
    SafeboxCheckin {inventory:ItemPosition,safe_cell:u8},
    SafeboxCheckout {safe_cell:u8,inventory:ItemPosition},
    SafeboxMove {source:u8,destination:u8,count:u8},
    MallCheckout {mall_cell:u8,inventory:ItemPosition},
    ShopBuy(u8),
    ShopSell {position:u8,count:u8},
    ShopEnd,
    BindItem {slot:u8,position:ItemPosition},
    /// `SendQuickSlotAddPacket` for a skill or emotion (`SLOT_TYPE_SKILL`/`_EMOTION`).
    BindQuickslot {slot:u8,kind:u8,position:u8},
    ClearQuickslot(u8),
    SwapQuickslot {source:u8,destination:u8},
}

pub struct PlayerStatePlugin;
impl Plugin for PlayerStatePlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<PlayerState>()
            .init_resource::<ItemCatalog>()
            .init_resource::<StorageState>()
            .init_resource::<ShopState>()
            .add_message::<PlayerAction>()
            .add_systems(Startup,load_catalog)
            .add_systems(Update,(poll_catalog,receive,dispatch).chain());
    }
}
fn load_catalog(mut commands:Commands,packs:Option<Res<M2Packs>>) {
    let Some(packs)=packs else{return;};let packs=packs.packs.clone();
    commands.insert_resource(CatalogTask(IoTaskPool::get().spawn(async move {
        let mut items=ItemTables::default();
        if let Ok(data)=packs.get(format!("{LOCALE_PATH}/item_list.txt")){items.load_item_list(&data);}
        let data=packs.get(format!("{LOCALE_PATH}/item_proto")).map_err(|e|format!("item_proto: {e:?}"))?;
        let tables=parse_item_proto(&data).map_err(|e|format!("item_proto: {e:?}"))?;
        items.load_item_table(&tables,|name|packs.exists(paths::virtual_name(&String::from_utf8_lossy(name))));
        if let Ok(data)=packs.get(format!("{LOCALE_PATH}/itemdesc.txt")){items.load_item_desc(&data);}
        Ok(items)
    })));
}
fn poll_catalog(mut commands:Commands,task:Option<ResMut<CatalogTask>>,mut catalog:ResMut<ItemCatalog>) {
    let Some(mut task)=task else{return;};
    if let Some(result)=check_ready(&mut task.0){match result{Ok(items)=>catalog.0=items,Err(e)=>warn!("{e}")};commands.remove_resource::<CatalogTask>();}
}
fn receive(
    mut events:MessageReader<NetMessage>,
    mut player:ResMut<PlayerState>,
    mut storage:ResMut<StorageState>,
    mut shop:ResMut<ShopState>,
) {
    for event in events.read() {
        match event {
            NetMessage::Game(event) => {
                player.apply(event);
                storage.apply(event);
                shop.apply(event);
            }
            NetMessage::Disconnected { .. } => {
                *player = PlayerState::default();
                *storage = StorageState::default();
                shop.clear();
            }
            _ => {}
        }
    }
}
fn dispatch(
    mut actions:MessageReader<PlayerAction>,
    mut net:ResMut<Net>,
    mut player:ResMut<PlayerState>,
    storage:Res<StorageState>,
    shop:Res<ShopState>,
) {
    for action in actions.read() {
        if net.game.phase()!=Phase::Game { continue; }
        let ok=match action {
            PlayerAction::Chat{text,kind} => !text.is_empty() && text.len()<512 && !text.chars().any(char::is_control) && matches!(kind,0|3|4|6) && net.game.send_chat(text,*kind),
            PlayerAction::QuestAnswer(answer) => net.game.send_script_answer(*answer),
            PlayerAction::QuestButton(index) => net.game.send_script_button(*index),
            PlayerAction::UseItem(pos) => pos.valid() && player.items.contains_key(pos) && net.game.stream.send_packet(&wire::TPacketCGItemUse{header:cg::ITEM_USE,pos:pos.wire()}),
            PlayerAction::MoveItem{source,destination,count} => {
                source!=destination && source.valid() && destination.valid() && player.items.get(source).is_some_and(|i|*count<=i.count) && net.game.stream.send_packet(&wire::TPacketCGItemMove{header:cg::ITEM_MOVE,pos:source.wire(),change_pos:destination.wire(),num:*count})
            }
            PlayerAction::SafeboxCheckin { inventory, safe_cell } => {
                storage.safebox_open
                    && usize::from(*safe_cell) < storage.safebox.len()
                    && inventory.window == 1
                    && inventory.cell < 90
                    && player.items.contains_key(inventory)
                    && net.game.send_safebox_checkin(inventory.wire(), *safe_cell)
            }
            PlayerAction::SafeboxCheckout { safe_cell, inventory } => {
                storage.safebox_open
                    && storage.safebox.get(usize::from(*safe_cell)).is_some_and(Option::is_some)
                    && inventory.window == 1
                    && inventory.cell < 90
                    && !player.items.contains_key(inventory)
                    && net.game.send_safebox_checkout(*safe_cell, inventory.wire())
            }
            PlayerAction::SafeboxMove { source, destination, count } => {
                storage.safebox_open
                    && usize::from(*source) < storage.safebox.len()
                    && usize::from(*destination) < storage.safebox.len()
                    && *source != *destination
                    && *count > 0
                    && storage.safebox[usize::from(*source)].is_some()
                    && net.game.send_safebox_item_move(*source, *destination, *count)
            }
            PlayerAction::MallCheckout { mall_cell, inventory } => {
                storage.mall_open
                    && storage.mall.get(usize::from(*mall_cell)).is_some_and(Option::is_some)
                    && inventory.window == 1
                    && inventory.cell < 90
                    && !player.items.contains_key(inventory)
                    && net.game.send_mall_checkout(*mall_cell, inventory.wire())
            }
            PlayerAction::ShopBuy(position) => {
                shop.open && usize::from(*position) < shop.tabs.len().saturating_mul(40) && net.game.send_shop_buy(*position)
            }
            PlayerAction::ShopSell { position, count } => {
                shop.open
                    && *count > 0
                    && *position < 90
                    && player.items.get(&ItemPosition::inventory(u16::from(*position))).is_some_and(|item| *count <= item.count)
                    && net.game.send_shop_sell(*position, *count)
            }
            PlayerAction::ShopEnd => shop.open && net.game.send_shop_end(),
            PlayerAction::BindItem{slot,position}=>*slot<32 && position.window==1 && position.cell<90 && player.items.contains_key(position) && net.game.stream.send_packet(&wire::TPacketCGQuickSlotAdd{header:cg::QUICKSLOT_ADD,pos:*slot,slot:TQuickSlot{Type:1,Position:position.cell as u8}}),
            PlayerAction::BindQuickslot{slot,kind,position}=>*slot<32 && matches!(*kind,2|3) && net.game.stream.send_packet(&wire::TPacketCGQuickSlotAdd{header:cg::QUICKSLOT_ADD,pos:*slot,slot:TQuickSlot{Type:*kind,Position:*position}}),
            PlayerAction::ClearQuickslot(slot)=>*slot<32 && net.game.stream.send_packet(&wire::TPacketCGQuickSlotDel{header:cg::QUICKSLOT_DEL,pos:*slot}),
            PlayerAction::SwapQuickslot{source,destination}=>*source<32 && *destination<32 && source!=destination && net.game.stream.send_packet(&wire::TPacketCGQuickSlotSwap{header:cg::QUICKSLOT_SWAP,pos:*source,change_pos:*destination}),
        };
        if !ok {player.line(1,"That action could not be sent.".into());}
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn updates_are_authoritative_and_keep_attributes_and_equipment() {
        let mut state=PlayerState::default();
        let p=wire::TPacketGCItemSet2{Cell:TItemPos{window_type:2,cell:94},vnum:10,count:1,alSockets:[2,3,4],..default()};
        state.apply(&NetEvent::Packet(GcPacket::ItemSet2(p)));
        assert_eq!(state.items[&ItemPosition::inventory(94)].sockets,[2,3,4]);
        state.apply(&NetEvent::Packet(GcPacket::ItemSet2(wire::TPacketGCItemSet2{vnum:0,..p})));
        assert!(state.items.is_empty());
    }
    #[test] fn other_actors_points_do_not_change_the_player() {
        let mut state=PlayerState{vid:42,..default()};
        let p=wire::TPacketGCPointChange{dwVID:9,Type:5,value:300,..default()};
        state.apply(&NetEvent::Packet(GcPacket::PlayerPointChange(p)));
        assert_eq!(state.points[5],0);
        state.apply(&NetEvent::Packet(GcPacket::PlayerPointChange(wire::TPacketGCPointChange{dwVID:42,..p})));
        assert_eq!(state.points[5],300);
    }
    #[test] fn main_character_keeps_equipment_that_arrived_first() {
        let mut state=PlayerState::default();
        let weapon=wire::TPacketGCItemSet2{Cell:TItemPos{window_type:2,cell:EQUIPMENT_WEAPON},vnum:4010,count:1,..default()};
        state.apply(&NetEvent::Packet(GcPacket::ItemSet2(weapon)));
        let main=m2_net::phase::MainCharacter{vid:7,race:5,empire:0,skill_group:0,name:"ninja".into(),x:0,y:0,bgm:None};
        state.apply(&NetEvent::MainCharacter(main));
        assert_eq!(state.vid,7);
        assert_eq!(state.weapon().map(|item|item.vnum),Some(4010));
        // `MainCharacter` creates the main instance as `TYPE_PC`.
        assert!(state.combat_self().is_pc());
    }
    #[test] fn own_actor_metadata_and_mount_come_from_the_packets() {
        let mut state=PlayerState { vid: 7, ..PlayerState::default() };
        let actor=m2_net::phase::ActorData{vid:7,kind:6,race:5,move_speed:0,attack_speed:0,state_flags:0,rotation:0.0,x:10,y:20,affect_flags:0,name:None,guild_id:12,level:30,alignment:-5000,pk_mode:2,empire:1,armor:0,weapon:0,hair:0,mount_vnum:0};
        state.apply(&NetEvent::ActorAppend(actor));
        assert_eq!((state.pk_mode,state.empire,state.guild_id,state.alignment),(2,1,12,-5000));
        state.apply(&NetEvent::Packet(GcPacket::Mount(wire::TPacketGCMount{vid:7,mount_vid:20104,..default()})));
        assert_eq!(state.mount_vnum,20104);
        state.apply(&NetEvent::ActorUpdate(m2_net::phase::ActorUpdate { vid:7, guild_id:12, move_speed:100, attack_speed:100, state_flags:0, affect_flags:0, alignment:-5000, pk_mode:2, armor:0, weapon:0, hair:0, mount_vnum:0 }));
        assert_eq!(state.mount_vnum,0);
        // A buff's type is not a flag bit: type 23 must not turn on bit 23.
        state.apply(&NetEvent::Packet(GcPacket::AffectAdd(wire::TPacketGCAffectAdd{elem:wire::TPacketAffectElement{dwType:23,..default()},..default()})));
        assert_eq!(state.affect_flags,0);
    }
    #[test] fn quest_packets_begin_update_and_end_a_quest() {
        let mut state = PlayerState::default();
        let packet = |flag, tail| NetEvent::Packet(GcPacket::QuestInfo { head: wire::TPacketGCQuestInfo { header: wire::gc::QUEST_INFO, size: 6, index: 42, flag }, tail });
        state.apply(&packet(1, vec![1]));
        let mut title = vec![0; 31];
        title[..5].copy_from_slice(b"Quest");
        state.apply(&packet(2, title));
        assert_eq!(state.quests[&42].title, "Quest");
        state.apply(&packet(1, vec![0]));
        assert!(!state.quests.contains_key(&42));
    }
    #[test] fn party_packets_keep_member_information_across_link_changes() {
        let mut state = PlayerState::default();
        let mut add = wire::TPacketGCPartyAdd { pid: 8, ..default() };
        add.set_name_str("Ally");
        state.apply(&NetEvent::Packet(GcPacket::PartyAdd(add)));
        state.apply(&NetEvent::Packet(GcPacket::PartyUpdate(wire::TPacketGCPartyUpdate { pid: 8, percent_hp: 73, state: 2, ..default() })));
        state.apply(&NetEvent::Packet(GcPacket::PartyLink(wire::TPacketGCPartyLink { pid: 8, vid: 15, ..default() })));
        state.apply(&NetEvent::Packet(GcPacket::PartyUnlink(wire::TPacketGCPartyUnlink { pid: 8, vid: 15, ..default() })));
        assert_eq!((state.party[&8].name.as_str(), state.party[&8].percent_hp, state.party[&8].vid), ("Ally", 73, 0));
        state.apply(&NetEvent::Packet(GcPacket::PartyRemove(wire::TPacketGCPartyRemove { pid: 8, ..default() })));
        assert!(state.party.is_empty());
    }
    #[test] fn own_gm_affect_comes_from_the_character_packet() {
        let mut state=PlayerState { vid: 7, ..PlayerState::default() };
        let actor=m2_net::phase::ActorData{vid:7,kind:6,race:0,move_speed:100,attack_speed:100,state_flags:0,rotation:0.0,x:0,y:0,affect_flags:1,name:None,guild_id:0,level:1,alignment:0,pk_mode:0,empire:2,armor:0,weapon:0,hair:0,mount_vnum:0};
        state.apply(&NetEvent::ActorAppend(actor));
        assert!(combat::has_affect(state.affect_flags,0));
    }
    #[test] fn weapon_cell_is_the_cpp_equipment_weapon_slot() {
        // GameType.h: `c_Equipment_Start = c_ItemSlot_Count` (90),
        // `c_Equipment_Weapon = c_Equipment_Start + 4`.
        assert_eq!(EQUIPMENT_WEAPON,94);
        assert_eq!(ItemPosition::from_wire(TItemPos{window_type:2,cell:94}),ItemPosition::equipment(94));
    }
    #[test] fn weapon_visual_uses_the_item_model_and_sub_model() {
        let mut catalog=ItemCatalog::default();
        catalog.0.items.insert(4010,ItemData{
            model_file_name:b"d:/ymir work/item/weapon/04010.gr2".to_vec(),
            sub_model_file_name:b"d:/ymir work/item/weapon/04010_1.gr2".to_vec(),
            table:m2_formats::proto::ItemTable{sub_type:1,..default()},
            ..default()
        });
        let packs=m2_formats::pack::PackManager::new("/nonexistent");
        let weapon=catalog.weapon_visual(&packs,4010).unwrap();
        assert_eq!(weapon.model,"ymir work/item/weapon/04010.gr2");
        assert_eq!(weapon.sub_model,"ymir work/item/weapon/04010_1.gr2");
        assert_eq!(weapon.weapon_type,1);
    }
}
