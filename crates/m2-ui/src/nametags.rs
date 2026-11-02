//! Character name tails (`CPythonTextTail::Render`): `Lv N`, the alignment
//! title and the name on one line above each character, the guild name above
//! that. `m2-game` decides which show and where ([`NameTags`]).

use bevy::prelude::*;
use m2_game::ingame::NameTags;

use crate::hud::GameUiRoot;

/// Labels are centred in a box this wide.
const BOX_WIDTH: f32 = 400.0;
/// The name line plus the guild line above it.
const BOX_HEIGHT: f32 = 30.0;
/// `CGraphicTextInstance` default font size.
const FONT_SIZE: f32 = 12.0;
/// `UpdateTextTailLevel`'s level colour.
const LEVEL_RGB: [u8; 3] = [152, 255, 51];

/// One pooled label; `parts` are the guild, level, title and name texts.
#[derive(Component)]
struct NameTagLabel {
    guild: Entity,
    level: Entity,
    title: Entity,
    name: Entity,
}

pub struct NameTagPlugin;

impl Plugin for NameTagPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, draw.after(bevy::transform::TransformSystems::Propagate));
    }
}

fn rgb(c: [u8; 3]) -> Color {
    Color::srgb_u8(c[0], c[1], c[2])
}

fn text(commands: &mut Commands, parent: Entity) -> Entity {
    let id = commands
        .spawn((
            Text::new(""),
            TextFont { font_size: bevy::text::FontSize::Px(FONT_SIZE), ..default() },
            TextColor(Color::WHITE),
            // `SetOutline(true)`: a one-pixel dark rim, not a drop shadow.
            TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::BLACK },
            Node { margin: UiRect::horizontal(Val::Px(2.0)), ..default() },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(id);
    id
}

fn spawn_label(commands: &mut Commands) -> (Entity, NameTagLabel) {
    let root = commands
        .spawn((
            GameUiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(BOX_WIDTH),
                height: Val::Px(BOX_HEIGHT),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
            GlobalZIndex(5),
            Pickable::IGNORE,
        ))
        .id();
    let guild = text(commands, root);
    let row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::FlexEnd, ..default() }, Pickable::IGNORE))
        .id();
    commands.entity(root).add_child(row);
    let level = text(commands, row);
    let title = text(commands, row);
    let name = text(commands, row);
    (root, NameTagLabel { guild, level, title, name })
}

/// Places one label per [`NameTags`] entry, spawning or hiding pooled ones.
#[allow(clippy::type_complexity)]
fn draw(
    mut commands: Commands,
    tags: Option<Res<NameTags>>,
    scale: Res<UiScale>,
    mut labels: Query<(Entity, &NameTagLabel, &mut Node)>,
    mut texts: Query<(&mut Text, &mut TextColor, &mut Node), Without<NameTagLabel>>,
) {
    let tags = tags.map(|tags| tags.tags.clone()).unwrap_or_default();
    let mut pool: Vec<_> = labels.iter_mut().collect();
    pool.sort_by_key(|(entity, ..)| *entity);
    let mut set = |entity: Entity, value: String, color: [u8; 3]| {
        let Ok((mut text, mut text_color, mut node)) = texts.get_mut(entity) else { return };
        let display = if value.is_empty() { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
        if text.0 != value {
            text.0 = value;
        }
        if text_color.0 != rgb(color) {
            text_color.0 = rgb(color);
        }
    };
    for (index, tag) in tags.iter().enumerate() {
        let position = tag.position / scale.0;
        let (left, top) = (Val::Px((position.x - BOX_WIDTH / 2.0).round()), Val::Px((position.y - BOX_HEIGHT).round()));
        let label = match pool.get_mut(index) {
            Some((_, label, node)) => {
                node.display = Display::Flex;
                node.left = left;
                node.top = top;
                NameTagLabel { guild: label.guild, level: label.level, title: label.title, name: label.name }
            }
            None => {
                // A new label: placed and filled from the next frame on.
                let (root, label) = spawn_label(&mut commands);
                commands.entity(root).insert(label);
                continue;
            }
        };
        set(label.guild, tag.guild.clone().unwrap_or_default(), [255, 255, 255]);
        set(label.level, tag.level.map(|level| format!("Lv {level}")).unwrap_or_default(), LEVEL_RGB);
        let (title, title_color) = tag.title.map_or((String::new(), [255, 255, 255]), |(title, color)| (title.to_owned(), color));
        set(label.title, title, title_color);
        set(label.name, tag.name.clone(), tag.color);
    }
    for (_, _, node) in pool.iter_mut().skip(tags.len()) {
        if node.display != Display::None {
            node.display = Display::None;
        }
    }
}
