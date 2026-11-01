//! Server-owned player data and UI requests. Widgets never predict item moves.
use std::collections::{BTreeMap, VecDeque};
use bevy::{prelude::*, tasks::{IoTaskPool, Task, futures::check_ready}};
use m2_assets::{M2Packs, paths};
use m2_formats::proto::{ItemData, ItemTables, parse_item_proto};
use m2_net::{phase::{NetEvent, Phase}, wire::{self, GcPacket, TItemPos, TPlayerItemAttribute, TQuickSlot, cg}};
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
#[derive(Resource)]
pub struct PlayerState {
    pub vid: u32, pub name: String, pub race: u16, pub skill_group: u8, pub points: [i32; 255],
    pub items: BTreeMap<ItemPosition, ItemStack>, pub quickslots: [TQuickSlot; 32],
    pub chat: VecDeque<ChatLine>,
}
impl Default for PlayerState {
    fn default() -> Self { Self { vid:0, name:String::new(), points:[0;255], items:BTreeMap::new(), quickslots:[TQuickSlot::default();32], chat:VecDeque::new(), race:0, skill_group:0 } }
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
            NetEvent::MainCharacter(main) => { self.vid=main.vid; self.name=main.name.clone(); self.race=main.race; self.skill_group=main.skill_group; }
            NetEvent::Chat{kind,text,..} if *kind != 5 => self.line(*kind,text.clone()),
            NetEvent::Packet(packet) => match packet {
                GcPacket::PlayerPoints(p) => self.points=p.points,
                GcPacket::PlayerPointChange(p) if p.dwVID==self.vid => self.points[p.Type as usize]=p.value,
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
        Some(WeaponVisual{vnum,model,sub_model,weapon_type:item.table.sub_type})
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
    UseItem(ItemPosition),
    MoveItem {source:ItemPosition,destination:ItemPosition,count:u8},
    BindItem {slot:u8,position:ItemPosition},
    ClearQuickslot(u8),
    SwapQuickslot {source:u8,destination:u8},
}

pub struct PlayerStatePlugin;
impl Plugin for PlayerStatePlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<PlayerState>().init_resource::<ItemCatalog>().add_message::<PlayerAction>()
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
fn receive(mut events:MessageReader<NetMessage>,mut player:ResMut<PlayerState>) {
    for event in events.read(){match event{NetMessage::Game(e)=>player.apply(e),NetMessage::Disconnected{..}=>*player=PlayerState::default(),_=>{}}}
}
fn dispatch(mut actions:MessageReader<PlayerAction>,mut net:ResMut<Net>,mut player:ResMut<PlayerState>) {
    for action in actions.read() {
        if net.game.phase()!=Phase::Game { continue; }
        let ok=match action {
            PlayerAction::Chat{text,kind} => !text.is_empty() && text.len()<512 && !text.chars().any(char::is_control) && matches!(kind,0|3|4|6) && net.game.send_chat(text,*kind),
            PlayerAction::UseItem(pos) => pos.valid() && player.items.contains_key(pos) && net.game.stream.send_packet(&wire::TPacketCGItemUse{header:cg::ITEM_USE,pos:pos.wire()}),
            PlayerAction::MoveItem{source,destination,count} => {
                source!=destination && source.valid() && destination.valid() && player.items.get(source).is_some_and(|i|*count<=i.count) && net.game.stream.send_packet(&wire::TPacketCGItemMove{header:cg::ITEM_MOVE,pos:source.wire(),change_pos:destination.wire(),num:*count})
            }
            PlayerAction::BindItem{slot,position}=>*slot<32 && position.window==1 && position.cell<90 && player.items.contains_key(position) && net.game.stream.send_packet(&wire::TPacketCGQuickSlotAdd{header:cg::QUICKSLOT_ADD,pos:*slot,slot:TQuickSlot{Type:1,Position:position.cell as u8}}),
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
