//! Native frontend. Login geometry/art follows `locale/en/ui/loginwindow.py`.
use bevy::{input_focus::{FocusCause,InputFocus,tab_navigation::{TabGroup,TabIndex}},picking::{hover::Hovered,pointer::PointerButton},prelude::*,text::{EditableText,EditableTextFilter,TextCursorStyle},ui::{InteractionDisabled,widget::NodeImageMode},ui_widgets::TextInput};
use m2_game::net::{LoginRequest,Net,ServerInfo};
use crate::{core::{self,Click,Consumed,Selected,Surface,UiSet},login::{LoginScreen,Status,empire_name,race_name},select::SelectScreen};

#[derive(Component)] struct FrontendRoot;
#[derive(Component,Clone,Copy,PartialEq)] enum Action{Login,Exit,Channels,Channel(usize),Choose(usize),Start,Logout}
#[derive(Component,Clone,Copy,PartialEq)] enum Field{Account,Password}
#[derive(Component)] struct PasswordMask;
#[derive(Component)] struct StatusLabel;
#[derive(Component)] struct ChannelLabel;
#[derive(Component)] struct SelectionLabel;
#[derive(Component)] struct ChannelPanel;

pub struct FrontendPlugin;
impl Plugin for FrontendPlugin {
    fn build(&self,app:&mut App){app.add_systems(Update,rebuild.in_set(UiSet::State)).add_systems(Update,actions.in_set(UiSet::Actions)).add_systems(Update,refresh.in_set(UiSet::Draw));}
}
fn mode(status:&Status)->u8{match status{Status::Idle|Status::Working(_)|Status::Failed(_)=>0,Status::Select{..}=>1,Status::Entering{..}=>2,Status::InGame{..}=>3}}
fn rebuild(mut commands:Commands,screen:Res<LoginScreen>,server:Res<ServerInfo>,assets:Res<AssetServer>,roots:Query<Entity,With<FrontendRoot>>,mut focus:ResMut<InputFocus>,mut last:Local<Option<u8>>){
    let mode=mode(&screen.status);if *last==Some(mode){return;}*last=Some(mode);focus.clear();
    for root in &roots{commands.entity(root).despawn();}
    if mode==3{return;}
    let root=commands.spawn((FrontendRoot,Node{width:Val::Percent(100.),height:Val::Percent(100.),position_type:PositionType::Absolute,..default()},GlobalZIndex(100),TabGroup::default(),Pickable::IGNORE)).id();
    match &screen.status {
        Status::Idle|Status::Working(_)|Status::Failed(_)=>{
            let bg=core::picture(&mut commands,&assets,root,"locale/en/ui/login.sub",Node{width:Val::Percent(100.),height:Val::Percent(100.),..default()});
            commands.entity(bg).insert(ImageNode{image_mode:NodeImageMode::Stretch,..default()});
            let anchor=commands.spawn((Node{position_type:PositionType::Absolute,left:Val::Percent(50.),bottom:Val::Px(313.),width:Val::Px(208.),height:Val::Px(97.),margin:UiRect::left(Val::Px(-104.)),..default()},Surface,Hovered::default())).id();commands.entity(root).add_child(anchor);
            core::picture(&mut commands,&assets,anchor,"locale/en/ui/login/loginwindow.sub",core::rect(0.,0.,208.,97.));
            for (field,y,index) in [(Field::Account,16.,0),(Field::Password,43.,1)] {
                let secret=field==Field::Password;
                let input=commands.spawn((field,TextInput,Surface,Hovered::default(),TabIndex(index),core::rect(77.,y,120.,18.),EditableText{max_characters:Some(16),..EditableText::new(if secret{""}else{&screen.id})},EditableTextFilter::new(|c|c.is_ascii()&&!c.is_control()),TextFont{font_size:bevy::text::FontSize::Px(12.),..default()},TextColor(if secret{Color::NONE}else{Color::WHITE}),TextCursorStyle{color:if secret{Color::NONE}else{Color::WHITE},selected_text_color:if secret{Some(Color::NONE)}else{None},selection_color:Color::srgba(0.2,0.3,0.7,0.4),..default()},TextLayout::no_wrap())).id();
                commands.entity(anchor).add_child(input);
                if secret{let mask=core::text(&mut commands,anchor,"",core::rect(77.,y,120.,18.));commands.entity(mask).insert(PasswordMask);}else{focus.set(input,FocusCause::Navigated);}
            }
            for (x,caption,action) in [(15.,"Connect",Action::Login),(105.,"Exit",Action::Exit)]{let id=core::button(&mut commands,&assets,anchor,core::rect(x,65.,88.,25.),"d:/ymir work/ui/public/large_button",caption);commands.entity(id).insert(action);}
            let label=core::text(&mut commands,anchor,"",core::rect(0.,-29.,145.,22.));commands.entity(label).insert(ChannelLabel);
            let select=core::button(&mut commands,&assets,anchor,core::rect(150.,-31.,52.,22.),"d:/ymir work/ui/public/small_button","Select");commands.entity(select).insert(Action::Channels);
            let status=core::text(&mut commands,anchor,"",core::rect(-86.,104.,380.,55.));commands.entity(status).insert(StatusLabel);
            let panel=commands.spawn((ChannelPanel,Surface,Hovered::default(),Node{display:Display::None,position_type:PositionType::Absolute,left:Val::Px(-84.),top:Val::Px(155.),width:Val::Px(375.),height:Val::Px(130.),..default()},ZIndex(1))).id();commands.entity(anchor).add_child(panel);core::board(&mut commands,&assets,panel,375.,130.);
            core::text(&mut commands,panel,&server.name,core::rect(20.,14.,330.,20.));
            for (index,channel) in server.channels.iter().enumerate(){let id=core::button(&mut commands,&assets,panel,core::rect(15.+(index%4)as f32*88.,42.+(index/4)as f32*28.,88.,25.),"d:/ymir work/ui/public/large_button",&channel.name);commands.entity(id).insert(Action::Channel(index));}
        }
        Status::Select{empire,characters}=>{
            let panel=commands.spawn((Node{position_type:PositionType::Absolute,left:Val::Percent(8.125),top:Val::Percent(36.667),width:Val::Px(208.),height:Val::Px(323.),..default()},Surface,Hovered::default())).id();commands.entity(root).add_child(panel);core::board(&mut commands,&assets,panel,208.,323.);
            core::text(&mut commands,panel,empire_name(*empire),core::rect(18.,16.,172.,20.));
            for (index,c) in characters.iter().enumerate(){let id=core::button(&mut commands,&assets,panel,core::rect(16.,48.+index as f32*34.,176.,28.),"d:/ymir work/ui/public/large_button",&format!("{}  Lv.{}",c.name,c.level));commands.entity(id).insert((Action::Choose(index),Selected(index==0)));}
            let label=core::text(&mut commands,panel,"",core::rect(16.,194.,180.,56.));commands.entity(label).insert(SelectionLabel);
            let start=core::button(&mut commands,&assets,panel,core::rect(16.,266.,88.,25.),"d:/ymir work/ui/public/large_button","Start");commands.entity(start).insert(Action::Start);
            let logout=core::button(&mut commands,&assets,panel,core::rect(104.,266.,88.,25.),"d:/ymir work/ui/public/large_button","Log out");commands.entity(logout).insert(Action::Logout);
        }
        Status::Entering{..}=>{let label=core::text(&mut commands,root,"",Node{position_type:PositionType::Absolute,left:Val::Percent(30.),bottom:Val::Px(55.),width:Val::Percent(40.),height:Val::Px(42.),..default()});commands.entity(label).insert(StatusLabel);}
        _=>{}
    }
}
#[allow(clippy::too_many_arguments)]
fn actions(mut clicks:MessageReader<Click>,buttons:Query<&Action>,mut panels:Query<&mut Node,With<ChannelPanel>>,mut fields:Query<(&Field,&mut EditableText)>,keys:Res<ButtonInput<KeyCode>>,mut screen:ResMut<LoginScreen>,mut select:ResMut<SelectScreen>,mut net:ResMut<Net>,server:Res<ServerInfo>,mut requests:MessageWriter<LoginRequest>,mut exit:MessageWriter<AppExit>,mut consumed:ResMut<Consumed>){
    let mut actions=clicks.read().filter(|c|c.button==PointerButton::Primary).filter_map(|c|buttons.get(c.entity).ok().copied()).collect::<Vec<_>>();
    if mode(&screen.status)==0 && keys.just_pressed(KeyCode::Enter){actions.push(Action::Login);consumed.keyboard=true;}
    for action in actions {
        match action {
            Action::Exit=>{exit.write(AppExit::Success);}
            Action::Logout=>{net.disconnect();screen.status=Status::Idle;screen.password.clear();}
            Action::Choose(index)=>{if let Status::Select{characters,..}=&screen.status{if index<characters.len(){select.selected=index;}}}
            Action::Start=>{if let Status::Select{characters,..}=&screen.status{if let Some(c)=characters.get(select.selected){screen.status=Status::Entering{slot:c.slot,step:"Starting…".into()};}}}
            Action::Channels if !net.busy()=>{for mut node in &mut panels{node.display=if node.display==Display::None{Display::Flex}else{Display::None};}}
            Action::Channel(index) if !net.busy() && index<server.channels.len()=>{screen.channel=index;for mut node in &mut panels{node.display=Display::None;}}
            Action::Login if !net.busy() && matches!(screen.status,Status::Idle|Status::Failed(_))=>{
                let mut id=String::new();let mut password=String::new();for (field,edit) in &fields{match field{Field::Account=>id=edit.value().to_string(),Field::Password=>password=edit.value().to_string()}}
                if id.is_empty()||password.is_empty(){screen.status=Status::Failed("Enter your account and password.".into());continue;}
                if server.channels.get(screen.channel).is_none(){screen.status=Status::Failed("Select a configured channel.".into());continue;}
                screen.id=id.clone();screen.password.clear();screen.status=Status::Working("Connecting to the account server…".into());
                requests.write(LoginRequest{id,password,channel:screen.channel});
                for (field,mut edit) in &mut fields{if *field==Field::Password{edit.clear();}}
            }
            _=>{}
        }
    }
}
fn refresh(mut commands:Commands,screen:Res<LoginScreen>,server:Res<ServerInfo>,select:Res<SelectScreen>,net:Res<Net>,focus:Res<InputFocus>,mut labels:Query<(&mut Text,Has<StatusLabel>,Has<ChannelLabel>,Has<SelectionLabel>,Has<PasswordMask>)>,fields:Query<(Entity,&Field,&EditableText)>,buttons:Query<(Entity,&Action)>,mut selected:Query<(&Action,&mut Selected)>){
    let password=fields.iter().find(|(_,f,_)|**f==Field::Password);
    for (mut text,status,channel,selection,mask) in &mut labels{let value=if status{match &screen.status{Status::Working(s)|Status::Failed(s)|Status::Entering{step:s,..}=>Some(s.clone()),_=>Some(String::new())}}else if channel{Some(server.channels.get(screen.channel).map_or("No channels",|c|c.name.as_str()).into())}else if selection{Some(if let Status::Select{characters,..}=&screen.status{characters.get(select.selected).map_or("No characters on this account".into(),|c|format!("{}\n{}h {:02}m",race_name(c.race),c.play_minutes/60,c.play_minutes%60))}else{String::new()})}else if mask{Some(password.map_or(String::new(),|(e,_,edit)|format!("{}{}","*".repeat(edit.value().to_string().chars().count()),if focus.get()==Some(e){"|"}else{""})))}else{None};if let Some(value)=value{if **text!=value{**text=value;}}}
    for (entity,action) in &buttons{let disabled=match action{Action::Login|Action::Channels|Action::Channel(_)=>net.busy(),Action::Start=>!matches!(&screen.status,Status::Select{characters,..}if !characters.is_empty()),_=>false};commands.entity(entity).insert_if(InteractionDisabled,||disabled);if !disabled{commands.entity(entity).remove::<InteractionDisabled>();}}
    for (entity,_,_) in &fields{if net.busy(){commands.entity(entity).insert(InteractionDisabled);}else{commands.entity(entity).remove::<InteractionDisabled>();}}
    for (action,mut selected) in &mut selected{if let Action::Choose(index)=action{selected.0=*index==select.selected;}}
}
