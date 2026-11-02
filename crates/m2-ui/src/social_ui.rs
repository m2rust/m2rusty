//! Messenger, private conversations, and party controls from uimessenger/uichat/uiparty.
use std::collections::{BTreeMap, VecDeque};
use bevy::{input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex}, picking::{hover::Hovered, pointer::PointerButton}, prelude::*, text::EditableText, ui_widgets::TextInput};
use m2_game::{combat::SkillState, ingame::{PlayerControl, TargetBoard}, net::{Net, NetMessage}, player::{PlayerAction, PlayerState}, social::{SocialEvent, SocialState}};
use m2_net::phase::NetEvent;
use crate::{core::{self, Click, Consumed, Control, FloatingWindow, Surface, Tooltip, UiSet, WindowTitle}, hud::GameUiRoot};

#[derive(Message)] pub struct OpenWhisper(pub String);
#[derive(Default)] struct Conversation { lines: VecDeque<String>, unread: usize }
impl Conversation { fn push(&mut self, line: String) { self.lines.push_back(line); while self.lines.len() > 300 { self.lines.pop_front(); } } }
#[derive(Clone)] enum Question { Party(u32, String), Friend(String), Request(u32, String) }
impl Question {
    fn prompt(&self) -> String { match self { Self::Party(_, name) => format!("{name} invited you to their party."), Self::Friend(name) => format!("{name} wants to add you as a friend."), Self::Request(_, name) => format!("{name} wants to join your party.") } }
    fn answer(&self, yes: bool) -> PlayerAction { match self {
        Self::Party(leader, _) => PlayerAction::PartyAnswer { leader: *leader, accept: yes },
        Self::Friend(name) => PlayerAction::Chat { kind: 0, text: format!("/messenger_auth {} {name}", if yes {"y"} else {"n"}) },
        Self::Request(vid, _) => PlayerAction::Chat { kind: 0, text: format!("/party_request_{} {vid}", if yes {"accept"} else {"deny"}) },
    } }
}
#[derive(Resource, Default)] struct SocialUi {
    conversations: BTreeMap<String, Conversation>, active: String, page: usize, list: u8, scroll: usize,
    questions: VecDeque<Question>, question_open: bool, member: u32,
}
#[derive(Component, Clone, Copy)] enum Action { Tab(u8), Row(usize), Remove(usize), Page(i32), Add, NewWhisper, Send, History(i32), Accept, Decline, Member(usize), Settings, Leave, Distribution, Role(u8), Kick }
#[derive(Component, Clone, Copy, PartialEq, Eq)] enum Field { Name, Message }
#[derive(Component)] struct FriendLabel(usize);
#[derive(Component)] struct FriendRow(usize);
#[derive(Component)] struct FriendIcon {row:usize, online:Option<bool>}
#[derive(Component)] struct PartyRoleIcon {row:usize, state:u8}
#[derive(Component)] struct History;
#[derive(Component)] struct QuestionText;
#[derive(Component)] struct PartyRoot;
#[derive(Component)] struct PartyRow(usize);
#[derive(Component)] struct PartyLabel(usize);
#[derive(Component)] struct PartyHealth(usize);
#[derive(Component)] struct PartyInfo;
#[derive(Component)] struct Unread;
#[derive(Component)] struct PageLabel;

pub struct SocialUiPlugin;
impl Plugin for SocialUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SocialUi>().add_message::<OpenWhisper>()
            .add_systems(Update, receive.in_set(UiSet::State))
            .add_systems(Update, (actions, party_actions).in_set(UiSet::Actions))
            .add_systems(Update, (refresh, refresh_party).in_set(UiSet::Draw));
    }
}
pub(crate) fn input(commands: &mut Commands, parent: Entity, node: Node, max: usize) -> Entity {
    // Click-to-focus (`PointerFocusPlugin`) only focuses entities with a
    // `TabIndex`; without one a click on the field blurs it instead.
    let id = commands.spawn((TextInput, TabIndex(0), Surface, Hovered::default(), node,
        EditableText { max_characters: Some(max), ..EditableText::new("") },
        TextFont { font_size: bevy::text::FontSize::Px(12.), ..default() }, TextColor(Color::WHITE), TextLayout::no_wrap(),
        BackgroundColor(Color::srgba(0., 0., 0., 0.55)))).id();
    commands.entity(parent).add_child(id); id
}
fn button(commands: &mut Commands, assets: &AssetServer, root: Entity, rect: Node, caption: &str, action: Action) -> Entity {
    let id = core::button(commands, assets, root, rect, "d:/ymir work/ui/public/small_thin_button", caption); commands.entity(id).insert(action); id
}
pub fn build(commands: &mut Commands, assets: &AssetServer) {
    commands.insert_resource(SocialUi::default());
    let root = core::window(commands, assets, "messenger", "Friends", Vec2::new(270., 410.), Vec2::new(500., 110.), core::Anchor::TopLeft, false);
    commands.entity(root).insert(GameUiRoot);
    for (i, caption) in ["Friends", "Guild", "Messages"].into_iter().enumerate() { button(commands, assets, root, core::rect(12. + i as f32 * 82., 33., 80., 22.), caption, Action::Tab(i as u8)); }
    for row in 0..10 {
        let entry = commands.spawn((FriendRow(row), core::rect(12., 62. + row as f32 * 26., 244., 25.))).id(); commands.entity(root).add_child(entry);
        let hit = commands.spawn((Action::Row(row), Control, Surface, Hovered::default(), core::rect(0., 0., 210., 24.))).id(); commands.entity(entry).add_child(hit);
        let icon = core::picture(commands, assets, hit, "d:/ymir work/ui/game/windows/messenger_list_offline.sub", core::rect(0., 3., 16., 16.));
        commands.entity(icon).insert(FriendIcon {row, online:None});
        let text = core::text(commands, hit, "", core::rect(20., 2., 185., 20.)); commands.entity(text).insert((FriendLabel(row), TextLayout::no_wrap()));
        button(commands, assets, entry, core::rect(215., 0., 25., 22.), "-", Action::Remove(row));
    }
    button(commands, assets, root, core::rect(12., 326., 36., 22.), "<", Action::Page(-1));
    button(commands, assets, root, core::rect(222., 326., 36., 22.), ">", Action::Page(1));
    let page = core::text(commands, root, "", core::rect(52., 328., 164., 18.)); commands.entity(page).insert(PageLabel);
    let name = input(commands, root, core::rect(14., 356., 240., 18.), 24); commands.entity(name).insert(Field::Name);
    button(commands, assets, root, core::rect(20., 380., 108., 22.), "Add friend", Action::Add);
    button(commands, assets, root, core::rect(142., 380., 108., 22.), "Whisper", Action::NewWhisper);

    let root = core::window(commands, assets, "whisper", "Whisper", Vec2::new(440., 330.), Vec2::new(290., 260.), core::Anchor::TopLeft, false);
    commands.entity(root).insert(GameUiRoot);
    let history = core::text(commands, root, "", core::rect(16., 36., 385., 228.)); commands.entity(history).insert((History, TextLayout::no_wrap(), TextFont { font_size: bevy::text::FontSize::Px(12.), ..default() }));
    button(commands, assets, root, core::rect(405., 38., 22., 22.), "^", Action::History(1));
    button(commands, assets, root, core::rect(405., 238., 22., 22.), "v", Action::History(-1));
    let message = input(commands, root, core::rect(16., 288., 318., 22.), 254); commands.entity(message).insert(Field::Message);
    button(commands, assets, root, core::rect(348., 286., 74., 24.), "Send", Action::Send);

    let root = core::window(commands, assets, "social_question", "Invitation", Vec2::new(340., 135.), Vec2::new(280., 210.), core::Anchor::TopLeft, false);
    commands.entity(root).insert(GameUiRoot);
    let text = core::text(commands, root, "", core::rect(18., 38., 304., 43.)); commands.entity(text).insert(QuestionText);
    button(commands, assets, root, core::rect(50., 93., 88., 24.), "Accept", Action::Accept);
    button(commands, assets, root, core::rect(200., 93., 88., 24.), "Decline", Action::Decline);

    let party = commands.spawn((PartyRoot, GameUiRoot, Surface, Hovered::default(), GlobalZIndex(130), core::rect(10., 120., 180., 365.))).id();
    crate::target::thin_board(commands, assets, party);
    button(commands, assets, party, core::rect(12., 9., 156., 22.), "Party settings", Action::Settings);
    for row in 0..8 {
        let entry = commands.spawn((PartyRow(row), Action::Member(row), Control, Surface, Hovered::default(), Tooltip(String::new()), core::rect(10., 38. + row as f32 * 38., 160., 35.))).id(); commands.entity(party).add_child(entry);
        let icon = core::picture(commands, assets, entry, "d:/ymir work/ui/game/windows/party_state_normal_01.sub", core::rect(0., 0., 16., 16.));
        commands.entity(icon).insert(PartyRoleIcon {row, state:255});
        let text = core::text(commands, entry, "", core::rect(20., 0., 140., 18.)); commands.entity(text).insert((PartyLabel(row), TextLayout::justify(Justify::Left)));
        let hp = commands.spawn((PartyHealth(row), core::rect(0., 22., 160., 5.), BackgroundColor(Color::srgb(0.7, 0.06, 0.06)), Pickable::IGNORE)).id(); commands.entity(entry).add_child(hp);
    }
    let root = core::window(commands, assets, "party_settings", "Party", Vec2::new(320., 225.), Vec2::new(200., 120.), core::Anchor::TopLeft, false); commands.entity(root).insert(GameUiRoot);
    let info = core::text(commands, root, "", core::rect(12., 34., 296., 38.)); commands.entity(info).insert(PartyInfo);
    for (i, (role, label)) in [(0, "Normal"), (2, "Attacker"), (6, "Berserker"), (3, "Tanker"), (4, "Buffer"), (5, "Skill master"), (7, "Defender")].into_iter().enumerate() {
        button(commands, assets, root, core::rect(12. + (i % 3) as f32 * 100., 79. + (i / 3) as f32 * 29., 96., 24.), label, Action::Role(role));
    }
    button(commands, assets, root, core::rect(112., 137., 96., 24.), "Kick", Action::Kick);
    button(commands, assets, root, core::rect(12., 181., 140., 24.), "EXP mode", Action::Distribution);
    button(commands, assets, root, core::rect(168., 181., 140., 24.), "Leave party", Action::Leave);
    let unread = commands.spawn((Unread, GameUiRoot, Action::NewWhisper, Control, Surface, Hovered::default(), Node { display: Display::None, position_type: PositionType::Absolute, right: Val::Px(18.), bottom: Val::Px(160.), width: Val::Px(220.), height: Val::Px(26.), ..default() }, BackgroundColor(Color::srgba(0.12, 0.08, 0.02, 0.85)), GlobalZIndex(160))).id();
    core::text(commands, unread, "New private message", core::rect(0., 4., 220., 20.));
}
fn list_entries(ui: &SocialUi, social: &SocialState) -> Vec<(String, bool, usize)> {
    let names: Vec<_> = match ui.list { 1 => social.messenger.guild.iter().map(|(n, on)| (n.clone(), *on)).collect(), 2 => ui.conversations.keys().map(|n| (n.clone(), true)).collect(), _ => social.messenger.friends.iter().map(|(n, on)| (n.clone(), *on)).collect() };
    names.into_iter().map(|(n, on)| { let unread = ui.conversations.get(&n).map_or(0, |c| c.unread); (n, on, unread) }).collect()
}
fn receive(mut messages: MessageReader<NetMessage>, mut events: MessageReader<SocialEvent>, mut ui: ResMut<SocialUi>, windows: Query<&FloatingWindow>, actors: Res<m2_game::ingame::Actors>) {
    for event in events.read() { if let SocialEvent::PartyInvite { leader, name } = event { ui.questions.push_back(Question::Party(*leader, name.clone())); } }
    for message in messages.read() {
        match message {
            NetMessage::Disconnected { .. } | NetMessage::Game(NetEvent::MainCharacter(_)) => *ui = SocialUi::default(),
            NetMessage::Game(NetEvent::Whisper { kind, from, text }) => {
                let reading = ui.active == *from && windows.iter().any(|w| w.id == "whisper" && w.open);
                let line = match kind { 0 => format!("{from}: {text}"), 1 => format!("{from} is offline."), 2 => format!("{from} is blocking whispers."), 3 => "You are blocking whispers.".into(), 4 => if text.is_empty() {"The message could not be delivered.".into()} else {text.clone()}, 5 => format!("[GM] {from}: {text}"), _ => text.clone() };
                let chat = ui.conversations.entry(from.clone()).or_default(); chat.push(line); if !reading { chat.unread += 1; }
            }
            NetMessage::Game(NetEvent::Chat { kind: 5, text, .. }) => {
                let mut words = text.split_whitespace();
                match words.next() {
                    Some("messenger_auth") => if let Some(name) = words.next() { ui.questions.push_back(Question::Friend(name.into())); },
                    Some("PartyRequest") => if let Some(vid) = words.next().and_then(|v| v.parse::<u32>().ok()) {
                        let name = actors.by_vid.get(&vid).map_or_else(|| format!("Player {vid}"), |a| a.name.clone()); ui.questions.push_back(Question::Request(vid, name));
                    },
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn actions(mut clicks: MessageReader<Click>, buttons: Query<&Action>, mut opens: MessageReader<OpenWhisper>, mut ui: ResMut<SocialUi>, mut social: ResMut<SocialState>, mut windows: Query<&mut FloatingWindow>, mut fields: Query<(Entity, &Field, &mut EditableText)>, keys: Res<ButtonInput<KeyCode>>, mut focus: ResMut<InputFocus>, mut consumed: ResMut<Consumed>, mut requests: MessageWriter<PlayerAction>, player: Res<PlayerState>) {
    let mut pending: Vec<_> = clicks.read().filter(|c| c.button == PointerButton::Primary).filter_map(|c| buttons.get(c.entity).ok().copied()).collect();
    if keys.just_pressed(KeyCode::Enter) && fields.iter().any(|(e, f, _)| *f == Field::Message && focus.get() == Some(e)) { pending.push(Action::Send); consumed.keyboard = true; }
    let mut open = opens.read().last().map(|o| o.0.clone());
    for action in pending {
        let name = || fields.iter().find(|(_, f, _)| **f == Field::Name).map_or(String::new(), |(_, _, e)| e.value().to_string().trim().to_owned());
        match action {
            Action::Tab(tab) => { ui.list = tab; ui.page = 0; }
            Action::Page(delta) => ui.page = ui.page.saturating_add_signed(delta as isize).min(list_entries(&ui, &social).len().saturating_sub(1) / 10),
            Action::Row(row) => { if let Some((name, _, _)) = list_entries(&ui, &social).get(ui.page * 10 + row) { open = Some(name.clone()); } }
            Action::Remove(row) if ui.list == 0 => { if let Some((name, _, _)) = list_entries(&ui, &social).get(ui.page * 10 + row) { requests.write(PlayerAction::FriendRemove(name.clone())); social.messenger.friends.remove(name); } }
            Action::Add => { let name = name(); if !name.is_empty() && !name.chars().any(char::is_whitespace) && name != player.name { requests.write(PlayerAction::FriendAdd(name)); } }
            Action::NewWhisper => {
                open = ui.conversations.iter().find(|(_, c)| c.unread > 0).map(|(n, _)| n.clone()).or_else(|| { let n = name(); (!n.is_empty()).then_some(n) });
            }
            Action::Send if !ui.active.is_empty() && windows.iter().any(|w| w.id == "whisper" && w.open) => {
                for (_, field, mut edit) in &mut fields { if *field == Field::Message {
                    let text = edit.value().to_string().trim().to_owned();
                    if !text.is_empty() && m2_formats::utf8::encode(1252, &text).len() < 255 && !text.chars().any(char::is_control) {
                        requests.write(PlayerAction::Whisper { to: ui.active.clone(), text: text.clone() });
                        let active = ui.active.clone(); ui.conversations.entry(active).or_default().push(format!("{}: {text}", player.name)); ui.scroll = 0; edit.clear();
                    }
                } }
            }
            Action::History(delta) => ui.scroll = ui.scroll.saturating_add_signed(delta as isize * 6).min(ui.conversations.get(&ui.active).map_or(0, |c| wrapped_history(c).len().saturating_sub(1))),
            Action::Accept | Action::Decline => {
                if let Some(question) = ui.questions.pop_front() { requests.write(question.answer(matches!(action, Action::Accept))); }
                ui.question_open = false; for mut w in &mut windows { if w.id == "social_question" { w.open = false; } }
            }
            _ => {}
        }
    }
    if let Some(name) = open.filter(|n| !n.is_empty() && n != &player.name) {
        ui.active = name.clone(); ui.scroll = 0; ui.conversations.entry(name).or_default().unread = 0;
        for mut window in &mut windows { if window.id == "whisper" { window.open = true; } }
        for (entity, field, _) in &fields { if *field == Field::Message { focus.set(entity, FocusCause::Navigated); } }
    }
}
fn party_skill_level(skills: &SkillState) -> u8 { skills.level(121).saturating_add(skills.grade(121).saturating_mul(20)) }
fn role_required(role: u8) -> u8 { match role { 2 => 10, 3 | 6 => 20, 4 => 25, 5 => 35, 7 => 40, _ => 0 } }
fn role_name(role: u8) -> &'static str { match role { 1 => "Leader", 2 => "Attacker", 3 => "Tanker", 4 => "Buffer", 5 => "Skill master", 6 => "Berserker", 7 => "Defender", _ => "Member" } }
fn party_actions(mut clicks: MessageReader<Click>, buttons: Query<&Action>, mut ui: ResMut<SocialUi>, player: Res<PlayerState>, social: Res<SocialState>, skills: Res<SkillState>, mut windows: Query<&mut FloatingWindow>, mut requests: MessageWriter<PlayerAction>, mut control: ResMut<PlayerControl>, mut board: ResMut<TargetBoard>, mut net: ResMut<Net>) {
    let me = player.party_self();
    let leader = me.is_some_and(|m| m.state == 1);
    let members: Vec<_> = player.party.values().collect();
    for click in clicks.read().filter(|c| c.button == PointerButton::Primary) {
        let Ok(action) = buttons.get(click.entity) else { continue };
        match *action {
            Action::Member(row) => if let Some(member) = members.get(row) {
                ui.member = member.pid;
                if member.vid != 0 && member.vid != player.vid { control.target = Some(member.vid); *board = TargetBoard { vid: member.vid, name: member.name.clone(), hp_percent: None, visible: true }; net.send_target(member.vid); }
                for mut w in &mut windows { if w.id == "party_settings" { w.open = true; } }
            },
            Action::Settings => { for mut w in &mut windows { if w.id == "party_settings" { w.open = !w.open; } } }
            Action::Leave => if let Some(me) = me { requests.write(PlayerAction::PartyRemove(me.pid)); },
            Action::Distribution if leader => { requests.write(PlayerAction::PartyDistribution(1 - social.party_distribute_mode.min(1))); }
            Action::Kick if leader => if let Some(member) = player.party.get(&ui.member).filter(|m| m.state != 1) { requests.write(PlayerAction::PartyRemove(member.pid)); },
            Action::Role(role) if leader && party_skill_level(&skills) >= role_required(role) => if let Some(member) = player.party.get(&ui.member).filter(|m| m.state != 1) {
                if member.state > 1 { requests.write(PlayerAction::PartyState { pid: member.pid, role: member.state, enabled: false }); }
                if role > 0 && role != member.state { requests.write(PlayerAction::PartyState { pid: member.pid, role, enabled: true }); }
            },
            _ => {}
        }
    }
}
fn wrapped_history(chat: &Conversation) -> Vec<String> {
    let mut lines = Vec::new();
    for message in &chat.lines {
        let mut line = String::new(); let mut count = 0;
        for word in message.split_whitespace() {
            let chars:Vec<_> = word.chars().collect();
            if count > 0 && count + 1 + chars.len() > 52 { lines.push(std::mem::take(&mut line)); count = 0; }
            if count > 0 { line.push(' '); count += 1; }
            for ch in chars { if count == 52 { lines.push(std::mem::take(&mut line)); count = 0; } line.push(ch); count += 1; }
        }
        lines.push(line);
    } lines
}
fn history_lines(chat: &Conversation, scroll: usize) -> String {
    let lines = wrapped_history(chat);
    let end = lines.len().saturating_sub(scroll); lines[end.saturating_sub(14)..end].join("\n")
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh(mut commands:Commands, assets:Res<AssetServer>, mut friend_icons:Query<(Entity, &mut FriendIcon)>, mut ui: ResMut<SocialUi>, social: Res<SocialState>, time: Res<Time>, mut windows: Query<(Entity, &mut FloatingWindow)>, mut texts: Query<(&mut Text, Option<&FriendLabel>, Has<History>, Has<QuestionText>, Has<PageLabel>, Option<&WindowTitle>)>, mut rows: Query<(&FriendRow, &mut Node)>, mut controls: Query<(&Action, &mut Node, Has<Unread>), Without<FriendRow>>, mut requests: MessageWriter<PlayerAction>) {
    // Window close/Escape declines the pending question, as the Python dialog does.
    for (_, mut window) in &mut windows {
        if window.id == "social_question" {
            if ui.question_open && !window.open { if let Some(q) = ui.questions.pop_front() { requests.write(q.answer(false)); } ui.question_open = false; }
            if !ui.question_open && !ui.questions.is_empty() { window.open = true; ui.question_open = true; }
            if ui.questions.is_empty() { window.open = false; }
        }
        if window.id == "whisper" && window.open { let active = ui.active.clone(); if let Some(c) = ui.conversations.get_mut(&active) { c.unread = 0; } }
    }
    let entries = list_entries(&ui, &social);
    let page = ui.page.min(entries.len().saturating_sub(1) / 10);
    if ui.page != page { ui.page = page; }
    for (row, mut node) in &mut rows { core::set_display(&mut node, if entries.get(ui.page * 10 + row.0).is_some() { Display::Flex } else { Display::None }); }
    for (action, mut node, _) in &mut controls {
        if matches!(action, Action::Remove(_)) { core::set_display(&mut node, if ui.list == 0 { Display::Flex } else { Display::None }); }
    }
    for (entity, mut icon) in &mut friend_icons {
        if let Some((_, online, _)) = entries.get(ui.page * 10 + icon.row) {
            if icon.online != Some(*online) { icon.online = Some(*online); commands.entity(entity).insert(core::Art::load(&assets, if *online {"d:/ymir work/ui/game/windows/messenger_list_online.sub"} else {"d:/ymir work/ui/game/windows/messenger_list_offline.sub"})); }
        }
    }
    let whisper = windows.iter().find(|(_, w)| w.id == "whisper").map(|(id, _)| id);
    for (mut text, friend, history, question, page, title) in &mut texts {
        let value = if let Some(row) = friend {
            entries.get(ui.page * 10 + row.0).map(|(name, _online, unread)| format!("{}{}", name, if *unread > 0 { format!(" ({unread})") } else { String::new() }))
        } else if history { Some(ui.conversations.get(&ui.active).map_or(String::new(), |c| history_lines(c, ui.scroll)))
        } else if question { Some(ui.questions.front().map_or(String::new(), Question::prompt))
        } else if page { Some(format!("{} / {}", ui.page + 1, entries.len().div_ceil(10).max(1)))
        } else if title.is_some_and(|t| Some(t.0) == whisper) { Some(format!("Whisper: {}", ui.active)) } else { None };
        if let Some(value) = value { if text.0 != value { text.0 = value; } }
    }
    for (_, mut node, unread) in &mut controls {
        if unread { core::set_display(&mut node, if ui.conversations.values().any(|c| c.unread > 0) && (time.elapsed_secs() * 2.).floor() as i32 % 2 == 0 { Display::Flex } else { Display::None }); }
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_party(mut commands: Commands, assets:Res<AssetServer>, mut role_icons:Query<(Entity,&mut PartyRoleIcon)>, ui: Res<SocialUi>, player: Res<PlayerState>, social: Res<SocialState>, skills: Res<SkillState>, mut root: Query<&mut Node, (With<PartyRoot>, Without<PartyRow>, Without<PartyHealth>)>, mut rows: Query<(&PartyRow, &mut Node, &mut Tooltip), Without<PartyHealth>>, mut health: Query<(&PartyHealth, &mut Node), (Without<PartyRow>, Without<PartyRoot>)>, mut texts: Query<(&mut Text, Option<&PartyLabel>, Has<PartyInfo>)>, buttons: Query<(Entity, &Action)>, mut windows: Query<&mut FloatingWindow>) {
    let members: Vec<_> = player.party.values().collect();
    let leader = player.party_self().is_some_and(|m| m.state == 1);
    if let Ok(mut node) = root.single_mut() { core::set_display(&mut node, if members.is_empty() { Display::None } else { Display::Flex }); core::edit_node(&mut node, |n| n.height = Val::Px(48. + members.len().min(8) as f32 * 38.)); }
    if members.is_empty() { for mut w in &mut windows { if w.id == "party_settings" { w.open = false; } } }
    for (row, mut node, mut tooltip) in &mut rows {
        let member = members.get(row.0); core::set_display(&mut node, if member.is_some() { Display::Flex } else { Display::None });
        tooltip.0 = member.map_or(String::new(), |m| format!("{}\n{}\nParty bonuses: {:?}", m.name, role_name(m.state), m.affects));
    }
    for (entity, mut icon) in &mut role_icons {
        if let Some(member) = members.get(icon.row) {
            if icon.state != member.state {
                icon.state = member.state;
                let name = match member.state {1=>"leader",2=>"attacker",3=>"tanker",4=>"buffer",5=>"skill_master",6=>"berserker",7=>"defender",_=>"normal"};
                commands.entity(entity).insert(core::Art::load(&assets, &format!("d:/ymir work/ui/game/windows/party_state_{name}_01.sub")));
            }
        }
    }
    for (row, mut node) in &mut health { core::edit_node(&mut node, |n| n.width = Val::Px(members.get(row.0).filter(|m| m.vid != 0).map_or(0., |m| 160. * f32::from(m.percent_hp.min(100)) / 100.))); }
    for (mut text, row, info) in &mut texts {
        if let Some(row) = row { core::set_text(&mut text, members.get(row.0).map_or(String::new(), |m| format!("{}{}", m.name, if m.vid == 0 {" (away)"} else {""}))); }
        else if info { core::set_text(&mut text, format!("{}\nEXP: {}", player.party.get(&ui.member).map_or("Select a party member", |m| m.name.as_str()), if social.party_distribute_mode == 0 {"By level"} else {"Equal distribution"})); }
    }
    let member = player.party.get(&ui.member).filter(|m| m.state != 1);
    for (entity, action) in &buttons {
        let enabled = match *action { Action::Role(role) => leader && member.is_some() && party_skill_level(&skills) >= role_required(role), Action::Kick => leader && member.is_some(), Action::Distribution => leader, _ => continue };
        if enabled { commands.entity(entity).remove::<bevy::ui::InteractionDisabled>(); } else { commands.entity(entity).insert(bevy::ui::InteractionDisabled); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default())).add_message::<Click>().add_message::<PlayerAction>().add_message::<NetMessage>().add_message::<SocialEvent>()
            .init_resource::<SocialState>().init_resource::<PlayerState>().init_resource::<m2_game::ingame::Actors>().init_resource::<PlayerControl>()
            .init_resource::<TargetBoard>().init_resource::<Net>().init_resource::<SkillState>().init_resource::<InputFocus>()
            .init_resource::<Consumed>().init_resource::<ButtonInput<KeyCode>>().add_plugins(SocialUiPlugin)
            .configure_sets(Update, (UiSet::State, UiSet::Actions, UiSet::Draw).chain());
        for id in ["whisper", "social_question", "party_settings"] {
            app.world_mut().spawn(FloatingWindow { id, open:false, size:Vec2::ZERO, position:Vec2::ZERO, moved:false, anchor:core::Anchor::TopLeft });
        }
        app
    }
    #[test]
    fn a_clicked_text_field_takes_the_keyboard_instead_of_blurring() {
        let mut app = App::new();
        app.add_plugins(bevy::input_focus::InputFocusPlugin).add_observer(bevy::input_focus::tab_navigation::acquire_focus_tab_index);
        let window = app.world_mut().spawn(Window::default()).id();
        let panel = app.world_mut().spawn((Node::default(), ChildOf(window))).id();
        let field = input(&mut app.world_mut().commands(), panel, Node::default(), 24);
        app.world_mut().flush();
        // What `PointerFocusPlugin::click_to_focus` triggers on a press.
        app.world_mut().trigger(bevy::input_focus::AcquireFocus { focused_entity: field, window });
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    }
    #[test]
    fn conversations_keep_history_and_unread_and_send_only_to_selected_recipient() {
        let mut app = app();
        let send = app.world_mut().spawn(Action::Send).id();
        let field = app.world_mut().spawn((Field::Message, EditableText::new(""))).id();
        let mut cursor = app.world().resource::<Messages<PlayerAction>>().get_cursor();
        app.world_mut().write_message(NetMessage::Game(NetEvent::Whisper {kind:0,from:"Ally".into(),text:"Hello".into()})); app.update();
        assert_eq!(app.world().resource::<SocialUi>().conversations["Ally"].unread, 1);
        app.world_mut().write_message(OpenWhisper("Ally".into())); app.update();
        assert_eq!(app.world().resource::<SocialUi>().conversations["Ally"].unread, 0);
        app.world_mut().get_mut::<EditableText>(field).unwrap().editor.set_text("Hi there");
        app.world_mut().write_message(Click {entity:send,button:PointerButton::Primary,count:1}); app.update();
        let sent:Vec<_> = cursor.read(app.world().resource::<Messages<PlayerAction>>()).collect();
        assert!(matches!(sent.as_slice(), [PlayerAction::Whisper{to,text}] if to == "Ally" && text == "Hi there"));
        assert!(app.world().get::<EditableText>(field).unwrap().value().to_string().is_empty());
        assert!(app.world().resource::<SocialUi>().conversations["Ally"].lines.back().unwrap().contains("Hi there"));
        app.world_mut().write_message(NetMessage::Game(NetEvent::Whisper {kind:1,from:"Other".into(),text:String::new()})); app.update();
        let ui = app.world().resource::<SocialUi>(); assert_eq!(ui.active, "Ally"); assert_eq!(ui.conversations["Other"].unread, 1); assert!(ui.conversations["Other"].lines[0].contains("offline"));
    }
    #[test]
    fn invitations_answer_original_ids_and_party_controls_use_pids_and_mastered_leadership() {
        let mut app = app();
        let accept = app.world_mut().spawn(Action::Accept).id();
        let role = app.world_mut().spawn(Action::Role(7)).id();
        let kick = app.world_mut().spawn(Action::Kick).id();
        let mut cursor = app.world().resource::<Messages<PlayerAction>>().get_cursor();
        app.world_mut().write_message(SocialEvent::PartyInvite{leader:19,name:"Leader".into()}); app.update();
        app.world_mut().write_message(Click{entity:accept,button:PointerButton::Primary,count:1}); app.update();
        assert!(matches!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).next(), Some(PlayerAction::PartyAnswer{leader:19,accept:true})));
        {
            let mut player = app.world_mut().resource_mut::<PlayerState>(); player.vid = 1; player.name = "Me".into();
            player.party.insert(100, m2_game::player::PartyMember {pid:100,vid:1,name:"Me".into(),state:1,..default()});
            player.party.insert(200, m2_game::player::PartyMember {pid:200,vid:2,name:"Ally".into(),state:2,..default()});
        }
        app.world_mut().resource_mut::<SocialUi>().member = 200;
        app.world_mut().resource_mut::<SkillState>().grades[121] = 2;
        app.world_mut().resource_mut::<SkillState>().levels[121] = 30;
        app.world_mut().write_message(Click{entity:role,button:PointerButton::Primary,count:1}); app.update();
        let sent:Vec<_> = cursor.read(app.world().resource::<Messages<PlayerAction>>()).collect();
        assert!(matches!(sent.as_slice(), [PlayerAction::PartyState{pid:200,role:2,enabled:false}, PlayerAction::PartyState{pid:200,role:7,enabled:true}]));
        app.world_mut().write_message(Click{entity:kick,button:PointerButton::Primary,count:1}); app.update();
        assert!(matches!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).next(), Some(PlayerAction::PartyRemove(200))));
    }
}
