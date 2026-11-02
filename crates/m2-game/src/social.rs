//! Game-phase packets that only keep state for the windows (`CPythonExchange`,
//! `CPythonGuild`, `CPythonMessenger`, the mini map's marks and targets, the
//! affect list) or ask the player a question (`RecvPartyInvite`,
//! `RecvQuestConfirmPacket`, ...). The packets that change what is drawn
//! (dig and fishing motions, specific effects) are handled by `ingame`.
//!
//! [`SocialState`] holds what the C++ singletons hold; [`SocialEvent`] carries
//! the one-shot Python callbacks (`PyCallClassMemberFunc`) for the UI.

use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::prelude::*;
use m2_net::phase::NetEvent;
use m2_net::wire::{
    self, DecodeError, GcPacket, Reader, TPacketAffectElement, TPacketGCGuildInfo, TPacketGCGuildSubGrade,
    TPacketGCGuildSubMember, TPacketGCGuildWar, TPacketGuildWarPoint, TPlayerItemAttribute, Wire, cstr_to_string,
};

use crate::net::NetMessage;
use crate::player::{ItemStack, PlayerState};

/// `EXCHANGE_SUBHEADER_GC_*` (`Packet.h`).
pub mod exchange {
    pub const START: u8 = 0;
    pub const ITEM_ADD: u8 = 1;
    pub const ITEM_DEL: u8 = 2;
    pub const ELK_ADD: u8 = 3;
    pub const ACCEPT: u8 = 4;
    pub const END: u8 = 5;
    pub const ALREADY: u8 = 6;
    pub const LESS_ELK: u8 = 7;
}

/// `MESSENGER_SUBHEADER_GC_*`.
pub mod messenger {
    pub const LIST: u8 = 0;
    pub const LOGIN: u8 = 1;
    pub const LOGOUT: u8 = 2;
    /// `MESSENGER_CONNECTED_STATE_ONLINE`.
    pub const ONLINE: u8 = 1;
}

/// `GUILD_SUBHEADER_GC_*`.
pub mod guild {
    pub const LOGIN: u8 = 0;
    pub const LOGOUT: u8 = 1;
    pub const LIST: u8 = 2;
    pub const GRADE: u8 = 3;
    pub const REMOVE: u8 = 5;
    pub const GRADE_NAME: u8 = 6;
    pub const GRADE_AUTH: u8 = 7;
    pub const INFO: u8 = 8;
    pub const COMMENTS: u8 = 9;
    pub const CHANGE_EXP: u8 = 10;
    pub const CHANGE_MEMBER_GRADE: u8 = 11;
    pub const SKILL_INFO: u8 = 12;
    pub const CHANGE_MEMBER_GENERAL: u8 = 13;
    pub const GUILD_INVITE: u8 = 14;
    pub const WAR: u8 = 15;
    pub const GUILD_NAME: u8 = 16;
    pub const GUILD_WAR_LIST: u8 = 17;
    pub const GUILD_WAR_END_LIST: u8 = 18;
    pub const WAR_POINT: u8 = 19;
    pub const MONEY_CHANGE: u8 = 20;
    /// `GUILD_WAR_*` states of `GUILD_SUBHEADER_GC_WAR`.
    pub const WAR_SEND_DECLARE: u8 = 1;
    pub const WAR_RECV_DECLARE: u8 = 3;
    pub const WAR_ON_WAR: u8 = 6;
    pub const WAR_END: u8 = 7;
    /// `GUILD_SKILL_MAX_NUM` (`PythonGuild.h`).
    pub const SKILL_MAX_NUM: usize = 12;
    /// `CHARACTER_NAME_MAX_LEN + 1`, `GUILD_NAME_MAX_LEN`, `GUILD_GRADE_NAME_MAX_LEN + 1`,
    /// `GULID_COMMENT_MAX_LEN + 1`.
    pub const NAME_LEN: usize = 25;
    pub const GUILD_NAME_LEN: usize = 12;
    pub const GRADE_NAME_LEN: usize = 9;
    pub const COMMENT_LEN: usize = 51;
}

/// `FISHING_SUBHEADER_GC_*`.
pub mod fishing {
    pub const START: u8 = 0;
    pub const STOP: u8 = 1;
    pub const REACT: u8 = 2;
    pub const SUCCESS: u8 = 3;
    pub const FAIL: u8 = 4;
    pub const FISH: u8 = 5;
}

/// `DS_SUB_HEADER_*`.
pub mod dragon_soul {
    pub const OPEN: u8 = 0;
    pub const REFINE_FAIL: u8 = 5;
    pub const REFINE_FAIL_TOO_MUCH_MATERIAL: u8 = 10;
    pub const REFINE_SUCCEED: u8 = 11;
}

/// `DUNGEON_SUBHEADER_GC_DESTINATION_POSITION`.
const DUNGEON_DESTINATION_POSITION: u8 = 1;
/// `CREATE_TARGET_TYPE_LOCATION`.
const CREATE_TARGET_TYPE_LOCATION: u8 = 1;
/// `EXCHANGE_ITEM_MAX_NUM` (`PythonExchange.h`).
pub const EXCHANGE_ITEM_MAX_NUM: usize = 12;

/// One side of `CPythonExchange` (`TExchangeData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExchangeSide {
    pub name: String,
    pub items: [Option<ItemStack>; EXCHANGE_ITEM_MAX_NUM],
    pub elk: u32,
    pub accept: bool,
}

/// `CPythonExchange`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exchange {
    pub trading: bool,
    pub me: ExchangeSide,
    pub target: ExchangeSide,
}

/// `CPythonGuild::SGuildMemberData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildMember {
    pub pid: u32,
    pub grade: u8,
    pub name: String,
    pub job: u8,
    pub level: u8,
    pub offer: u32,
    pub general: bool,
}

/// `CPythonGuild::TGuildInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildInfo {
    pub name: String,
    pub id: u32,
    pub master_pid: u32,
    pub level: u32,
    pub exp: u32,
    pub member_count: u32,
    pub max_member_count: u32,
    pub money: u32,
    pub has_land: bool,
}

/// `CPythonGuild::TGuildSkillData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildSkills {
    pub point: u8,
    pub levels: [u8; guild::SKILL_MAX_NUM],
    pub guild_point: u16,
    pub max_guild_point: u16,
}

/// `CPythonGuild`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Guild {
    pub enabled: bool,
    pub info: GuildInfo,
    pub members: BTreeMap<u32, GuildMember>,
    /// Grade index -> (name, authority flags).
    pub grades: BTreeMap<u8, (String, u8)>,
    /// `RegisterComment`: (id, name, comment).
    pub comments: Vec<(u32, String, String)>,
    pub skills: GuildSkills,
    /// `m_GuildWarOpponentSet` (`StartGuildWar`).
    pub wars: HashSet<u32>,
    /// `RegisterGuildName`: every guild the server named.
    pub names: HashMap<u32, String>,
}

/// `CPythonMessenger`: friends and guild members, name -> online.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Messenger {
    pub friends: BTreeMap<String, bool>,
    pub guild: BTreeMap<String, bool>,
}

/// A mini-map atlas mark (`RegisterAtlasMark`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtlasMark {
    pub kind: u8,
    pub name: String,
    pub x: i32,
    pub y: i32,
}

/// A guild's land (`RegisterGuildArea`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuildArea {
    pub id: u32,
    pub guild_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// A quest target on the mini map (`CPythonMiniMap::CreateTarget`) and its
/// ground arrow (`CPythonBackground::CreateTargetEffect`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestTarget {
    pub name: String,
    /// The character it follows, 0 for a fixed location.
    pub vid: u32,
    /// Set by `GC_TARGET_UPDATE`.
    pub position: Option<(i32, i32)>,
}

/// `TPacketGCViewEquip`: another player's equipment, by wear index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewEquip {
    pub vid: u32,
    pub items: Vec<Option<ItemStack>>,
}

/// `OpenRefineDialog` + `AppendMaterialToRefineDialog`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefineDialog {
    pub pos: u8,
    pub result_vnum: u32,
    pub cost: i32,
    pub prob: i32,
    /// `TPacketGCRefineInformationNew::type`; 0 for the old packet.
    pub kind: u8,
    pub materials: Vec<(u32, u32)>,
}

/// State kept from the packets this module handles.
#[derive(Resource, Clone, Debug, Default)]
pub struct SocialState {
    /// `BINARY_NEW_AddAffect` list, by `(type, apply_on)`.
    pub affects: BTreeMap<(u32, u8), TPacketAffectElement>,
    pub exchange: Exchange,
    pub guild: Guild,
    pub messenger: Messenger,
    /// `ChangePartyParameter`: the item distribution mode.
    pub party_distribute_mode: u8,
    pub atlas_marks: Vec<AtlasMark>,
    pub guild_areas: Vec<GuildArea>,
    pub targets: BTreeMap<i32, QuestTarget>,
    /// `SetDungeonDestinationPosition`.
    pub dungeon_destination: Option<(u32, u32)>,
    /// `BINARY_PrivateShop_Appear`: shop signs over characters.
    pub shop_signs: HashMap<u32, String>,
    /// `BINARY_LoverInfo` / `BINARY_UpdateLovePoint`.
    pub lover: Option<(String, u8)>,
    pub view_equip: Option<ViewEquip>,
    pub refine: Option<RefineDialog>,
    /// Whether the main character has its line out (`IsFishing`).
    pub fishing: bool,
}

/// The Python callbacks the UI answers (`uiGame`/`game.py` methods).
#[derive(Message, Clone, Debug, PartialEq)]
pub enum SocialEvent {
    /// `RecvPartyInviteQuestion(leader_pid, name)`.
    PartyInvite { leader: u32, name: String },
    /// `RecvGuildInviteQuestion(guild_id, name)`.
    GuildInvite { guild_id: u32, name: String },
    /// `AskGuildName()`.
    AskGuildName,
    /// `BINARY_OnQuestConfirm(msg, timeout, pid)`.
    QuestConfirm { message: String, timeout: i32, pid: u32 },
    StartExchange,
    EndExchange,
    /// `BINARY_GuildWar_On*`.
    GuildWar { state: u8, self_guild: u32, opponent: u32, kind: u8 },
    /// `BINARY_GuildWar_OnRecvPoint`.
    GuildWarPoint { gain: u32, opponent: u32, point: i32 },
    /// `DeleteGuild()`: the main player left the guild.
    GuildDeleted,
    /// `OnFishingFailure`; `OnFishingNotify(is_fish, name)` when the line is
    /// still out (`while_fishing`), else `OnFishingSuccess`; `OnFishingNotifyUnknown`.
    FishingFailure,
    FishingCatch { vnum: u32, while_fishing: bool },
    FishingUnknown,
    /// `BINARY_DragonSoulRefineWindow_*`.
    DragonSoulRefine { result: u8, window: u8, cell: u16 },
    OpenAtlas,
}

pub struct SocialPlugin;

impl Plugin for SocialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SocialState>().add_message::<SocialEvent>().add_systems(Update, receive);
    }
}

fn receive(
    mut messages: MessageReader<NetMessage>,
    mut state: ResMut<SocialState>,
    mut events: MessageWriter<SocialEvent>,
    player: Option<Res<PlayerState>>,
    actors: Option<Res<crate::ingame::Actors>>,
) {
    for message in messages.read() {
        match message {
            NetMessage::Disconnected { .. } | NetMessage::Game(NetEvent::MainCharacter(_)) => {
                *state = SocialState::default();
            }
            NetMessage::Game(NetEvent::Packet(packet)) => {
                let name_of = |vid: u32| {
                    if player.as_ref().is_some_and(|player| player.vid == vid) {
                        return player.as_ref().map(|player| player.name.clone());
                    }
                    actors.as_ref().and_then(|actors| actors.by_vid.get(&vid)).map(|actor| actor.name.clone())
                };
                let me = player.as_ref().map_or((0, String::new()), |player| (player.vid, player.name.clone()));
                if let Err(error) = state.apply(packet, &me, &name_of, &mut |event| {
                    events.write(event);
                }) {
                    warn!("malformed social packet: {error:?}");
                }
            }
            _ => {}
        }
    }
}

fn item(vnum: u32, count: u8, sockets: [i32; 3], attributes: [TPlayerItemAttribute; 7]) -> ItemStack {
    ItemStack { vnum, count, flags: 0, anti_flags: 0, sockets, attributes }
}

impl SocialState {
    /// The `Recv*` handlers of the packets above. `me` is the main
    /// character's (vid, name); `name_of` is `CInstanceBase::GetNameString`.
    pub fn apply(
        &mut self,
        packet: &GcPacket,
        me: &(u32, String),
        name_of: &dyn Fn(u32) -> Option<String>,
        emit: &mut dyn FnMut(SocialEvent),
    ) -> Result<(), DecodeError> {
        match packet {
            GcPacket::AffectAdd(p) => {
                self.affects.insert((p.elem.dwType, p.elem.bPointIdxApplyOn), p.elem);
            }
            GcPacket::AffectRemove(p) => {
                self.affects.remove(&(p.dwType, p.bApplyOn));
            }
            GcPacket::Exchange(p) => self.apply_exchange(p, me, name_of, emit),
            GcPacket::Messenger { head, tail } => self.apply_messenger(head.subheader, tail)?,
            GcPacket::Guild { head, tail } => self.apply_guild(head.subheader, tail, me, emit)?,
            GcPacket::PartyInvite(p) => {
                // `RecvPartyInvite` asks only when the leader is in view.
                if let Some(name) = name_of(p.leader_pid) {
                    emit(SocialEvent::PartyInvite { leader: p.leader_pid, name });
                }
            }
            GcPacket::PartyParameter(p) => self.party_distribute_mode = p.bDistributeMode,
            GcPacket::QuestConfirm(p) => emit(SocialEvent::QuestConfirm {
                message: cstr_to_string(&p.msg),
                timeout: p.timeout,
                pid: p.requestPID,
            }),
            GcPacket::RequestMakeGuild(_) => emit(SocialEvent::AskGuildName),
            GcPacket::ShopSign(p) => {
                let sign = cstr_to_string(&p.szSign);
                if sign.is_empty() {
                    self.shop_signs.remove(&p.dwVID);
                } else {
                    self.shop_signs.insert(p.dwVID, sign);
                }
            }
            GcPacket::LoverInfo(p) => self.lover = Some((cstr_to_string(&p.szName), p.byLovePoint)),
            GcPacket::LovePointUpdate(p) => {
                if let Some((_, point)) = &mut self.lover {
                    *point = p.byLovePoint;
                } else {
                    self.lover = Some((String::new(), p.byLovePoint));
                }
            }
            GcPacket::NpcPosition { head, tail } => {
                // `ClearAtlasMarkInfo` + `RegisterAtlasMark` per entry.
                self.atlas_marks.clear();
                let mut reader = Reader::new(tail);
                for _ in 0..head.count {
                    let mark = wire::TNPCPosition::read(&mut reader)?;
                    self.atlas_marks.push(AtlasMark { kind: mark.bType, name: cstr_to_string(&mark.name), x: mark.x, y: mark.y });
                }
            }
            GcPacket::LandList { tail, .. } => {
                self.guild_areas.clear();
                let mut reader = Reader::new(tail);
                while reader.remaining() >= wire::TLandPacketElement::SIZE {
                    let land = wire::TLandPacketElement::read(&mut reader)?;
                    self.guild_areas.push(GuildArea {
                        id: land.dwID,
                        guild_id: land.dwGuildID,
                        x: land.x,
                        y: land.y,
                        width: land.width,
                        height: land.height,
                    });
                }
            }
            GcPacket::TargetCreateNew(p) => {
                let vid = if p.byType == CREATE_TARGET_TYPE_LOCATION { 0 } else { p.dwVID };
                self.targets.insert(p.lID, QuestTarget { name: cstr_to_string(&p.szTargetName), vid, position: None });
                emit(SocialEvent::OpenAtlas);
            }
            GcPacket::TargetUpdate(p) => {
                if let Some(target) = self.targets.get_mut(&p.lID) {
                    target.position = Some((p.lX, p.lY));
                }
            }
            GcPacket::TargetDelete(p) => {
                self.targets.remove(&p.lID);
            }
            GcPacket::Dungeon { head, tail } => {
                if head.subheader == DUNGEON_DESTINATION_POSITION {
                    let mut reader = Reader::new(tail);
                    self.dungeon_destination = Some((u32::read(&mut reader)?, u32::read(&mut reader)?));
                }
            }
            GcPacket::ViewEquip(p) => {
                let items = p
                    .equips
                    .iter()
                    .map(|set| (set.vnum != 0).then(|| item(set.vnum, set.count, set.alSockets, set.aAttr)))
                    .collect();
                self.view_equip = Some(ViewEquip { vid: p.dwVID, items });
            }
            GcPacket::RefineInformation(p) => self.refine = Some(refine_dialog(p.pos, 0, &p.refine_table)),
            GcPacket::RefineInformationNew(p) => self.refine = Some(refine_dialog(p.pos, p.r#type, &p.refine_table)),
            GcPacket::DragonSoulRefine(p) => match p.bSubType {
                dragon_soul::OPEN
                | dragon_soul::REFINE_FAIL..=dragon_soul::REFINE_FAIL_TOO_MUCH_MATERIAL
                | dragon_soul::REFINE_SUCCEED => {
                    emit(SocialEvent::DragonSoulRefine { result: p.bSubType, window: p.Pos.window_type, cell: p.Pos.cell })
                }
                _ => {}
            },
            GcPacket::Fishing(p) => match p.subheader {
                // The motions are `ingame`'s; the notices are the UI's.
                fishing::FISH if p.info == 0 => emit(SocialEvent::FishingUnknown),
                fishing::FISH => emit(SocialEvent::FishingCatch { vnum: p.info, while_fishing: self.fishing }),
                fishing::START if p.info == me.0 => self.fishing = true,
                fishing::STOP | fishing::SUCCESS if p.info == me.0 => self.fishing = false,
                fishing::FAIL if p.info == me.0 => {
                    self.fishing = false;
                    emit(SocialEvent::FishingFailure);
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// `CPythonNetworkStream::RecvExchangePacket`.
    fn apply_exchange(
        &mut self,
        p: &wire::TPacketGCExchange,
        me: &(u32, String),
        name_of: &dyn Fn(u32) -> Option<String>,
        emit: &mut dyn FnMut(SocialEvent),
    ) {
        let exchange = &mut self.exchange;
        let side = if p.is_me != 0 { &mut exchange.me } else { &mut exchange.target };
        match p.subheader {
            exchange::START => {
                *exchange = Exchange { trading: true, ..Exchange::default() };
                exchange.me.name = me.1.clone();
                if let Some(name) = name_of(p.arg1) {
                    exchange.target.name = name;
                }
                emit(SocialEvent::StartExchange);
            }
            exchange::ITEM_ADD => {
                if let Some(slot) = side.items.get_mut(usize::from(p.arg2.cell)) {
                    *slot = Some(item(p.arg1, p.arg3 as u8, p.alValues, p.aAttr));
                }
            }
            exchange::ITEM_DEL => {
                if let Some(slot) = side.items.get_mut(p.arg1 as u8 as usize) {
                    *slot = None;
                }
            }
            exchange::ELK_ADD => side.elk = p.arg1,
            exchange::ACCEPT => side.accept = p.arg1 as u8 != 0,
            exchange::END => {
                emit(SocialEvent::EndExchange);
                exchange.trading = false;
            }
            exchange::ALREADY | exchange::LESS_ELK => {}
            _ => {}
        }
    }

    /// `CPythonNetworkStream::RecvMessenger`.
    fn apply_messenger(&mut self, subheader: u8, tail: &[u8]) -> Result<(), DecodeError> {
        let mut reader = Reader::new(tail);
        let name = |reader: &mut Reader, length: u8| -> Result<String, DecodeError> {
            Ok(cstr_to_string(reader.take(usize::from(length))?))
        };
        match subheader {
            messenger::LIST => {
                while reader.remaining() > 0 {
                    let connected = u8::read(&mut reader)?;
                    let length = u8::read(&mut reader)?;
                    let friend = name(&mut reader, length)?;
                    self.messenger.friends.insert(friend, connected & messenger::ONLINE != 0);
                }
            }
            messenger::LOGIN | messenger::LOGOUT => {
                let length = u8::read(&mut reader)?;
                let friend = name(&mut reader, length)?;
                self.messenger.friends.insert(friend, subheader == messenger::LOGIN);
            }
            _ => {}
        }
        Ok(())
    }

    /// `CPythonNetworkStream::RecvGuild`. The war keys it inserts
    /// (`InsertGVGKey`) are returned by [`Self::gvg_keys`].
    fn apply_guild(
        &mut self,
        subheader: u8,
        tail: &[u8],
        me: &(u32, String),
        emit: &mut dyn FnMut(SocialEvent),
    ) -> Result<(), DecodeError> {
        let mut reader = Reader::new(tail);
        let guild_state = &mut self.guild;
        let messenger = &mut self.messenger;
        match subheader {
            guild::LOGIN | guild::LOGOUT => {
                let pid = u32::read(&mut reader)?;
                if let Some(member) = guild_state.members.get(&pid)
                    && member.name != me.1
                {
                    messenger.guild.insert(member.name.clone(), subheader == guild::LOGIN);
                }
            }
            guild::REMOVE => {
                let pid = u32::read(&mut reader)?;
                // `IsMainPlayer`: the member whose name is the player's.
                if guild_state.members.get(&pid).is_some_and(|member| member.name == me.1) {
                    *guild_state = Guild { names: std::mem::take(&mut guild_state.names), ..Guild::default() };
                    messenger.guild.clear();
                    emit(SocialEvent::GuildDeleted);
                } else if let Some(member) = guild_state.members.remove(&pid) {
                    messenger.guild.remove(&member.name);
                }
            }
            guild::LIST => {
                while reader.remaining() > 0 {
                    let member = TPacketGCGuildSubMember::read(&mut reader)?;
                    let name = if member.byNameFlag != 0 {
                        cstr_to_string(reader.take(guild::NAME_LEN)?)
                    } else {
                        guild_state.members.get(&member.pid).map(|known| known.name.clone()).unwrap_or_default()
                    };
                    if name != me.1 {
                        messenger.guild.entry(name.clone()).or_insert(false);
                    }
                    guild_state.members.insert(
                        member.pid,
                        GuildMember {
                            pid: member.pid,
                            grade: member.byGrade,
                            name,
                            job: member.byJob,
                            level: member.byLevel,
                            offer: member.dwOffer,
                            general: member.byIsGeneral != 0,
                        },
                    );
                }
            }
            guild::GRADE => {
                let count = u8::read(&mut reader)?;
                for _ in 0..count {
                    let index = u8::read(&mut reader)?;
                    let grade = TPacketGCGuildSubGrade::read(&mut reader)?;
                    guild_state.grades.insert(index, (cstr_to_string(&grade.grade_name), grade.auth_flag));
                }
            }
            guild::GRADE_NAME => {
                let index = u8::read(&mut reader)?;
                let name = cstr_to_string(reader.take(guild::GRADE_NAME_LEN)?);
                guild_state.grades.entry(index).or_default().0 = name;
            }
            guild::GRADE_AUTH => {
                let index = u8::read(&mut reader)?;
                let flag = u8::read(&mut reader)?;
                guild_state.grades.entry(index).or_default().1 = flag;
            }
            guild::INFO => {
                let info = TPacketGCGuildInfo::read(&mut reader)?;
                guild_state.enabled = true;
                guild_state.info = GuildInfo {
                    name: cstr_to_string(&info.name),
                    id: info.guild_id,
                    master_pid: info.master_pid,
                    level: u32::from(info.level),
                    exp: info.exp,
                    member_count: u32::from(info.member_count),
                    max_member_count: u32::from(info.max_member_count),
                    money: info.gold,
                    has_land: info.hasLand != 0,
                };
            }
            guild::COMMENTS => {
                let count = u8::read(&mut reader)?;
                guild_state.comments.clear();
                for _ in 0..count {
                    let id = u32::read(&mut reader)?;
                    let name = cstr_to_string(reader.take(guild::NAME_LEN)?);
                    let comment = cstr_to_string(reader.take(guild::COMMENT_LEN)?);
                    guild_state.comments.push((id, name, comment));
                }
            }
            guild::CHANGE_EXP => {
                guild_state.info.level = u32::from(u8::read(&mut reader)?);
                guild_state.info.exp = u32::read(&mut reader)?;
            }
            guild::CHANGE_MEMBER_GRADE => {
                let pid = u32::read(&mut reader)?;
                let grade = u8::read(&mut reader)?;
                if let Some(member) = guild_state.members.get_mut(&pid) {
                    member.grade = grade;
                }
            }
            guild::SKILL_INFO => {
                let skills = &mut guild_state.skills;
                skills.point = u8::read(&mut reader)?;
                skills.levels.copy_from_slice(reader.take(guild::SKILL_MAX_NUM)?);
                skills.guild_point = u16::read(&mut reader)?;
                skills.max_guild_point = u16::read(&mut reader)?;
            }
            guild::CHANGE_MEMBER_GENERAL => {
                let pid = u32::read(&mut reader)?;
                let flag = u8::read(&mut reader)?;
                if let Some(member) = guild_state.members.get_mut(&pid) {
                    member.general = flag != 0;
                }
            }
            guild::GUILD_INVITE => {
                let guild_id = u32::read(&mut reader)?;
                let name = cstr_to_string(reader.take(guild::GUILD_NAME_LEN)?);
                emit(SocialEvent::GuildInvite { guild_id, name });
            }
            guild::WAR => {
                let war = TPacketGCGuildWar::read(&mut reader)?;
                match war.bWarState {
                    guild::WAR_ON_WAR => {
                        guild_state.wars.insert(war.dwGuildOpp);
                    }
                    guild::WAR_END => {
                        guild_state.wars.remove(&war.dwGuildOpp);
                    }
                    _ => {}
                }
                if matches!(war.bWarState, guild::WAR_SEND_DECLARE | guild::WAR_RECV_DECLARE | guild::WAR_ON_WAR | guild::WAR_END) {
                    emit(SocialEvent::GuildWar {
                        state: war.bWarState,
                        self_guild: war.dwGuildSelf,
                        opponent: war.dwGuildOpp,
                        kind: war.bType,
                    });
                }
            }
            guild::GUILD_NAME => {
                while reader.remaining() > 0 {
                    let id = u32::read(&mut reader)?;
                    let name = cstr_to_string(reader.take(guild::GUILD_NAME_LEN)?);
                    guild_state.names.insert(id, name);
                }
            }
            guild::GUILD_WAR_LIST | guild::GUILD_WAR_END_LIST => {
                // `InsertGVGKey`/`RemoveGVGKey` are `PlayerState`'s (combat).
            }
            guild::WAR_POINT => {
                let point = TPacketGuildWarPoint::read(&mut reader)?;
                emit(SocialEvent::GuildWarPoint {
                    gain: point.dwGainGuildID,
                    opponent: point.dwOpponentGuildID,
                    point: point.lPoint,
                });
            }
            guild::MONEY_CHANGE => guild_state.info.money = u32::read(&mut reader)?,
            _ => {}
        }
        Ok(())
    }
}

/// `GUILD_SUBHEADER_GC_GUILD_WAR_LIST` / `_END_LIST` pairs, for the GVG keys.
pub fn guild_war_pairs(tail: &[u8]) -> Result<Vec<(u32, u32)>, DecodeError> {
    let mut reader = Reader::new(tail);
    let mut pairs = Vec::new();
    while reader.remaining() > 0 {
        pairs.push((u32::read(&mut reader)?, u32::read(&mut reader)?));
    }
    Ok(pairs)
}

fn refine_dialog(pos: u8, kind: u8, table: &wire::TRefineTable) -> RefineDialog {
    let count = usize::from(table.material_count).min(table.materials.len());
    RefineDialog {
        pos,
        result_vnum: table.result_vnum,
        cost: table.cost,
        prob: table.prob,
        kind,
        materials: table.materials[..count].iter().map(|material| (material.vnum, material.count)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::default;

    fn apply(state: &mut SocialState, packet: GcPacket) -> Vec<SocialEvent> {
        let mut events = Vec::new();
        let names = |vid: u32| (vid == 9).then(|| "Trader".to_owned());
        state.apply(&packet, &(1, "Me".to_owned()), &names, &mut |event| events.push(event)).unwrap();
        events
    }

    #[test]
    fn exchange_tracks_both_sides_until_the_end() {
        let mut state = SocialState::default();
        let packet = |subheader, is_me, arg1, cell: u16, arg3| {
            GcPacket::Exchange(wire::TPacketGCExchange {
                subheader,
                is_me,
                arg1,
                arg2: wire::TItemPos { window_type: 1, cell },
                arg3,
                ..default()
            })
        };
        assert_eq!(apply(&mut state, packet(exchange::START, 0, 9, 0, 0)), vec![SocialEvent::StartExchange]);
        assert_eq!((state.exchange.me.name.as_str(), state.exchange.target.name.as_str()), ("Me", "Trader"));
        apply(&mut state, packet(exchange::ITEM_ADD, 1, 27001, 3, 5));
        apply(&mut state, packet(exchange::ELK_ADD, 0, 1000, 0, 0));
        apply(&mut state, packet(exchange::ACCEPT, 0, 1, 0, 0));
        assert_eq!(state.exchange.me.items[3].as_ref().map(|item| (item.vnum, item.count)), Some((27001, 5)));
        assert_eq!((state.exchange.target.elk, state.exchange.target.accept), (1000, true));
        apply(&mut state, packet(exchange::ITEM_DEL, 1, 3, 0, 0));
        assert!(state.exchange.me.items[3].is_none());
        assert_eq!(apply(&mut state, packet(exchange::END, 0, 0, 0, 0)), vec![SocialEvent::EndExchange]);
        assert!(!state.exchange.trading);
    }

    #[test]
    fn guild_list_names_members_and_logins_reach_the_messenger() {
        let mut state = SocialState::default();
        let mut tail = Vec::new();
        for (pid, name) in [(5u32, "Me"), (6, "Ally")] {
            tail.extend(TPacketGCGuildSubMember { pid, byGrade: 2, byNameFlag: 1, ..default() }.encode());
            let mut field = [0u8; guild::NAME_LEN];
            field[..name.len()].copy_from_slice(name.as_bytes());
            tail.extend(field);
        }
        let guild = |subheader, tail: Vec<u8>| GcPacket::Guild {
            head: wire::TPacketGCGuild { header: wire::gc::GUILD, size: 4 + tail.len() as u16, subheader },
            tail,
        };
        apply(&mut state, guild(guild::LIST, tail));
        assert_eq!(state.guild.members[&6].name, "Ally");
        assert_eq!(state.messenger.guild.keys().collect::<Vec<_>>(), vec!["Ally"]);
        apply(&mut state, guild(guild::LOGIN, 6u32.to_le_bytes().to_vec()));
        assert!(state.messenger.guild["Ally"]);
        // Removing the main player destroys the guild.
        assert_eq!(apply(&mut state, guild(guild::REMOVE, 5u32.to_le_bytes().to_vec())), vec![SocialEvent::GuildDeleted]);
        assert!(state.guild.members.is_empty() && state.messenger.guild.is_empty());
    }

    #[test]
    fn messenger_list_reads_length_prefixed_names() {
        let mut state = SocialState::default();
        let tail = [&[1u8, 3][..], b"Bob", &[0, 2], b"Al"].concat();
        let head = wire::TPacketGCMessenger { header: wire::gc::MESSENGER, size: 4 + tail.len() as u16, subheader: messenger::LIST };
        apply(&mut state, GcPacket::Messenger { head, tail });
        assert_eq!(state.messenger.friends.get("Bob"), Some(&true));
        assert_eq!(state.messenger.friends.get("Al"), Some(&false));
    }

    #[test]
    fn shop_signs_appear_and_disappear() {
        let mut state = SocialState::default();
        let mut sign = wire::TPacketGCShopSign { dwVID: 9, ..default() };
        sign.szSign[..4].copy_from_slice(b"Shop");
        apply(&mut state, GcPacket::ShopSign(sign));
        assert_eq!(state.shop_signs[&9], "Shop");
        apply(&mut state, GcPacket::ShopSign(wire::TPacketGCShopSign { dwVID: 9, ..default() }));
        assert!(state.shop_signs.is_empty());
    }

    #[test]
    fn party_invite_needs_the_leader_in_view() {
        let mut state = SocialState::default();
        assert!(apply(&mut state, GcPacket::PartyInvite(wire::TPacketGCPartyInvite { leader_pid: 4, ..default() })).is_empty());
        assert_eq!(
            apply(&mut state, GcPacket::PartyInvite(wire::TPacketGCPartyInvite { leader_pid: 9, ..default() })),
            vec![SocialEvent::PartyInvite { leader: 9, name: "Trader".into() }]
        );
    }
}
