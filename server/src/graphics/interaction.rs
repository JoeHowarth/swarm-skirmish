use std::borrow::Cow;

use bevy::prelude::*;
use swarm_lib::bot_logger::{LogEntry, LogLevel};

use super::MapMode;
use crate::{
    game::bot_update::BotLogs,
    replay::LiveOrReplay,
    types::Tick,
    TickSpeed,
};

#[derive(Resource)]
pub enum Selected {
    Bot(Entity),
    None,
}

#[derive(Resource, Default)]
pub struct KeyRepeatTimer {
    pub timer: Timer,
}

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_selection, handle_tick_controls, toggle_logs),
        );
        app.insert_resource(Selected::None);
        app.insert_resource(KeyRepeatTimer {
            timer: Timer::from_seconds(0.2, TimerMode::Repeating),
        });
    }
}

#[derive(Component)]
struct LogsTextContainer;

fn toggle_logs(
    mut commands: Commands,
    selected: Res<Selected>,
    keys: Res<ButtonInput<KeyCode>>,
    bots: Query<Ref<BotLogs>>,
    logs_text_container: Query<Entity, With<LogsTextContainer>>,
) {
    match selected.as_ref() {
        Selected::Bot(entity) => {
            if keys.just_pressed(KeyCode::KeyL) {
                debug!("Pressed L");
                if let Ok(container) = logs_text_container.get_single() {
                    debug!("Despawning logs");
                    commands.entity(container).despawn_recursive();
                } else {
                    debug!("Spawning logs");
                    let logs = &bots.get(*entity).unwrap().0;
                    spawn_logs_view(commands.reborrow(), logs);
                }
            } else if bots.get(*entity).unwrap().is_changed() {
                debug!("Bot logs changed");
                if let Ok(container) = logs_text_container.get_single() {
                    debug!("Refreshing logs on change");
                    commands.entity(container).despawn_recursive();
                    let logs = &bots.get(*entity).unwrap().0;
                    spawn_logs_view(commands.reborrow(), logs);
                }
            }
        }
        Selected::None => {}
    }
}

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);

fn spawn_logs_view(mut commands: Commands, logs: &[LogEntry]) {
    commands
        .spawn((
            Node {
                width: Val::Px(300.0),
                height: Val::Auto,
                border: UiRect::all(Val::Px(5.0)),
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                // horizontally center child text
                justify_content: JustifyContent::Start,
                // vertically center child text
                align_items: AlignItems::Start,

                ..default()
            },
            BorderColor(Color::BLACK),
            BackgroundColor(NORMAL_BUTTON),
            LogsTextContainer,
        ))
        .with_children(|parent| {
            let logs = logs.iter().filter(|log| log.level == LogLevel::Info);
            let button_text_color = TextColor(Color::srgb(0.9, 0.9, 0.9));
            let button_text_font = TextFont {
                font_size: 12.0,
                ..default()
            };
            for log in logs {
                parent.spawn((
                    Text::new(format!("[{:?}] {}", log.level, log.message)),
                    Node {
                        // Padding around each item
                        padding: UiRect::all(Val::Px(5.0)),
                        ..default()
                    },
                    button_text_color,
                    button_text_font.clone(),
                ));
            }
        });
}

fn handle_selection(selected: Res<Selected>, mut map_mode: ResMut<MapMode>) {
    match selected.as_ref() {
        Selected::Bot(entity) => {
            if map_mode.as_ref() != &MapMode::Bot(*entity) {
                *map_mode = MapMode::Bot(*entity);
            }
        }
        Selected::None => {
            if map_mode.as_ref() != &MapMode::All {
                *map_mode = MapMode::All;
            }
        }
    }
}

fn handle_tick_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut tick_speed: ResMut<TickSpeed>,
    replay_or_live: Res<State<LiveOrReplay>>,
    mut tick: ResMut<Tick>,
    time: Res<Time>,
    mut key_timer: ResMut<KeyRepeatTimer>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        tick_speed.is_paused = !tick_speed.is_paused;
        println!("Pressed P");
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        tick_speed.ms = ((tick_speed.ms as f64 * 4. / 3.) as u64).min(2000);
        println!("Decreased tick speed to {}", tick_speed.ms);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        tick_speed.ms = ((tick_speed.ms as f64 * 3. / 4.) as u64).max(50);
        println!("Increased tick speed to {}", tick_speed.ms);
    }

    if *replay_or_live == LiveOrReplay::Replay && tick_speed.is_paused {
        key_timer.timer.tick(time.delta());

        if keys.just_pressed(KeyCode::ArrowLeft) {
            tick.0 -= 1;
            key_timer.timer.reset();
        } else if keys.just_pressed(KeyCode::ArrowRight) {
            tick.0 += 1;
            key_timer.timer.reset();
        } else if keys.pressed(KeyCode::ArrowLeft)
            && key_timer.timer.just_finished()
        {
            tick.0 -= 1;
            key_timer.timer.reset();
        } else if keys.pressed(KeyCode::ArrowRight)
            && key_timer.timer.just_finished()
        {
            tick.0 += 1;
            key_timer.timer.reset();
        }
    }
}
