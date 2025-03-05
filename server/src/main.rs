#![allow(unused_imports)]
#![feature(mpmc_channel)]

use core::{
    CellState,
    CorePlugin,
    CoreSystemsSet,
    Inventory,
    PawnKind,
    SGridWorld as GridWorld,
};
use std::time::Duration;

use actions::{ActionsPlugin, ActionsSystemSet};
use bevy::{prelude::*, time::common_conditions::on_timer};
use server::{BotHandlerPlugin, BotId, ServerSystems};
use subscriptions::{SubscriptionsPlugin, SubscriptionsSystemSet};
use swarm_lib::{Energy, Item, Pos, Team};
use tilemap::TilemapSystemSet;

mod actions;
mod core;
mod server;
mod subscriptions;
mod tilemap;

pub const MAP_SIZE: (usize, usize) = (50, 50);

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
                ActionsSystemSet,
                CoreSystemsSet,
                TilemapSystemSet,
                ServerSystems,
                SubscriptionsSystemSet,
            )
                .chain()
                .run_if(on_timer(Duration::from_millis(500))),
        )
        .add_systems(Update, (exit_system, check_win_condition, display_win_ui))
        .run();
}

fn init_map(mut commands: Commands) {
    let mut grid_world =
        GridWorld::new(MAP_SIZE.0, MAP_SIZE.1, CellState::empty());

    let player = commands
        .spawn((
            PawnKind::default(),
            Team::Player,
            Energy(100),
            Pos((2, 2).into()),
        ))
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
    for coord in grid_world.find_path((5, 3), (8, 13)).unwrap() {
        let cell = grid_world.get_pos_mut(coord);
        cell.item = Some(Item::Crumb);
    }
    // Add fent at end of crumb trail
    grid_world.get_mut(8, 14).item = Some(Item::Fent);

    grid_world.get_mut(2, 8).item = Some(Item::Truffle);
    grid_world.get_mut(12, 2).item = Some(Item::Truffle);

    // Add a border of Blocked cells around the edge of the grid
    for x in 0..MAP_SIZE.0 {
        // Top and bottom borders
        grid_world.set(x, 0, CellState::blocked());
        grid_world.set(x, MAP_SIZE.1 - 1, CellState::blocked());
    }

    for y in 0..MAP_SIZE.1 {
        // Left and right borders
        grid_world.set(0, y, CellState::blocked());
        grid_world.set(MAP_SIZE.0 - 1, y, CellState::blocked());
    }

    commands.insert_resource(grid_world);
}

pub fn camera_setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        bevy_pancam::PanCam {
            move_keys: bevy_pancam::DirectionKeys::arrows(),
            grab_buttons: vec![MouseButton::Right],
            min_scale: 0.25,
            max_scale: 5.0,
            ..default()
        },
        Transform::from_scale(Vec3::splat(2.0)), // Start zoomed out
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
        let Some(&amt) = inventory.get(&Item::Fent) else {
            continue;
        };
        if amt == 0 {
            continue;
        }

        let Some(&amt) = inventory.get(&Item::Truffle) else {
            continue;
        };
        if amt < 2 {
            continue;
        }
        info!("Team {team} won! Bot {bot:?} picked up the Fent and 2 Truffles");
        commands.insert_resource(Won(*team));
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
