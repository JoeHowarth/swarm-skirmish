#![allow(unused_imports, dead_code)]
#![feature(mpmc_channel)]
#![feature(arbitrary_self_types)]

use std::{
    sync::{LazyLock, RwLock},
    time::Duration,
};

use argh::FromArgs;
use bevy::{
    color::palettes::css,
    prelude::*,
};
use game::{
    apply_actions::ActionsPlugin,
    bot_update::{BotId, BotUpdatePlugin},
    core::{CorePlugin, CoreSystemsSet},
};
use graphics::GraphicsSystemSet;
use levels::{Levels, LevelsPlugin};
use strum::IntoDiscriminant;
use swarm_lib::{
    BotData,
    Item,
    Pos,
    Team,
};
use types::Tick;

mod game;
mod graphics;
mod levels;
mod types;

static MAP_SIZE: LazyLock<RwLock<Option<(usize, usize)>>> =
    LazyLock::new(|| RwLock::new(None));

pub fn get_map_size() -> Option<(usize, usize)> {
    *MAP_SIZE.read().expect("Failed to read RWLock")
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


fn main() {
    let args: Args = argh::from_env();

    let scale = 32.0;
    let res = match &args.level {
        Some(Levels::EconLoop(args)) => {
            (args.width as f32 * scale, args.height as f32 * scale)
        }
        Some(Levels::RandomCrumbsAndTruffles(args)) => {
            (args.width as f32 * scale, args.height as f32 * scale)
        }
        _ => (500.0, 500.0),
    };
    let res = (res.0 + 2.0, res.1 + 2.0);

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: res.into(),
                    ..default()
                }),
                ..default()
            }),
            bevy_pancam::PanCamPlugin,
        ))
        .add_plugins((
            graphics::GraphicsPlugin,
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
        .insert_resource(TickSpeed {
            ms: args.tick_ms,
            is_paused: false,
        })
        .insert_state(DataSource::Live)
        .add_systems(Startup, camera_setup)
        .add_systems(
            OnExit(GameState::InGame),
            |mut commands: Commands, pawns: Query<Entity, With<BotData>>| {
                for pawn in pawns.iter() {
                    commands.entity(pawn).despawn_recursive();
                }
            },
        )
        .configure_sets(
            Update,
            (TickSystemSet, CoreSystemsSet, GraphicsSystemSet)
                .chain()
                .run_if(in_state(GameState::InGame))
                .run_if(should_tick),
        )
        .add_systems(
            Update,
            (
                update_tick.in_set(TickSystemSet),
                exit_system,
                check_win_condition,
                display_win_ui,
            ),
        )
        .run();
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
struct TickSystemSet;

#[derive(Resource)]
pub struct TickSpeed {
    pub ms: u64,
    pub is_paused: bool,
}

pub fn should_tick(
    tick_ms: Res<TickSpeed>,
    time: Res<Time>,
    mut timer: Local<Timer>,
) -> bool {
    if tick_ms.is_paused {
        return false;
    }
    timer.tick(time.delta());
    if timer.just_finished() {
        timer.set_duration(Duration::from_millis(tick_ms.ms));
        timer.reset();
        return true;
    }
    false
}

fn update_tick(mut tick: ResMut<Tick>) {
    tick.0 += 1;
}

#[derive(States, Hash, Eq, PartialEq, Clone, Debug)]
pub enum DataSource {
    Replay,
    Live,
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
            grab_buttons: vec![MouseButton::Right, MouseButton::Left],
            min_scale: 0.25,
            max_scale: 5.0,
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Main Text"),
        TextColor(css::RED.into()),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            ..default()
        },
        TextFont {
            font_size: 20.0,
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
    query: Query<(&BotId, &BotData)>,
    won: Option<Res<Won>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if won.is_some() {
        return;
    }
    for (bot_id, bot_data) in query.iter() {
        let amt = bot_data.inventory.get(Item::Fent);
        if amt == 0 {
            continue;
        }

        let amt = bot_data.inventory.get(Item::Truffle);
        if amt < 2 {
            continue;
        }
        info!(
            "Team {team} won! Bot {bot_id:?} picked up the Fent and 2 Truffles",
            team = bot_data.team
        );
        commands.insert_resource(Won(bot_data.team));
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
