use bevy::prelude::*;

use super::MapMode;
use crate::TickSpeed;

#[derive(Resource)]
pub enum Selected {
    Bot(Entity),
    None,
}

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (interaction_system, keyboard_interaction));
        app.insert_resource(Selected::None);
    }
}

fn interaction_system(selected: Res<Selected>, mut map_mode: ResMut<MapMode>) {
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

fn keyboard_interaction(
    keys: Res<ButtonInput<KeyCode>>,
    mut tick_speed: ResMut<TickSpeed>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        tick_speed.is_paused = !tick_speed.is_paused;
        println!("Pressed P");
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        tick_speed.ms = (tick_speed.ms * 4 / 3).min(2000);
        println!("Decreased tick speed to {}", tick_speed.ms);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        tick_speed.ms = (tick_speed.ms * 3 / 4).max(50);
        println!("Increased tick speed to {}", tick_speed.ms);
    }
}
