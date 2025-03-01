#![allow(unused_imports)]
#![feature(mpmc_channel)]

use core::{CellState, CorePlugin, CoreSystemsSet, Inventory, PawnKind, Pos};
use std::{collections::VecDeque, net::TcpListener, time::Duration};

use actions::{ActionsPlugin, ActionsSystemSet};
use array2d::Array2D;
use bevy::{
    prelude::*,
    time::common_conditions::on_timer,
    utils::{HashMap, HashSet},
};
use gridworld::GridWorld;
use server::{
    ActionRecv,
    BotHandlerPlugin,
    BotId,
    BotIdToEntity,
    ServerSystems,
    ServerUpdates,
    SubscriptionRecv,
};
use subscriptions::{SubscriptionsPlugin, SubscriptionsSystemSet};
use swarm_lib::{
    Action,
    BotMsgEnvelope,
    CellStateRadar,
    Dir,
    Item,
    RadarBotData,
    RadarData,
    ServerUpdate,
    ServerUpdateEnvelope,
    SubscriptionType,
    Team,
};
use tilemap::TilemapSystemSet;

mod actions;
mod core;
mod gridworld;
mod server;
mod subscriptions;
mod tilemap;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (500.0, 500.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            tilemap::TilemapPlugin,
            BotHandlerPlugin,
            ActionsPlugin,
            SubscriptionsPlugin,
            CorePlugin,
        ))
        .add_systems(Startup, (camera_setup, init_map))
        .configure_sets(
            Update,
            (
                ServerSystems,
                SubscriptionsSystemSet,
                ActionsSystemSet,
                CoreSystemsSet,
                TilemapSystemSet,
            )
                .chain()
                .run_if(on_timer(Duration::from_millis(250))),
        )
        .add_systems(Update, (exit_system, check_win_condition, display_win_ui))
        .run();
}

fn init_map(mut commands: Commands) {
    let mut grid_world = GridWorld::new(16, 16);

    let player = commands
        .spawn((PawnKind::default(), Team::Player, Pos((2, 2).into())))
        .id();

    // let enemy = commands
    //     .spawn((PawnKind::FindBot, Team::Enemy, Pos((13, 13).into())))
    //     .id();

    grid_world.set(2, 2, CellState::new_with_pawn(player));
    // grid_world.set(13, 13, CellState::new_with_pawn(enemy));

    for y in 1..10 {
        grid_world.set(10, y, CellState::blocked());
    }

    // Add crumbs
    for coord in grid_world.find_path((3, 2), (8, 13)).unwrap() {
        let cell = grid_world.get_mut(coord.x as usize, coord.y as usize);
        cell.item = Some(Item::Crumb);
    }
    // Add fent at end of crumb trail
    grid_world.get_mut(8, 14).item = Some(Item::Fent);

    commands.insert_resource(grid_world);
}

pub fn camera_setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        bevy_pancam::PanCam {
            move_keys: bevy_pancam::DirectionKeys::arrows(),
            grab_buttons: vec![MouseButton::Right],
            ..default()
        },
    ));
}

pub fn exit_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: EventWriter<AppExit>,
) {
    if keys.all_pressed([KeyCode::ControlLeft, KeyCode::KeyC]) {
        exit.send(AppExit::Success);
    }
}

#[derive(Resource)]
pub struct Won(pub Team);

#[derive(Component)]
struct WinDisplay;

fn check_win_condition(
    mut commands: Commands,
    query: Query<(&BotId, &Inventory, &Team)>,
    won: Option<Res<Won>>,
) {
    if won.is_some() {
        return;
    }
    for (bot, inventory, team) in query.iter() {
        if let Some(&amt) = inventory.get(&Item::Fent) {
            if amt > 0 {
                info!("Team {team} won! Bot {bot:?} picked up the Fent");
                commands.insert_resource(Won(*team));
            }
        }
    }
}

fn display_win_ui(
    mut commands: Commands,
    won: Option<Res<Won>>,
    query: Query<Entity, With<WinDisplay>>,
) {
    // Only create UI if we have a win and haven't created the UI yet
    if won.is_some() && query.is_empty() {
        let team = match won.unwrap().0 {
            Team::Player => "Player",
            Team::Enemy => "Enemy",
        };

        commands
            .spawn((
                Node {
                    width: Val::Percent(50.0),
                    height: Val::Percent(25.0),
                    position_type: PositionType::Absolute,
                    left: Val::Percent(25.0),
                    top: Val::Percent(35.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(20.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.9)),
                WinDisplay,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new(format!("Team {} Won!", team)),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

                parent.spawn((
                    Text::new("Press Ctrl+C to exit"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
    }
}
