use std::ops::{Add, Sub};

use bevy_ecs::component::Component;
use bevy_math::{IVec2, UVec2, Vec2};
use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(
    Debug,
    Copy,
    Clone,
    Serialize,
    Deserialize,
    strum_macros::EnumIter,
    strum_macros::FromRepr,
    strum_macros::EnumDiscriminants,
)]
#[repr(u8)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display,
)]
pub enum Team {
    Player,
    Enemy,
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
        match deltas {
            (0, 1) => Some(Dir::Up),
            (0, -1) => Some(Dir::Down),
            (-1, 0) => Some(Dir::Left),
            (1, 0) => Some(Dir::Right),
            _ => None,
        }
    }
}

////////////////////////////
///////// Energy ///////////
///////// ///////////////////
#[derive(
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

impl From<u32> for Energy {
    fn from(value: u32) -> Self {
        Energy(value)
    }
}

impl std::ops::Deref for Energy {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Energy {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Sub<i32> for Energy {
    type Output = Option<Energy>;

    fn sub(self, rhs: i32) -> Self::Output {
        if rhs < 0 {
            return Some(Energy(self.0 + rhs.unsigned_abs()));
        }

        let rhs_u32 = rhs as u32;
        if self.0 >= rhs_u32 {
            Some(Energy(self.0 - rhs_u32))
        } else {
            None
        }
    }
}

impl Add<i32> for Energy {
    type Output = Energy;

    fn add(self, rhs: i32) -> Self::Output {
        if rhs < 0 {
            let rhs_u32 = rhs.unsigned_abs();
            if self.0 >= rhs_u32 {
                Energy(self.0 - rhs_u32)
            } else {
                Energy(0)
            }
        } else {
            Energy(self.0 + rhs as u32)
        }
    }
}

impl Add for Energy {
    type Output = Energy;

    fn add(self, rhs: Self) -> Self::Output {
        Energy(self.0 + rhs.0)
    }
}

impl Sub for Energy {
    type Output = Option<Energy>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.0 >= rhs.0 {
            Some(Energy(self.0 - rhs.0))
        } else {
            None
        }
    }
}

impl std::fmt::Display for Energy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Energy({})", self.0)
    }
}

////////////////////////////
/////////// Pos ////////////
/////////// /////////////////

#[derive(Debug, Copy, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct Pos(pub (usize, usize));

impl From<Pos> for Vec2 {
    fn from(val: Pos) -> Self {
        Vec2::new(val.x() as f32, val.y() as f32)
    }
}

impl Add<Dir> for Pos {
    type Output = Option<Pos>;

    fn add(self, dir: Dir) -> Self::Output {
        let (x, y) = self.as_isize();
        let (dx, dy) = dir.to_deltas();

        let new_x = x + dx;
        let new_y = y + dy;

        // Ensure coordinates are non-negative before creating Pos
        if new_x < 0 || new_y < 0 {
            None
        } else {
            Some(Pos::from((new_x as usize, new_y as usize)))
        }
    }
}

impl Add<(isize, isize)> for Pos {
    type Output = (isize, isize);

    fn add(self, (rhs_x, rhs_y): (isize, isize)) -> Self::Output {
        let (self_x, self_y) = self.as_isize();
        (self_x + rhs_x, self_y + rhs_y)
    }
}

impl Add for Pos {
    type Output = (isize, isize);

    fn add(self, rhs: Self) -> Self::Output {
        let (self_x, self_y) = self.as_isize();
        let (rhs_x, rhs_y) = rhs.as_isize();
        (self_x + rhs_x, self_y + rhs_y)
    }
}

impl Sub for Pos {
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
