use array2d::Array2D;
use bevy_ecs::component::Component;
use bevy_math::{IVec2, UVec2};
use rand::Rng;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{
    Action,
    ActionEnvelope,
    ActionId,
    BotResponse,
    Dir,
    Team,
};

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
pub enum CellKindRadar {
    #[default]
    Unknown,
    Empty,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellStateRadar {
    pub kind: CellKindRadar,
    pub pawn: Option<usize>,
    pub item: Option<Item>,
}

impl CellStateRadar {
    pub fn has_item(item: Item) -> impl Fn(&CellStateRadar) -> bool + Copy {
        move |cell| cell.item == Some(item)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Item {
    Crumb,
    Fent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarData {
    pub center_world_pos: Pos,
    pub bots: Vec<RadarBotData>,
    pub cells: Array2D<CellStateRadar>,
}

impl RadarData {
    /// Get radar size (assumes square radar)
    pub fn size(&self) -> usize {
        self.cells.column_len() // or row_len(), they should be equal
    }

    /// Get the center index of the radar (where the bot is located)
    pub fn center_index(&self) -> usize {
        self.size() / 2
    }

    /// Convert relative coordinates to array indices
    /// (0,0) relative is the center of the radar (where the bot is)
    /// Returns None if the resulting indices would be out of bounds
    pub fn rel_to_index(
        &self,
        rel_x: isize,
        rel_y: isize,
    ) -> Option<(usize, usize)> {
        let center = self.center_index() as isize;
        let x = center + rel_x;
        let y = center + rel_y;

        if x >= 0
            && x < self.size() as isize
            && y >= 0
            && y < self.size() as isize
        {
            Some((x as usize, y as usize))
        } else {
            None
        }
    }

    /// Convert array indices to relative coordinates
    pub fn index_to_rel(
        &self,
        index_x: usize,
        index_y: usize,
    ) -> (isize, isize) {
        let center = self.center_index();
        (
            index_x as isize - center as isize,
            index_y as isize - center as isize,
        )
    }

    /// Convert relative coordinates to world coordinates
    pub fn rel_to_world(&self, rel_x: isize, rel_y: isize) -> Pos {
        let (world_x, world_y) = self.center_world_pos.as_isize();
        Pos::from((world_x + rel_x, world_y + rel_y))
    }

    /// Convert world coordinates to relative coordinates
    pub fn world_to_rel(&self, world: Pos) -> (isize, isize) {
        let (center_x, center_y) = self.center_world_pos.as_isize();
        let (world_x, world_y) = world.as_isize();
        (world_x as isize - center_x, world_y as isize - center_y)
    }

    /// Get a cell by relative coordinates
    /// Returns None if out of bounds
    pub fn get_relative(
        &self,
        rel_x: isize,
        rel_y: isize,
    ) -> Option<&CellStateRadar> {
        self.rel_to_index(rel_x, rel_y)
            .and_then(|(x, y)| self.cells.get(x, y))
    }

    pub fn get_dir(&self, dir: Dir) -> &CellStateRadar {
        let (rel_x, rel_y) = dir.to_deltas();
        self.get_relative(rel_x, rel_y).unwrap()
    }

    pub fn find_dirs(
        &self,
        mut filter: impl FnMut(&CellStateRadar) -> bool,
    ) -> Option<(Dir, &CellStateRadar)> {
        Dir::iter().find_map(|dir| {
            let cell = self.get_dir(dir);
            if filter(cell) {
                Some((dir, cell))
            } else {
                None
            }
        })
    }

    /// Get a mutable reference to a cell by relative coordinates
    /// Returns None if out of bounds
    pub fn get_relative_mut(
        &mut self,
        rel_x: isize,
        rel_y: isize,
    ) -> Option<&mut CellStateRadar> {
        if let Some((x, y)) = self.rel_to_index(rel_x, rel_y) {
            self.cells.get_mut(x, y)
        } else {
            None
        }
    }

    /// Filter cells based on a predicate, returning iterator of (rel_coords,
    /// cell_ref) pairs
    pub fn filter<F>(
        &self,
        filter: F,
    ) -> impl Iterator<Item = ((isize, isize), &CellStateRadar)> + '_
    where
        F: Fn(&CellStateRadar) -> bool + Copy + 'static,
    {
        let size = self.size();
        let center = self.center_index();

        (0..size).flat_map(move |y| {
            (0..size).filter_map(move |x| {
                let cell = self.cells.get(x, y)?;
                if filter(cell) {
                    let rel_x = x as isize - center as isize;
                    let rel_y = y as isize - center as isize;
                    Some(((rel_x, rel_y), cell))
                } else {
                    None
                }
            })
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
        let center = self.center_index();
        if let Some(cell) = self.cells.get(center, center) {
            if filter(cell) {
                return Some(((0, 0), cell));
            }
        }

        // Search spirally outward from center
        let size = self.size() as isize ;
        let size = size + size;
        let max_distance = size.max(1); // Ensure we don't go into an infinite loop

        for distance in 1..max_distance {
            // Check all cells at Manhattan distance 'distance' from center
            for dx in -distance..=distance {
                for dy in [-distance, distance] {
                    // Top and bottom edges
                    if dx.abs() + dy.abs() != distance {
                        continue;
                    }

                    if let Some((x, y)) = self.rel_to_index(dx, dy) {
                        if let Some(cell) = self.cells.get(x, y) {
                            if filter(cell) {
                                return Some(((dx, dy), cell));
                            }
                        }
                    }
                }
            }

            for dy in (-distance + 1)..distance {
                for dx in [-distance, distance] {
                    // Left and right edges
                    if dx.abs() + dy.abs() != distance {
                        continue;
                    }

                    if let Some((x, y)) = self.rel_to_index(dx, dy) {
                        if let Some(cell) = self.cells.get(x, y) {
                            if filter(cell) {
                                return Some(((dx, dy), cell));
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

/// Builder for BotResponse to enable fluent method chaining
#[derive(Debug, Clone, Default)]
pub struct BotResponseBuilder {
    actions: Vec<ActionEnvelope>,
}

impl BotResponseBuilder {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn push_action_id(
        &mut self,
        action: Action,
        id: ActionId,
    ) -> &mut Self {
        self.actions.push(ActionEnvelope { id, action });
        self
    }

    pub fn push_action(&mut self, action: Action) -> &mut Self {
        self.push_action_id(action, rand::rng().random())
    }

    pub fn build(&mut self) -> BotResponse {
        BotResponse {
            actions: std::mem::take(&mut self.actions),
        }
    }
}

impl Dir {
    pub fn to_deltas(&self) -> (isize, isize) {
        match self {
            Dir::Up => (0, 1),
            Dir::Down => (0, -1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    pub fn from_deltas_ivec(deltas: IVec2) -> Option<Self> {
        Dir::from_deltas((deltas.x as isize, deltas.y as isize))
    }

    pub fn from_deltas(deltas: (isize, isize)) -> Option<Self> {
        match (deltas.0, deltas.1) {
            (0, 1) => Some(Dir::Up),
            (0, -1) => Some(Dir::Down),
            (-1, 0) => Some(Dir::Left),
            (1, 0) => Some(Dir::Right),
            _ => None,
        }
    }
}

#[derive(
    Component, Debug, Copy, Clone, Deserialize, Serialize, Eq, PartialEq,
)]
pub struct Pos(pub (usize, usize));

impl std::ops::Add<(isize, isize)> for Pos {
    type Output = (isize, isize);

    fn add(self, (rhs_x, rhs_y): (isize, isize)) -> Self::Output {
        let (self_x, self_y) = self.as_isize();
        (self_x + rhs_x, self_y + rhs_y)
    }
}

impl std::ops::Add for Pos {
    type Output = (isize, isize);

    fn add(self, rhs: Self) -> Self::Output {
        let (self_x, self_y) = self.as_isize();
        let (rhs_x, rhs_y) = rhs.as_isize();
        (self_x + rhs_x, self_y + rhs_y)
    }
}

impl std::ops::Sub for Pos {
    type Output = (isize, isize);

    fn sub(self, rhs: Self) -> Self::Output {
        let (self_x, self_y) = self.as_isize();
        let (rhs_x, rhs_y) = rhs.as_isize();
        (self_x - rhs_x, self_y - rhs_y)
    }
}

impl Pos {
    pub fn x(&self) -> usize {
        self.0 .0
    }
    pub fn y(&self) -> usize {
        self.0 .1
    }

    pub fn uvec2(&self) -> UVec2 {
        UVec2::new(self.x() as u32, self.y() as u32)
    }

    pub fn as_isize(&self) -> (isize, isize) {
        (self.0 .0 as isize, self.0 .1 as isize)
    }
}

impl From<(u32, u32)> for Pos {
    fn from(value: (u32, u32)) -> Self {
        Self((value.0 as usize, value.1 as usize))
    }
}

impl From<(usize, usize)> for Pos {
    fn from(value: (usize, usize)) -> Self {
        Self(value)
    }
}

impl From<(isize, isize)> for Pos {
    fn from(value: (isize, isize)) -> Self {
        if value.0 < 0 || value.1 < 0 {
            panic!("Cannot construct Pos from negative values")
        }
        Self((value.0 as usize, value.1 as usize))
    }
}

impl From<(i32, i32)> for Pos {
    fn from(value: (i32, i32)) -> Self {
        if value.0 < 0 || value.1 < 0 {
            panic!("Cannot construct Pos from negative values")
        }
        Self((value.0 as usize, value.1 as usize))
    }
}

impl std::fmt::Display for Pos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pos({}, {})", self.x(), self.y())
    }
}
