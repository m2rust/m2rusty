//! Server-provided blacksmith/scroll quote, matching uiRefine.RefineDialogNew.
use bevy::{picking::pointer::PointerButton, prelude::*};
use m2_game::{player::{ItemCatalog, PlayerAction, PlayerState}, social::{RefineDialog, SocialState}};
use crate::{core::{self, Click, FloatingWindow, UiSet}, hud::GameUiRoot};

#[derive(Component)] struct RefineRoot;
#[derive(Component)] struct Details;
#[derive(Component)] struct Preview;
#[derive(Component, Clone, Copy)] enum Action { Accept, Cancel }
#[derive(Resource, Default)] struct RefineUi { quote: Option<RefineDialog>, warning: bool, was_open: bool }
pub struct RefinePlugin;
impl Plugin for RefinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RefineUi>().add_systems(Update, actions.in_set(UiSet::Actions))
            .add_systems(Update, refresh.in_set(UiSet::Draw));
    }
}
pub fn build(commands: &mut Commands, assets: &AssetServer) {
    commands.insert_resource(RefineUi::default());
    let root = core::window(commands, assets, "refine", "Blacksmith", Vec2::new(300., 350.), Vec2::new(300., 100.), core::Anchor::TopLeft, false);
    commands.entity(root).insert((GameUiRoot, RefineRoot));
    let preview = commands.spawn((Preview, core::rect(134., 35., 32., 96.), ImageNode::default(), Pickable::IGNORE)).id();
    commands.entity(root).add_child(preview);
    let details = core::text(commands, root, "", core::rect(18., 136., 264., 170.));
    commands.entity(details).insert((Details, TextLayout::justify(Justify::Left)));
    for (x, label, action) in [(40., "Refine", Action::Accept), (178., "Cancel", Action::Cancel)] {
        let id = core::button(commands, assets, root, core::rect(x, 315., 82., 23.), "d:/ymir work/ui/public/middle_button", label);
        commands.entity(id).insert(action);
    }
}
pub(crate) fn item_name(catalog: &ItemCatalog, vnum: u32) -> String {
    catalog.get(vnum).map(|d| d.table.locale_name_utf8(1252).into_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("Item {vnum}"))
}
pub(crate) fn icon_path(catalog: &ItemCatalog, vnum: u32) -> String {
    catalog.get(vnum).map(|d| String::from_utf8_lossy(&d.icon_file_name).into_owned()).filter(|p| !p.is_empty()).unwrap_or_else(|| format!("icon/item/{vnum:05}.tga"))
}
fn affordable(quote: &RefineDialog, player: &PlayerState) -> bool {
    player.points[11] >= quote.cost && quote.materials.iter().all(|(vnum, need)| {
        player.items.iter().filter(|(pos, item)| pos.window == 1 && pos.cell < 90 && item.vnum == *vnum).map(|(_, item)| u32::from(item.count)).sum::<u32>() >= *need
    })
}
fn actions(mut clicks: MessageReader<Click>, buttons: Query<&Action>, mut ui: ResMut<RefineUi>, mut social: ResMut<SocialState>, player: Res<PlayerState>, mut windows: Query<&mut FloatingWindow, With<RefineRoot>>, mut requests: MessageWriter<PlayerAction>) {
    for click in clicks.read().filter(|c| c.button == PointerButton::Primary) {
        let Ok(action) = buttons.get(click.entity) else { continue };
        if ui.quote.as_ref() != social.refine.as_ref() { continue; }
        let Some(quote) = social.refine.as_ref() else { continue };
        match action {
            Action::Accept if affordable(quote, &player) && player.items.contains_key(&m2_game::player::ItemPosition::inventory(u16::from(quote.pos))) => {
                if quote.prob < 100 && quote.kind != 5 && !ui.warning { ui.warning = true; continue; }
                requests.write(PlayerAction::Refine { position: quote.pos, kind: quote.kind });
            }
            Action::Cancel => { requests.write(PlayerAction::Refine { position: 255, kind: 255 }); }
            _ => continue,
        }
        social.refine = None; ui.quote = None; ui.was_open = false; ui.warning = false;
        for mut window in &mut windows { window.open = false; }
    }
}
fn refresh(mut commands: Commands, assets: Res<AssetServer>, catalog: Res<ItemCatalog>, player: Res<PlayerState>, mut social: ResMut<SocialState>, mut ui: ResMut<RefineUi>, mut windows: Query<&mut FloatingWindow, With<RefineRoot>>, mut details: Query<&mut Text, With<Details>>, mut preview: Query<(&mut ImageNode, &mut Node), With<Preview>>, buttons: Query<(Entity, &Action)>, mut requests: MessageWriter<PlayerAction>) {
    let Ok(mut window) = windows.single_mut() else { return };
    if ui.quote != social.refine { ui.quote = social.refine.clone(); ui.warning = false; window.open = ui.quote.is_some(); }
    if ui.was_open && !window.open && social.refine.is_some() {
        requests.write(PlayerAction::Refine { position: 255, kind: 255 }); social.refine = None; ui.quote = None;
    }
    ui.was_open = window.open;
    let Some(q) = &ui.quote else { return };
    let available = affordable(q, &player);
    let materials = q.materials.iter().map(|(vnum, count)| format!("{} x{}", item_name(&catalog, *vnum), count)).collect::<Vec<_>>().join("\n");
    let warning = if ui.warning { match q.kind { 2 => "Failure can reduce the item's upgrade level.\nClick Refine again to confirm.", 3 => "Failure can destroy this item.\nClick Refine again to confirm.", _ => "Failure can destroy this item.\nClick Refine again to confirm." } } else if !available { "You need more materials or Yang." } else { "" };
    for mut text in &mut details { core::set_text(&mut text, format!("{}\nSuccess: {}%     Cost: {} Yang\n{}\n{}", item_name(&catalog, q.result_vnum), q.prob, q.cost, materials, warning)); }
    for (mut image, mut node) in &mut preview {
        let handle = assets.load(m2_assets::paths::asset_source_uri(&icon_path(&catalog, q.result_vnum)));
        if image.image != handle { image.image = handle; }
        core::edit_node(&mut node, |n| n.height = Val::Px(catalog.get(q.result_vnum).map_or(1, |d| d.table.size.clamp(1, 3)) as f32 * 32.));
    }
    for (entity, action) in &buttons {
        if matches!(action, Action::Accept) && !available { commands.entity(entity).insert(bevy::ui::InteractionDisabled); } else { commands.entity(entity).remove::<bevy::ui::InteractionDisabled>(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_game::player::{ItemPosition, ItemStack};
    #[test]
    fn refine_requires_current_quote_and_confirms_destruction_then_sends_server_type() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default())).add_message::<Click>().add_message::<PlayerAction>()
            .init_resource::<SocialState>().init_resource::<ItemCatalog>().init_resource::<PlayerState>().add_plugins(RefinePlugin).configure_sets(Update, (UiSet::Actions, UiSet::Draw).chain());
        let root = app.world_mut().spawn((RefineRoot, FloatingWindow { id: "refine", open: false, size: Vec2::ZERO, position: Vec2::ZERO, moved: false, anchor: core::Anchor::TopLeft })).id();
        let accept = app.world_mut().spawn(Action::Accept).id();
        app.world_mut().resource_mut::<PlayerState>().items.insert(ItemPosition::inventory(4), ItemStack { vnum: 10, count: 1, flags: 0, anti_flags: 0, sockets: [0;3], attributes: Default::default() });
        app.world_mut().resource_mut::<SocialState>().refine = Some(RefineDialog { pos:4, result_vnum:11, cost:0, prob:60, kind:2, materials:vec![] });
        let mut cursor = app.world().resource::<Messages<PlayerAction>>().get_cursor();
        app.update(); assert!(app.world().get::<FloatingWindow>(root).unwrap().open);
        app.world_mut().write_message(Click { entity:accept, button:PointerButton::Primary, count:1 }); app.update();
        assert_eq!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).count(), 0);
        // A server requote invalidates the previous confirmation.
        app.world_mut().resource_mut::<SocialState>().refine.as_mut().unwrap().cost = 20;
        app.world_mut().resource_mut::<PlayerState>().points[11] = 100;
        app.world_mut().write_message(Click { entity:accept, button:PointerButton::Primary, count:1 }); app.update();
        assert_eq!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).count(), 0);
        app.world_mut().write_message(Click { entity:accept, button:PointerButton::Primary, count:1 }); app.update();
        assert_eq!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).count(), 0);
        app.world_mut().write_message(Click { entity:accept, button:PointerButton::Primary, count:1 }); app.update();
        let sent: Vec<_> = cursor.read(app.world().resource::<Messages<PlayerAction>>()).collect();
        assert!(matches!(sent.as_slice(), [PlayerAction::Refine { position:4, kind:2 }]));
        assert!(app.world().resource::<SocialState>().refine.is_none());
        assert!(!app.world().get::<FloatingWindow>(root).unwrap().open);
        app.world_mut().resource_mut::<SocialState>().refine = Some(RefineDialog::default()); app.update();
        app.world_mut().get_mut::<FloatingWindow>(root).unwrap().open = false; app.update();
        assert!(matches!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).next(), Some(PlayerAction::Refine {position:255,kind:255})));
    }
}
