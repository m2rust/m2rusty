//! Phase 0 client: a Bevy window with the C++ Python host running beside it.
//!
//! The interpreter is started on the main thread before Bevy takes over the
//! loop and is finalised after it returns, so it outlives every frame.

use bevy::prelude::*;
use m2_script::PythonHost;

fn main() -> AppExit {
    let python = match PythonHost::start("m2rusty") {
        Ok(host) => host,
        Err(err) => {
            eprintln!("m2-client: python host failed to start: {err}");
            return AppExit::error();
        }
    };
    if let Err(err) = python.run_line("import dbg\ndbg.Tracen('m2-client: python host up')") {
        eprintln!("m2-client: boot line failed: {err}");
        return AppExit::error();
    }

    let exit = App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "m2rusty".into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .run();

    drop(python);
    exit
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Text::new("m2rusty: Bevy + Python 2.7 host"),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
}
