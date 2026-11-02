//! Full-screen curtain during server-directed map transfers.

use bevy::prelude::*;
use m2_game::ingame::WarpTransition;

#[derive(Component)]
struct WarpCurtain;

pub struct WarpFadePlugin;

impl Plugin for WarpFadePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn).add_systems(PostUpdate, draw);
    }
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        WarpCurtain,
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::NONE),
        GlobalZIndex(i32::MAX),
        Pickable::IGNORE,
    ));
}

fn draw(transition: Res<WarpTransition>, mut curtain: Query<&mut BackgroundColor, With<WarpCurtain>>) {
    let Ok(mut color) = curtain.single_mut() else { return };
    let next = Color::srgba(0.0, 0.0, 0.0, transition.alpha());
    if color.0 != next {
        color.0 = next;
    }
}
