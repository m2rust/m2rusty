//! Offline visual fixtures: `cargo run -p m2-client --example ui_review -- status /tmp/status.png`.
use bevy::{prelude::*, render::view::screenshot::{Screenshot, save_to_disk}};
use m2_game::{player::{PlayerState, PartyMember, Quest, ItemPosition, ItemStack}, social::{SocialState, RefineDialog}, net::{NetPlugin, NetMessage}};
use m2_ui::{core::{FloatingWindow, UiSet}, character::CharacterTab, login::{LoginScreen, Status}, social_ui::OpenWhisper};
#[derive(Resource)] struct Review { mode: String, path: String, setup: bool, captured: bool }
fn main() {
    let args:Vec<_> = std::env::args().collect();
    let mut app = App::new();
    app.add_plugins(m2_assets::M2AssetsPlugin::new("bin"))
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window:Some(Window { title:"UI review".into(),resolution:(1280,800).into(),..default() }),..default() }))
        .add_plugins((m2_render::M2RenderPlugin,m2_audio::M2AudioPlugin::new(Default::default()),m2_game::M2GamePlugin,NetPlugin,m2_ui::M2UiPlugin))
        .insert_resource(Review {mode:args.get(1).cloned().unwrap_or("status".into()),path:args.get(2).cloned().unwrap_or("/tmp/m2-ui-review.png".into()),setup:false,captured:false})
        .add_systems(Startup, fixture).add_systems(Update, review.after(UiSet::Draw));
    app.run();
}
fn fixture(mut commands:Commands, mut screen:ResMut<LoginScreen>, mut player:ResMut<PlayerState>, mut social:ResMut<SocialState>) {
    commands.spawn(Camera2d);
    screen.status = Status::InGame {name:"Warrior".into(),x:0,y:0};
    player.name = "Warrior".into(); player.vid = 1; player.race = 0;
    for (i,v) in [(1,75),(3,2_000_000_000),(4,2_100_000_000),(5,11800),(6,12400),(7,2100),(8,2600),(12,90),(13,90),(14,40),(15,5),(17,155),(18,22),(19,180),(20,560),(21,100),(22,105),(23,130),(26,3),(28,2),(29,650),(30,720),(95,20),(91,30),(202,20),(203,30),(11,12_000_000)] {player.points[i]=v;}
    player.items.insert(ItemPosition::inventory(4),ItemStack {vnum:19,count:1,flags:0,anti_flags:0,sockets:[0;3],attributes:Default::default()});
    for (id,name) in [(1,"The blacksmith's request"),(2,"Hunt the wolves"),(3,"A letter from the guard"),(4,"The missing merchant"),(5,"A dangerous journey"),(6,"The next adventure")] {
        player.quests.insert(id,Quest {title:name.into(),counter_name:if id==2 {"Wolves".into()}else{String::new()},counter_value:7,clock_name:if id==2 {"Time left".into()}else{String::new()},clock_value:300,clock_updated:Some(std::time::Instant::now()),..default()});
    }
    for (pid,vid,name,state,hp) in [(100,1,"Warrior",1,94),(200,2,"Rina",2,78),(300,0,"Shaman",4,0)] {player.party.insert(pid,PartyMember {pid,vid,name:name.into(),state,percent_hp:hp,..default()});}
    for (name,online) in [("Rina",true),("Shaman",false),("A very long friend name",true)] {social.messenger.friends.insert(name.into(),online);}
}
#[allow(clippy::too_many_arguments)]
fn review(mut commands:Commands,time:Res<Time>,mut state:ResMut<Review>,mut windows:Query<&mut FloatingWindow>,mut tab:ResMut<CharacterTab>,mut social:ResMut<SocialState>,mut whisper:MessageWriter<OpenWhisper>,mut net:MessageWriter<NetMessage>,mut exit:MessageWriter<AppExit>) {
    if !state.setup && time.elapsed_secs()>1. {
        for mut window in &mut windows {
            match (state.mode.as_str(),window.id) {
                ("status"|"quests","character") => {window.open=true;window.position=Vec2::new(280.,140.);window.moved=true;},
                ("social","messenger") => {window.open=true;window.position=Vec2::new(210.,90.);window.moved=true;},
                ("social","whisper") => {window.position=Vec2::new(510.,160.);window.moved=true;},
                _ => {}
            }
        }
        if state.mode=="quests" {*tab=CharacterTab::Quest;}
        if state.mode=="social" {
            net.write(NetMessage::Game(m2_net::phase::NetEvent::Whisper {kind:0,from:"Rina".into(),text:"Are you ready to meet at the blacksmith? Bring your sword and the upgrade materials.".into()}));
            whisper.write(OpenWhisper("Rina".into()));
        }
        if state.mode=="refine" {social.refine=Some(RefineDialog {pos:4,result_vnum:20,cost:120000,prob:60,kind:0,materials:vec![(30001,2),(30004,4),(30015,3)]});}
        if state.mode=="exchange" {social.exchange.trading=true;social.exchange.me.name="Warrior".into();social.exchange.target.name="Rina".into();social.exchange.target.elk=50000;}
        state.setup=true;
    }
    if time.elapsed_secs()>5. && !state.captured {commands.spawn(Screenshot::primary_window()).observe(save_to_disk(state.path.clone()));state.captured=true;}
    if time.elapsed_secs()>6. {exit.write(AppExit::Success);}
}
