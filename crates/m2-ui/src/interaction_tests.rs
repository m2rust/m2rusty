//! Run the new UI systems without a renderer to catch ECS access conflicts.
use bevy::{asset::{AssetApp, AssetPlugin}, input_focus::InputFocus, prelude::*};
use m2_game::{combat::SkillState, ingame::{Actors, ClearTarget, GameWorld, PlayerControl, TargetBoard}, net::{Net, NetMessage}, player::{ItemCatalog, PlayerAction, PlayerState}, social::{SocialEvent, SocialState}};
use crate::{core::{Click, Consumed}, hud::ChatState};
#[test]
fn interaction_systems_initialize_without_conflicting_queries() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default())).init_asset::<Image>()
        .init_resource::<ItemCatalog>().init_resource::<PlayerState>().init_resource::<SkillState>()
        .init_resource::<SocialState>().init_resource::<Actors>().init_resource::<PlayerControl>()
        .init_resource::<TargetBoard>().init_resource::<GameWorld>().init_resource::<Net>()
        .init_resource::<InputFocus>().init_resource::<Consumed>().init_resource::<ChatState>()
        .init_resource::<crate::login::LoginScreen>().init_resource::<ButtonInput<KeyCode>>().init_resource::<UiScale>()
        .init_resource::<crate::inventory::InventoryUi>().init_resource::<crate::item_tooltip::LocaleGame>()
        .add_message::<Click>().add_message::<PlayerAction>().add_message::<NetMessage>()
        .add_message::<SocialEvent>().add_message::<ClearTarget>()
        .add_plugins((crate::character::CharacterPlugin, crate::refine::RefinePlugin, crate::social_ui::SocialUiPlugin, crate::exchange::ExchangePlugin, crate::target::TargetBoardPlugin));
    app.configure_sets(Update, (crate::core::UiSet::State, crate::core::UiSet::Actions, crate::core::UiSet::Draw).chain());
    app.update();
}
