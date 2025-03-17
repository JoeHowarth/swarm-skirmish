use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css,
    gizmos,
    prelude::*,
    text::{FontSmoothing, TextBounds},
};
use bevy_ecs_tilemap::prelude::*;
use image::DynamicImage;
use swarm_lib::{
    known_map::ClientCellState,
    Action::{self, *},
    ActionWithId,
    BotData,
    BuildingKind,
    CellKind,
    FrameKind,
    Item,
    Pos,
    Team,
};

use super::{
    interaction::Selected,
    tilemap::TilemapWorldCoords,
    MapMode,
    Textures,
};
use crate::{
    apply_actions::{ActionContainer, ActionState, CurrentAction, PastActions},
    bot_update::{BotId, BotIdToEntity},
    get_map_size,
    types::{CellState, GridWorld, Tick},
    GameState,
    MAP_SIZE,
};

pub struct RenderBotsPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct RenderBotsSystemSet;

impl Plugin for RenderBotsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, render_move_to);
        app.add_systems(
            Update,
            (render_bots, ensure_bot_sprite).in_set(RenderBotsSystemSet),
        );
    }
}

fn ensure_bot_sprite(
    mut commands: Commands,
    bots: Query<(Entity, &BotData), Without<Sprite>>,
    textures: Res<Textures>,
    tilemap_coords: Res<TilemapWorldCoords>,
) {
    for (entity, bot_data) in bots.iter() {
        commands
            .entity(entity)
            .insert((
                Sprite::from_atlas_image(
                    textures.pawns.0.clone(),
                    TextureAtlas {
                        layout: textures.pawns.1.clone(),
                        index: match bot_data.frame {
                            FrameKind::Flea => 0,
                            FrameKind::Tractor => 1,
                            FrameKind::Building(BuildingKind::Small) => 2,
                        },
                    },
                ),
                Transform::from_xyz(
                    tilemap_coords.pos_to_world(&bot_data.pos).x,
                    tilemap_coords.pos_to_world(&bot_data.pos).y,
                    1.0,
                )
                .with_scale(Vec3::new(0.5, 0.5, 1.0)),
            ))
            .observe(
                |click: Trigger<Pointer<Click>>,
                 mut selected: ResMut<Selected>| {
                    // tdodo
                    let entity = click.entity();
                    match selected.as_ref() {
                        Selected::Bot(prev_selected) => {
                            if *prev_selected == entity {
                                *selected = Selected::None;
                            } else {
                                *selected = Selected::Bot(entity);
                            }
                        }
                        Selected::None => {
                            *selected = Selected::Bot(entity);
                        }
                    }
                },
            )
            .with_children(|parent| {
                parent.spawn((
                    Text2d::new("<Action Reason>"),
                    TextColor(css::ANTIQUE_WHITE.into()),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    BotLabel::Action,
                    // Wrap text in the rectangle
                    TextLayout::new(JustifyText::Left, LineBreak::WordBoundary),
                    TextBounds::from(Vec2::new(130.0, 100.0)),
                    Transform::from_translation(Vec3::new(0.0, 20.0, 20.0)),
                ));
                parent.spawn((
                    Text2d::new("E <Energy>"),
                    TextColor(css::ANTIQUE_WHITE.into()),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    BotLabel::Energy,
                    TextLayout::new(JustifyText::Left, LineBreak::WordBoundary),
                    TextBounds::from(Vec2::new(50.0, 20.0)),
                    Transform::from_translation(Vec3::new(-40.0, 0., 20.0)),
                ));
            });
    }
}

#[derive(Component)]
enum BotLabel {
    Action,
    Energy,
}

fn render_bots(
    current_tick: Res<Tick>,
    bot_data_q: Query<(&BotData, &BotId)>,
    mut bots_with_sprites: Query<
        (
            Entity,
            &mut Transform,
            &mut Visibility,
            &Children,
            &CurrentAction,
            &PastActions,
        ),
        With<Sprite>,
    >,
    mut bot_action_labels: Query<(&mut Text2d, &BotLabel)>,
    tilemap_coords: Res<TilemapWorldCoords>,
    map_mode: Res<MapMode>,
    bot_id_to_entity: Res<BotIdToEntity>,
) {
    for (
        entity,
        mut transform,
        mut visibility,
        children,
        current_action,
        past_actions,
    ) in bots_with_sprites.iter_mut()
    {
        let (bot_data, bot_id) = bot_data_q.get(entity).unwrap();
        match *map_mode {
            MapMode::All => {
                update_bot_sprite(
                    &mut transform,
                    &mut visibility,
                    &tilemap_coords,
                    bot_data,
                );

                update_text_labels(
                    children,
                    &mut bot_action_labels,
                    get_reason(&current_tick, &current_action, &past_actions)
                        .unwrap_or("")
                        .to_owned(),
                    format!("E: {}", bot_data.energy.0),
                );
                // for child in children.iter() {
                //     let Ok((mut text, label)) =
                //         bot_action_labels.get_mut(*child)
                //     else {
                //         continue;
                //     };

                //     match label {
                //         BotLabel::Action => {
                //             text.0 = get_reason(
                //                 &current_tick,
                //                 &current_action,
                //                 &past_actions,
                //             )
                //             .unwrap_or("")
                //             .to_owned();
                //         }
                //         BotLabel::Energy => {
                //             text.0 = format!("E: {}", bot_data.energy.0);
                //         }
                //     }
                // }
            }
            MapMode::Team(team) => {
                todo!()
            }
            MapMode::Bot(bot_e) => {
                let (_selected_bot, selected_bot_id) =
                    bot_data_q.get(bot_e).unwrap();

                // Check if selected_bot has seen this bot
                if bot_data
                    .known_bots
                    .iter()
                    .any(|b| b.bot_id == selected_bot_id.0)
                {
                    update_bot_sprite(
                        &mut transform,
                        &mut visibility,
                        &tilemap_coords,
                        bot_data,
                    );
                } else {
                    *visibility = Visibility::Hidden;
                }

                let (action_str, energy_str) = if bot_e == entity {
                    (
                        get_reason(
                            &current_tick,
                            &current_action,
                            &past_actions,
                        )
                        .unwrap_or("")
                        .to_owned(),
                        format!("E: {}", bot_data.energy.0),
                    )
                } else {
                    ("".to_string(), "".to_string())
                };

                update_text_labels(
                    children,
                    &mut bot_action_labels,
                    action_str,
                    energy_str,
                );

                // for child in children.iter() { let Ok((mut text, label)) =
                //         bot_action_labels.get_mut(*child)
                //     else {
                //         continue;
                //     };

                //     if bot_e != entity {
                //         text.0 = "".to_string();
                //         continue;
                //     }

                //     match label {
                //         BotLabel::Action => {
                //             text.0 = get_reason(
                //                 &current_tick,
                //                 &current_action,
                //                 &past_actions,
                //             )
                //             .unwrap_or("")
                //             .to_owned();
                //         }
                //         BotLabel::Energy => {
                //             text.0 = format!("E: {}", bot_data.energy.0);
                //         }
                //     }
                // }
            }
        }
    }
}

fn update_text_labels(
    children: &Children,
    bot_action_labels: &mut Query<(&mut Text2d, &BotLabel)>,
    action_str: String,
    energy_str: String,
) {
    let child = children
        .iter()
        .find(|child| {
            let Ok((text, BotLabel::Action)) = bot_action_labels.get(**child)
            else {
                return false;
            };
            true
        })
        .unwrap();
    bot_action_labels.get_mut(*child).unwrap().0 .0 = action_str;

    let child = children
        .iter()
        .find(|child| {
            let Ok((text, BotLabel::Energy)) = bot_action_labels.get(**child)
            else {
                return false;
            };
            true
        })
        .unwrap();
    bot_action_labels.get_mut(*child).unwrap().0 .0 = energy_str;
}

fn update_bot_sprite(
    transform: &mut Transform,
    visibility: &mut Visibility,
    tilemap_coords: &TilemapWorldCoords,
    bot_data: &BotData,
) {
    *visibility = Visibility::Visible;
    let pos = tilemap_coords.pos_to_world(&bot_data.pos);
    transform.translation.x = pos.x;
    transform.translation.y = pos.y;
}

fn get_reason(
    current_tick: &Tick,
    current_action: &CurrentAction,
    past_actions: &PastActions,
) -> Option<&'static str> {
    if let Some(action) = &current_action.0 {
        Some(action.reason)
    } else if let Some(action) = past_actions.0.last() {
        if action.completed_tick + 1 >= current_tick.0 {
            Some(action.reason)
        } else {
            None
        }
    } else {
        None
    }
}

fn render_move_to(
    mut gizmos: Gizmos,
    actions: Query<(&BotData, &CurrentAction)>,
    tilemap_coords: Option<Res<TilemapWorldCoords>>,
) {
    let Some(tilemap_coords) = tilemap_coords else {
        return;
    };

    for (bot_data, current_action) in actions.iter() {
        let Some(ActionContainer { kind, state, .. }) = &current_action.0
        else {
            continue;
        };

        if let (Action::MoveTo(path), ActionState::MoveTo { idx }) =
            (kind, state)
        {
            let mut pos = bot_data.pos;
            for dst in &path[*idx..] {
                let src_world = tilemap_coords.pos_to_world(&pos);
                let dst_world = tilemap_coords.pos_to_world(dst);

                gizmos.line(
                    Vec3::new(src_world.x, src_world.y, 100.0),
                    Vec3::new(dst_world.x, dst_world.y, 100.0),
                    css::RED,
                );

                pos = *dst;
            }
        }
    }
}
