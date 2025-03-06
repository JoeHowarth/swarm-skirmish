use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{Action, ActionEnvelope, Dir, Team};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarBotData {
    pub bot_id: u32,
    pub team: Team,
    /// World coordinates
    pub pos: Pos,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Default,
)]
pub enum CellKind {
    #[default]
    Unknown,
    Empty,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellStateRadar {
    pub kind: CellKind,
    /// Index of pawn in pawns array
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pawn: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub item: Option<Item>,
    pub pos: Pos, // Added world position to each cell
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
)]
pub enum Item {
    Crumb,
    Fent,
    Truffle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarData {
    pub center_world_pos: Pos,
    pub pawns: Vec<RadarBotData>,
    /// Note: cells sorted by closeness to center, with ties broken by
    /// direction
    pub cells: Vec<CellStateRadar>,
}

#[derive(
    Component,
    Debug,
    Copy,
    Clone,
    Deserialize,
    Serialize,
    Eq,
    PartialEq,
    Default,
    PartialOrd,
    Ord,
)]
pub struct Energy(pub u32);

impl Action {
    pub fn energy(&self) -> Option<Energy> {
        Some(
            match self {
                Action::MoveDir(_) => 1,
                Action::MoveTo(_) => return None,
                Action::Harvest(_) => 5,
            }
            .into(),
        )
    }
}

#[derive(
    Component, Debug, Copy, Clone, Deserialize, Serialize, Eq, PartialEq,
)]
pub struct Pos(pub (usize, usize));

impl CellStateRadar {
    pub fn has_item(item: Item) -> impl Fn(&CellStateRadar) -> bool + Copy {
        move |cell| cell.item == Some(item)
    }
}

/// Builder for BotResponse to enable fluent method chaining
#[derive(Debug, Clone, Default)]
pub struct BotResponseBuilder {
    pub actions: Vec<ActionEnvelope>,
}

impl RadarData {
    /// Convert relative coordinates to world coordinates
    pub fn rel_to_world(&self, rel_x: isize, rel_y: isize) -> Option<Pos> {
        let (world_x, world_y) = self.center_world_pos.as_isize();
        let new_x = world_x + rel_x;
        let new_y = world_y + rel_y;

        // Ensure coordinates are non-negative before creating Pos
        if new_x < 0 || new_y < 0 {
            // If coordinates would be negative, return None
            None
        } else {
            Some(Pos::from((new_x, new_y)))
        }
    }

    /// Convert world coordinates to relative coordinates
    pub fn world_to_rel(&self, world: Pos) -> (isize, isize) {
        let (center_x, center_y) = self.center_world_pos.as_isize();
        let (world_x, world_y) = world.as_isize();
        (world_x - center_x, world_y - center_y)
    }

    /// Get a cell by relative coordinates
    /// Returns None if no cell exists at those coordinates
    pub fn get_relative(
        &self,
        rel_x: isize,
        rel_y: isize,
    ) -> Option<&CellStateRadar> {
        let target_pos = self.rel_to_world(rel_x, rel_y)?;
        self.cells.iter().find(|cell| cell.pos == target_pos)
    }

    /// Get a cell in the specified direction
    pub fn get_dir(&self, dir: Dir) -> Option<&CellStateRadar> {
        let (rel_x, rel_y) = dir.to_deltas();
        self.get_relative(rel_x, rel_y)
    }

    /// Find directions that match a predicate
    pub fn find_dirs(
        &self,
        mut filter: impl FnMut(&CellStateRadar) -> bool,
    ) -> Option<(Dir, &CellStateRadar)> {
        Dir::iter().find_map(|dir| {
            if let Some(cell) = self.get_dir(dir) {
                if filter(cell) {
                    return Some((dir, cell));
                }
            }
            None
        })
    }

    /// Get a mutable reference to a cell by relative coordinates
    /// Returns None if no cell exists at those coordinates
    pub fn get_relative_mut(
        &mut self,
        rel_x: isize,
        rel_y: isize,
    ) -> Option<&mut CellStateRadar> {
        let target_pos = self.rel_to_world(rel_x, rel_y)?;
        self.cells.iter_mut().find(|cell| cell.pos == target_pos)
    }

    /// Filter cells based on a predicate, returning iterator of (rel_coords,
    /// cell_ref) pairs
    /// Relies on cells list being sorted by manhattan distance
    pub fn filter<F>(
        &self,
        filter: F,
    ) -> impl Iterator<Item = ((isize, isize), &CellStateRadar)> + '_
    where
        F: Fn(&CellStateRadar) -> bool + Copy + 'static,
    {
        self.cells.iter().filter_map(move |cell| {
            if filter(cell) {
                let rel_coords = self.world_to_rel(cell.pos);
                Some((rel_coords, cell))
            } else {
                None
            }
        })
    }

    /// Find the closest cell that matches the filter
    /// Returns relative coordinates and cell reference if found
    pub fn find<F>(
        &self,
        filter: F,
    ) -> Option<((isize, isize), &CellStateRadar)>
    where
        F: Fn(&CellStateRadar) -> bool + Copy + 'static,
    {
        // First check if center cell matches
        if let Some(center_cell) = self.get_relative(0, 0) {
            if filter(center_cell) {
                return Some(((0, 0), center_cell));
            }
        }

        // Find all matching cells
        // Relies on filter returning cells sorted by manhattan distance
        self.filter(filter).next()
    }
}
