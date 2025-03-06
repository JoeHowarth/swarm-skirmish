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
use std::{
    sync::{Arc, LazyLock, OnceLock, RwLock},
    time::Duration,
};

use apply_actions::{ActionsPlugin, ActionsSystemSet};
use argh::{FromArgValue, FromArgs};
use bevy::{
    prelude::*,
    state::state::FreelyMutableState,
    time::common_conditions::on_timer,
    utils::HashMap,
};
use bot_update::{BotId, BotUpdatePlugin, BotUpdateSystemSet};
use levels::{Levels, LevelsDiscriminants, LevelsPlugin};
use serde::{Deserialize, Serialize};
use strum::IntoDiscriminant;
use swarm_lib::{bot_logger::BotLogger, Bot, Energy, Item, Pos, Team};
use tilemap::TilemapSystemSimUpdateSet;

mod apply_actions;
mod bot_update;
mod core;
mod levels;
mod tilemap;

static MAP_SIZE: LazyLock<RwLock<Option<(usize, usize)>>> =
    LazyLock::new(|| RwLock::new(None));

pub fn get_map_size() -> Option<(usize, usize)> {
    MAP_SIZE.read().expect("Failed to read RWLock").clone()
}

#[derive(FromArgs)]
/// Swarm Server
pub struct Args {
    #[argh(subcommand)]
    /// the level to load
    pub level: Option<Levels>,

    #[argh(option, default = "500")]
    /// the tick rate in milliseconds
    pub tick_ms: u64,

    #[argh(option)]
    /// the width of the map
    pub width: Option<usize>,

    #[argh(option)]
    /// the height of the map
    pub height: Option<usize>,
}

use dlopen2::wrapper::{Container, WrapperApi};

fn main() {
    let args: Args = argh::from_env();

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
            ActionsPlugin,
            CorePlugin,
            LevelsPlugin,
            BotUpdatePlugin,
        ))
        .insert_state(
            args.level
                .as_ref()
                .unwrap_or(&Levels::default())
                .discriminant(),
        )
        .insert_resource(args.level.unwrap_or_default())
        .insert_state(GameState::Idle)
        .add_systems(Startup, camera_setup)
        .add_systems(
            OnExit(GameState::InGame),
            |mut commands: Commands, pawns: Query<Entity, With<PawnKind>>| {
                for pawn in pawns.iter() {
                    commands.entity(pawn).despawn_recursive();
                }
            },
        )
        .configure_sets(
            Update,
            (
                BotUpdateSystemSet,
                ActionsSystemSet,
                CoreSystemsSet,
                TilemapSystemSimUpdateSet,
            )
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(on_timer(Duration::from_millis(args.tick_ms))),
        )
        .add_systems(Update, (exit_system, check_win_condition, display_win_ui))
        .run();
}

#[derive(States, Hash, Eq, PartialEq, Clone, Debug, Default)]
pub enum GameState {
    #[default]
    Idle,
    InGame,
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
    mut next_state: ResMut<NextState<GameState>>,
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
        next_state.set(GameState::Idle);
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
