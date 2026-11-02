//! Native widgets, original artwork, focus and independent floating windows.
use bevy::{input_focus::{InputFocus, tab_navigation::TabNavigationPlugin}, picking::{events::{PointerClick,PointerDrag,PointerPress}, hover::Hovered, pointer::PointerButton}, prelude::*, ui::{ComputedUiRenderTargetInfo,ComputedUiTargetCamera,ContentSize,InteractionDisabled,LayoutConfig,Pressed,widget::NodeImageMode}, ui_widgets::Button, window::PrimaryWindow};
use m2_assets::{paths, subimage::SubImage};
use m2_game::ingame::UiFocus;

#[derive(SystemSet,Debug,Clone,Copy,PartialEq,Eq,Hash)] pub enum UiSet { State, Input, Actions, Draw }
/// Input the UI used this frame. `drag_dropped` suppresses the `Click` that
/// Bevy also emits on the release that ended a slot drag.
#[derive(Resource,Default)] pub struct Consumed {pub keyboard:bool,pub pointer:bool,pub drag_dropped:bool}
#[derive(Component)] pub struct Surface;
#[derive(Component)] pub struct Control;
#[derive(Message,Clone,Copy)] pub struct Click {pub entity:Entity,pub button:PointerButton,pub count:u8}
#[derive(Component,Clone)] pub struct Tooltip(pub String);
#[derive(Component,Clone)] pub enum Art { Image(Handle<Image>), Sub(Handle<SubImage>) }
impl Art {
    pub fn load(assets:&AssetServer,path:&str)->Self {
        let uri=paths::asset_source_uri(path);
        if path.to_ascii_lowercase().ends_with(".sub") {Self::Sub(assets.load(uri))}else{Self::Image(assets.load(uri))}
    }
    fn resolve(&self,subs:&Assets<SubImage>)->Option<(Handle<Image>,Option<Rect>)> {
        match self {Self::Image(image)=>Some((image.clone(),None)),Self::Sub(sub)=>subs.get(sub).map(|s|(s.image.clone(),Some(s.rect)))}
    }
}
#[derive(Component)] pub struct ButtonArt(pub [Art;3]);
#[derive(Component, PartialEq)] pub struct Selected(pub bool);
/// Where an un-moved floating window sticks to the screen.
#[derive(Clone,Copy,PartialEq)]
pub enum Anchor {
    /// `position` is the top-left corner.
    TopLeft,
    /// `position` is the margin from the bottom-right corner.
    BottomRight(Vec2),
    /// `position.x` is the left margin; the window is vertically centred off by `offset`.
    LeftCenter(f32),
}
#[derive(Component)] pub struct FloatingWindow {pub id:&'static str,pub open:bool,pub size:Vec2,pub position:Vec2,pub moved:bool,pub anchor:Anchor}
/// The text of a floating window's title bar.
#[derive(Component)] pub struct WindowTitle(pub Entity);
#[derive(Component)] struct DragWindow(Entity);
#[derive(Component)] struct CloseWindow(Entity);
#[derive(Component)] struct OpenState(bool);
#[derive(Resource,Default)] pub struct WindowManager {pub serial:i32,pub modal:Option<Entity>}

pub struct UiCorePlugin;
impl Plugin for UiCorePlugin {
    fn build(&self,app:&mut App) {
        if !app.is_plugin_added::<TabNavigationPlugin>(){app.add_plugins(TabNavigationPlugin);}
        app.init_resource::<Consumed>().init_resource::<WindowManager>().add_message::<Click>()
            .configure_sets(Update,(UiSet::State,UiSet::Input,UiSet::Actions,UiSet::Draw).chain().before(m2_game::ingame::GameControl))
            .add_systems(First,|mut c:ResMut<Consumed>|*c=Consumed::default())
            .add_observer(pointer_click).add_observer(window_press).add_observer(window_drag)
            .add_systems(Startup,spawn_tooltip)
            .add_systems(Update,(resize_scale,raise_open_windows,close_windows,position_windows,resolve_art,game_focus,tooltip).chain().in_set(UiSet::Draw))
            .configure_sets(PostUpdate,bevy::ui::UiSystems::Layout.run_if(ui_layout_needed));
    }
}
/// Whether any input of `ui_layout_system` changed since it last ran; gates
/// `UiSystems::Layout` (Bevy forbids conditions on the system alone, and the
/// set's other members, the scrollbar and popover widgets, are unused). Bevy runs
/// that system (taffy plus a walk over every node, hidden windows included) every
/// frame; with all game windows built up front that cost most of a millisecond
/// while the UI stood still. These are the components and resources it reads;
/// text and image changes reach it through `ContentSize`. A removal runs it in
/// the same frame, so its own removal readers never miss one.
#[allow(clippy::type_complexity)]
fn ui_layout_needed(
    changed:Query<(),Or<(Changed<Node>,Changed<ContentSize>,Changed<ComputedUiRenderTargetInfo>,Changed<ComputedUiTargetCamera>,Changed<bevy::text::EmSize>,Changed<UiTransform>,Changed<ScrollPosition>,Changed<Outline>,Changed<LayoutConfig>,Changed<ChildOf>,Changed<Children>)>>,
    (mut removed_nodes,mut removed_children,mut removed_parents,mut removed_content):(RemovedComponents<Node>,RemovedComponents<Children>,RemovedComponents<ChildOf>,RemovedComponents<ContentSize>),
    rem:Res<bevy::text::RemSize>,scale:Res<UiScale>,
)->bool{
    let removed=removed_nodes.read().count()+removed_children.read().count()+removed_parents.read().count()+removed_content.read().count();
    removed>0 || rem.is_changed() || scale.is_changed() || !changed.is_empty()
}
/// Edits `node` through a copy and marks it changed only when it differs. Any
/// changed `Node` makes Bevy re-run the whole UI layout, so systems that refresh
/// every frame must not touch nodes whose values stay the same.
pub fn edit_node(node:&mut Mut<Node>,edit:impl FnOnce(&mut Node)){let mut next=node.clone();edit(&mut next);node.set_if_neq(next);}
/// [`edit_node`] for the common case of showing or hiding a node.
pub fn set_display(node:&mut Mut<Node>,display:Display){if node.display!=display{node.display=display;}}
pub fn set_text(text:&mut Mut<Text>,value:String){if text.0!=value{text.0=value;}}
pub fn rect(x:f32,y:f32,w:f32,h:f32)->Node {Node{position_type:PositionType::Absolute,left:Val::Px(x),top:Val::Px(y),width:Val::Px(w),height:Val::Px(h),..default()}}
pub fn picture(commands:&mut Commands,assets:&AssetServer,parent:Entity,path:&str,node:Node)->Entity {
    let id=commands.spawn((node,Art::load(assets,path),ImageNode{image_mode:NodeImageMode::Stretch,..default()},Pickable::IGNORE)).id();commands.entity(parent).add_child(id);id
}
pub fn text(commands:&mut Commands,parent:Entity,value:impl Into<String>,node:Node)->Entity {
    let id=commands.spawn((node,Text::new(value),TextFont{font_size:bevy::text::FontSize::Px(12.),..default()},TextColor(Color::srgb(0.9,0.9,0.9)),TextShadow::default(),Pickable::IGNORE)).id();commands.entity(parent).add_child(id);id
}
pub fn button(commands:&mut Commands,assets:&AssetServer,parent:Entity,node:Node,prefix:&str,caption:&str)->Entity {
    let art=std::array::from_fn(|i|Art::load(assets,&format!("{prefix}_{:02}.sub",i+1)));
    button_with(commands,parent,node,art,caption)
}
/// A three-state button whose artwork is loaded by the caller (the original
/// client also has multi-state `.tga` buttons, not only `.sub` regions).
pub fn button_with(commands:&mut Commands,parent:Entity,node:Node,art:[Art;3],caption:&str)->Entity {
    let id=commands.spawn((Button,Control,Surface,Hovered::default(),node,ImageNode{image_mode:NodeImageMode::Stretch,..default()},ButtonArt(art))).id();
    commands.entity(parent).add_child(id);
    if !caption.is_empty(){let label=text(commands,id,caption,Node{width:Val::Percent(100.),height:Val::Percent(100.),display:Display::Flex,justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()});commands.entity(label).insert(TextLayout::justify(Justify::Center));}
    id
}
pub fn art_button(commands:&mut Commands,assets:&AssetServer,parent:Entity,node:Node,prefix:&str,caption:&str)->Entity {
    let art=std::array::from_fn(|i|Art::load(assets,&format!("{prefix}_{:02}.tga",i+1)));
    button_with(commands,parent,node,art,caption)
}
/// `ui.Board`'s four corners, four tiled edges, and tiled base.
pub fn board(commands:&mut Commands,assets:&AssetServer,parent:Entity,w:f32,h:f32) {
    let c=32.;
    for (part,x,y,width,height,tile) in [
        ("Board_Base",c,c,w-2.*c,h-2.*c,true),
        ("Board_Line_Left",0.,c,c,h-2.*c,true),("Board_Line_Right",w-c,c,c,h-2.*c,true),
        ("Board_Line_Top",c,0.,w-2.*c,c,true),("Board_Line_Bottom",c,h-c,w-2.*c,c,true),
        ("Board_Corner_LeftTop",0.,0.,c,c,false),("Board_Corner_RightTop",w-c,0.,c,c,false),
        ("Board_Corner_LeftBottom",0.,h-c,c,c,false),("Board_Corner_RightBottom",w-c,h-c,c,c,false)] {
        let id=picture(commands,assets,parent,&format!("d:/ymir work/ui/pattern/{part}.tga"),rect(x,y,width.max(0.),height.max(0.)));
        if tile {commands.entity(id).insert(ImageNode{image_mode:NodeImageMode::Tiled{tile_x:true,tile_y:true,stretch_value:1.},..default()});}
    }
}
/// A Metin2 board window with its title bar. When `bottom_right`, `position`
/// is the margin from the bottom-right corner of the screen (the original
/// `x = SCREEN_WIDTH - width - right`, `y = SCREEN_HEIGHT - height - bottom`).
pub fn window(commands:&mut Commands,assets:&AssetServer,id:&'static str,title:&str,size:Vec2,position:Vec2,anchor:Anchor,open:bool)->Entity {
    let start=match anchor{Anchor::TopLeft|Anchor::LeftCenter(_)=>position,Anchor::BottomRight(_)=>Vec2::ZERO};
    let root=commands.spawn((OpenState(false),FloatingWindow{id,open,size,position:start,moved:false,anchor},Surface,Hovered::default(),GlobalZIndex(200),Node{display:if open{Display::Flex}else{Display::None},..rect(position.x,position.y,size.x,size.y)})).id();
    board(commands,assets,root,size.x,size.y);
    let titlebar=commands.spawn((Surface,Hovered::default(),DragWindow(root),rect(8.,7.,size.x-15.,23.))).id();commands.entity(root).add_child(titlebar);
    for (part,x,w) in [("left",0.,32.),("center",32.,size.x-79.),("right",size.x-47.,32.)] {
        let id=picture(commands,assets,titlebar,&format!("d:/ymir work/ui/pattern/titlebar_{part}.tga"),rect(x,0.,w,23.));
        if part=="center"{commands.entity(id).insert(ImageNode{image_mode:NodeImageMode::Tiled{tile_x:true,tile_y:false,stretch_value:1.},..default()});}
    }
    // `TitleName` is centred in the title bar (`x: 77`, centre aligned).
    let title_text=text(commands,titlebar,title,Node{width:Val::Px(size.x-15.),height:Val::Px(23.),display:Display::Flex,justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()});
    commands.entity(title_text).insert((WindowTitle(root),TextLayout::justify(Justify::Center)));
    let close=button(commands,assets,titlebar,rect(size.x-36.,3.,16.,16.),"d:/ymir work/ui/public/close_button","");commands.entity(close).insert(CloseWindow(root));
    root
}
fn pointer_click(mut event:On<PointerClick>,controls:Query<(),With<Control>>,disabled:Query<(),With<InteractionDisabled>>,mut clicks:MessageWriter<Click>,mut consumed:ResMut<Consumed>){
    if controls.contains(event.entity) && !disabled.contains(event.entity) {consumed.pointer=true;clicks.write(Click{entity:event.entity,button:event.button,count:event.count});event.propagate(false);}
}
fn ancestor_window(mut entity:Entity,parents:&Query<&ChildOf>,windows:&Query<&FloatingWindow>)->Option<Entity>{
    loop{if windows.contains(entity){return Some(entity);}entity=parents.get(entity).ok()?.parent();}
}
fn window_press(event:On<PointerPress>,parents:Query<&ChildOf>,windows:Query<&FloatingWindow>,mut zs:Query<&mut GlobalZIndex>,mut manager:ResMut<WindowManager>){
    if let Some(root)=ancestor_window(event.entity,&parents,&windows){if manager.modal.is_some_and(|modal|modal!=root){return;}manager.serial+=1;if let Ok(mut z)=zs.get_mut(root){z.0=200+manager.serial;}}
}
fn window_drag(mut event:On<PointerDrag>,titles:Query<&DragWindow>,mut windows:Query<&mut FloatingWindow>,scale:Res<UiScale>,mut consumed:ResMut<Consumed>){
    if event.button!=PointerButton::Primary{return;}
    if let Ok(title)=titles.get(event.entity){if let Ok(mut window)=windows.get_mut(title.0){window.position+=event.delta/scale.0;window.moved=true;consumed.pointer=true;event.propagate(false);}}
}
fn resize_scale(windows:Query<&Window,With<PrimaryWindow>>,mut scale:ResMut<UiScale>){
    if let Ok(window)=windows.single(){let desired=(window.width()/800.).min(window.height()/600.).min(1.).max(0.25);if (scale.0-desired).abs()>0.001{scale.0=desired;}}
}
fn raise_open_windows(mut manager:ResMut<WindowManager>,mut windows:Query<(&FloatingWindow,&mut OpenState,&mut GlobalZIndex)>){
    for (window,mut state,mut z) in &mut windows{if window.open&&!state.0{manager.serial+=1;z.0=200+manager.serial;}if state.0!=window.open{state.0=window.open;}}
}
fn close_windows(mut clicks:MessageReader<Click>,close:Query<&CloseWindow>,mut windows:Query<(&mut FloatingWindow,&GlobalZIndex)>,keys:Res<ButtonInput<KeyCode>>,mut consumed:ResMut<Consumed>,mut focus:ResMut<InputFocus>,parents:Query<&ChildOf>){
    let mut target=None;
    for click in clicks.read(){if click.button==PointerButton::Primary{if let Ok(close)=close.get(click.entity){target=Some(close.0);}}}
    if keys.just_pressed(KeyCode::Escape) && !consumed.keyboard {
        if focus.get().is_some(){focus.clear();consumed.keyboard=true;}
        else {let max=windows.iter().filter(|(w,_)|w.open).map(|(_,z)|z.0).max();if let Some(max)=max{for (mut w,z) in &mut windows{if z.0==max&&w.open{w.open=false;consumed.keyboard=true;break;}}}}
    }
    if let Some(target)=target{if let Ok((mut w,_))=windows.get_mut(target){w.open=false;}}
    if let Some(mut entity)=focus.get(){loop{if let Ok((w,_))=windows.get(entity){if !w.open{focus.clear();}break;}if let Ok(parent)=parents.get(entity){entity=parent.parent();}else{break;}}}
}
fn position_windows(primary:Query<&Window,With<PrimaryWindow>>,scale:Res<UiScale>,mut windows:Query<(&mut FloatingWindow,&mut Node)>){
    let Ok(primary)=primary.single()else{return;};let viewport=Vec2::new(primary.width(),primary.height())/scale.0;
    for (mut window,mut node) in &mut windows {
        let mut position=window.position;
        if !window.moved {
            match window.anchor {
                Anchor::TopLeft => {}
                Anchor::BottomRight(margin)=>{position=(viewport-window.size-margin).max(Vec2::ZERO);}
                Anchor::LeftCenter(offset)=>{
                    position=Vec2::new(position.x,(viewport.y-window.size.y)/2.+offset).max(Vec2::ZERO);
                }
            }
        }
        position=position.clamp(Vec2::ZERO,(viewport-window.size).max(Vec2::ZERO));
        if window.position!=position{window.position=position;}
        let (left,top,display)=(Val::Px(position.x),Val::Px(position.y),if window.open{Display::Flex}else{Display::None});
        if node.left!=left || node.top!=top || node.display!=display {
            node.left=left;node.top=top;node.display=display;
        }
    }
}
fn resolve_art(subs:Res<Assets<SubImage>>,mut images:Query<(&mut ImageNode,Option<&Art>,Option<&ButtonArt>,Option<&Hovered>,Has<Pressed>,Option<&Selected>,Has<InteractionDisabled>)>){
    for (mut image,art,button,hover,pressed,selected,disabled) in &mut images {
        let art=button.map(|b|&b.0[if !disabled&&(pressed||selected.is_some_and(|s|s.0)){2}else if !disabled&&hover.is_some_and(|h|h.0){1}else{0}]).or(art);
        if let Some((handle,rect))=art.and_then(|a|a.resolve(&subs)){if image.image!=handle || image.rect!=rect{image.image=handle;image.rect=rect;}}
        if button.is_some(){let color=if disabled{Color::srgba(0.45,0.45,0.45,0.8)}else{Color::WHITE};if image.color!=color{image.color=color;}}
    }
}
fn game_focus(input:Res<InputFocus>,fields:Query<(),With<bevy::ui_widgets::TextInput>>,surfaces:Query<&Hovered,With<Surface>>,consumed:Res<Consumed>,manager:Res<WindowManager>,inventory:Option<Res<crate::inventory::InventoryUi>>,mut focus:ResMut<UiFocus>){
    focus.keyboard=consumed.keyboard || input.get().is_some_and(|e|fields.contains(e)) || manager.modal.is_some();
    // `OnMouseLeftButtonDown` with something attached only drops it; the world
    // is not clicked.
    let attached=inventory.is_some_and(|ui|ui.carried.is_some()||ui.held_item().is_some());
    let over_ui=surfaces.iter().any(|h|h.0) || manager.modal.is_some();
    focus.world_hover=attached && !over_ui;
    focus.pointer=consumed.pointer || over_ui || attached;
}
#[derive(Component)] struct CoreTooltip;
#[derive(Component)] struct CoreTooltipText;
fn spawn_tooltip(mut commands:Commands){
    let root=commands.spawn((CoreTooltip,Node{display:Display::None,position_type:PositionType::Absolute,left:Val::Px(0.),top:Val::Px(0.),width:Val::Px(240.),padding:UiRect::all(Val::Px(6.)),border:UiRect::all(Val::Px(1.)),..default()},BackgroundColor(Color::srgba(0.,0.,0.,0.9)),BorderColor::all(Color::srgb(0.7,0.65,0.5)),GlobalZIndex(100_000),Pickable::IGNORE)).id();
    let label=commands.spawn(Node{width:Val::Percent(100.),..default()}).id();
    commands.entity(root).add_child(label);
    // The `Text` itself carries the marker: the label node above only sizes it.
    let text=commands.spawn((CoreTooltipText,Text::new(""),TextFont{font_size:bevy::text::FontSize::Px(12.),..default()},TextColor(Color::srgb(0.95,0.92,0.85)),TextShadow::default())).id();
    commands.entity(label).add_child(text);
}
fn tooltip(primary:Query<&Window,With<PrimaryWindow>>,scale:Res<UiScale>,time:Res<Time>,hovered:Query<(Entity,&Tooltip,&Hovered)>,mut tip:Query<&mut Node,With<CoreTooltip>>,mut labels:Query<&mut Text,With<CoreTooltipText>>,mut last:Local<Option<(Entity,f32)>>){
    let Ok(window)=primary.single()else{return;};
    let current=hovered.iter().find(|(_,tip,h)|h.0&&!tip.0.is_empty());
    let entity=current.map(|(e,..)|e);
    if entity!=last.as_ref().map(|(e,_)|*e){*last=entity.map(|e|(e,time.elapsed_secs()));}
    let ready=last.is_some_and(|(_,at)|time.elapsed_secs()-at>0.3);
    let (Some((_,tooltip,_)),true)=(current,ready) else{
        for mut node in &mut tip{set_display(&mut node,Display::None);}
        return;
    };
    let Some(cursor)=window.cursor_position().map(|p|p/scale.0)else{return;};
    let viewport=Vec2::new(window.width(),window.height())/scale.0;
    for mut node in &mut tip{edit_node(&mut node,|node|{node.display=Display::Flex;node.left=Val::Px((cursor.x+16.).min(viewport.x-240.).max(0.));crate::skills::place_vertically(node,cursor,viewport);});}
    for mut text in &mut labels{if **text!=tooltip.0{**text=tooltip.0.clone();}}
}

#[cfg(test)]
mod input_tests {
    use super::*;
    #[test]
    fn an_attached_item_captures_clicks_but_keeps_world_hover_for_npc_giving() {
        let mut app = App::new();
        app.init_resource::<InputFocus>().init_resource::<Consumed>().init_resource::<WindowManager>().init_resource::<UiFocus>().init_resource::<crate::inventory::InventoryUi>().add_systems(Update,game_focus);
        app.world_mut().resource_mut::<crate::inventory::InventoryUi>().carried = Some(crate::inventory::Carried {kind:2,position:1,from_quick:None});
        app.update(); let focus = app.world().resource::<UiFocus>(); assert!(focus.pointer && focus.world_hover);
        app.world_mut().spawn((Surface,Hovered(true))); app.update(); let focus = app.world().resource::<UiFocus>(); assert!(focus.pointer && !focus.world_hover);
    }
}
