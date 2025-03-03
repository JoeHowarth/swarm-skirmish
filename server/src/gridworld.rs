use array2d::Array2D;
use bevy::prelude::*;
use swarm_lib::Pos;

use crate::core::{CellKind, CellState};

#[derive(Resource)]
pub struct GridWorld {
    pub grid: Array2D<CellState>,
}

impl GridWorld {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            grid: Array2D::filled_with(CellState::default(), width, height),
        }
    }

    pub fn find_path(
        &self,
        start: impl Into<Pos>,
        goal: impl Into<Pos>,
    ) -> Option<Vec<Pos>> {
        PathFinder::new(self.width(), self.height())
            .find_path(self, start, goal)
    }

    pub fn get_pos(&self, pos: Pos) -> CellState {
        self.get(pos.x(), pos.y())
    }

    pub fn get(&self, x: usize, y: usize) -> CellState {
        *self.grid.get(x, y).unwrap()
    }

    pub fn get_pos_mut(&mut self, pos: Pos) -> &mut CellState {
        self.get_mut(pos.x(), pos.y())
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut CellState {
        self.grid.get_mut(x, y).unwrap()
    }

    pub fn set(&mut self, x: usize, y: usize, state: CellState) {
        let _ = self.grid.set(x, y, state);
    }

    pub fn width(&self) -> usize {
        self.grid.num_rows()
    }

    pub fn height(&self) -> usize {
        self.grid.num_columns()
    }

    pub fn iter(&self) -> impl Iterator<Item = ((usize, usize), &CellState)> {
        self.grid.enumerate_row_major()
    }

    pub fn nearby(
        &self,
        x: usize,
        y: usize,
        dist: usize,
    ) -> impl Iterator<Item = ((usize, usize), &CellState)> {
        let width = self.width();
        let height = self.height();

        // Convert to isize for easier boundary calculations
        let x = x as isize;
        let y = y as isize;
        let dist = dist as isize;

        // Create ranges for x and y coordinates within distance
        let x_range = (x - dist).max(0)..(x + dist + 1).min(width as isize);
        let y_range = (y - dist).max(0)..(y + dist + 1).min(height as isize);

        // Create iterator over coordinates within the ranges
        let iter = x_range.flat_map(move |curr_x| {
            y_range.clone().filter_map(move |curr_y| {
                // Calculate Manhattan distance
                let dx = (curr_x - x).abs();
                let dy = (curr_y - y).abs();

                // Only include cells within the Manhattan distance
                if dx + dy <= dist {
                    // Convert back to usize for grid access
                    Some((
                        (curr_x as usize, curr_y as usize),
                        self.grid
                            .get(curr_x as usize, curr_y as usize)
                            .unwrap(),
                    ))
                } else {
                    None
                }
            })
        });

        iter
    }

    pub fn in_bounds_i(&self, pos: (isize, isize)) -> bool {
        pos.0 >= 0
            && pos.1 >= 0
            && pos.0 < self.width() as isize
            && pos.1 < self.height() as isize
    }

    pub fn in_bounds(&self, pos: &Pos) -> bool {
        pos.x() < self.width() && pos.y() < self.height()
    }
}

#[derive(Debug)]
pub struct PathFinder {
    f_scores: Array2D<i32>,
    g_scores: Array2D<i32>,
    came_from: Array2D<Option<(usize, usize)>>,
    width: usize,
    height: usize,
}

impl PathFinder {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            f_scores: Array2D::filled_with(i32::MAX, width, height),
            g_scores: Array2D::filled_with(i32::MAX, width, height),
            came_from: Array2D::filled_with(None, width, height),
            width,
            height,
        }
    }

    fn manhattan_distance(from: (usize, usize), to: (usize, usize)) -> i32 {
        let dx = (from.0 as isize - to.0 as isize).abs() as i32;
        let dy = (from.1 as isize - to.1 as isize).abs() as i32;
        dx + dy
    }

    fn reconstruct_path(&self, current: (usize, usize)) -> Vec<Pos> {
        let mut path = vec![Pos(current)];
        let mut current = current;

        while let Some(pos) = self.came_from.get(current.0, current.1).unwrap()
        {
            current = *pos;
            path.push(Pos(current));
        }

        path.reverse();
        path
    }

    pub fn find_path(
        &mut self,
        grid: &GridWorld,
        start: impl Into<Pos>,
        goal: impl Into<Pos>,
    ) -> Option<Vec<Pos>> {
        use std::{
            cmp::Ordering,
            collections::{BinaryHeap, HashSet},
        };
        let start = start.into().0;
        let goal = goal.into().0;

        #[derive(Eq, PartialEq)]
        struct Node {
            pos: (usize, usize),
            f_score: i32,
        }

        impl Ord for Node {
            fn cmp(&self, other: &Self) -> Ordering {
                // Reverse ordering for min-heap
                other.f_score.cmp(&self.f_score)
            }
        }

        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        // Reset pathfinding data
        self.f_scores = Array2D::filled_with(i32::MAX, self.width, self.height);
        self.g_scores = Array2D::filled_with(i32::MAX, self.width, self.height);
        self.came_from = Array2D::filled_with(None, self.width, self.height);

        let mut open_set = BinaryHeap::new();
        let mut closed_set = HashSet::new();

        // Initialize start node
        self.g_scores.set(start.0, start.1, 0).unwrap();
        self.f_scores
            .set(start.0, start.1, Self::manhattan_distance(start, goal))
            .unwrap();

        open_set.push(Node {
            pos: start,
            f_score: *self.f_scores.get(start.0, start.1).unwrap(),
        });

        while let Some(Node { pos: current, .. }) = open_set.pop() {
            if current == goal {
                return Some(self.reconstruct_path(current));
            }

            if !closed_set.insert(current) {
                continue;
            }

            // Check neighbors (using nearby with distance 1)
            for ((nx, ny), state) in grid.nearby(current.0, current.1, 1) {
                // Skip if neighbor is blocked or already evaluated
                if state.kind == CellKind::Blocked
                    || closed_set.contains(&(nx, ny))
                {
                    continue;
                }

                // Calculate tentative g_score
                let tentative_g_score =
                    self.g_scores.get(current.0, current.1).unwrap() + 1;

                if tentative_g_score < *self.g_scores.get(nx, ny).unwrap() {
                    // This path is better than any previous one
                    self.came_from.set(nx, ny, Some(current)).unwrap();
                    self.g_scores.set(nx, ny, tentative_g_score).unwrap();
                    let f_score = tentative_g_score
                        + Self::manhattan_distance((nx, ny), goal);
                    self.f_scores.set(nx, ny, f_score).unwrap();

                    open_set.push(Node {
                        pos: (nx, ny),
                        f_score,
                    });
                }
            }
        }

        None // No path found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_grid() -> GridWorld {
        // Create a 5x5 grid for testing
        let mut grid = GridWorld::new(5, 5);

        // Fill with unique values to make testing easier
        for x in 0..5 {
            for y in 0..5 {
                grid.set(x, y, CellState::default());
            }
        }
        grid
    }

    #[test]
    fn test_nearby_distance_0() {
        let grid = create_test_grid();
        let nearby: Vec<_> = grid.nearby(2, 2, 0).collect();
        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0].0, (2, 2));
    }

    #[test]
    fn test_nearby_distance_1() {
        let grid = create_test_grid();
        let nearby: Vec<_> = grid.nearby(2, 2, 1).collect();
        // Should include center + 4 adjacent cells
        assert_eq!(nearby.len(), 5);

        let positions: Vec<_> =
            nearby.iter().map(|((x, y), _)| (*x, *y)).collect();
        assert!(positions.contains(&(2, 2))); // center
        assert!(positions.contains(&(1, 2))); // left
        assert!(positions.contains(&(3, 2))); // right
        assert!(positions.contains(&(2, 1))); // down
        assert!(positions.contains(&(2, 3))); // up
    }

    #[test]
    fn test_nearby_corner() {
        let grid = create_test_grid();
        let nearby: Vec<_> = grid.nearby(0, 0, 1).collect();
        // Corner should only have 3 cells (including itself)
        assert_eq!(nearby.len(), 3);

        let positions: Vec<_> =
            nearby.iter().map(|((x, y), _)| (*x, *y)).collect();
        assert!(positions.contains(&(0, 0))); // corner
        assert!(positions.contains(&(1, 0))); // right
        assert!(positions.contains(&(0, 1))); // up
    }

    #[test]
    fn test_nearby_large_distance() {
        let grid = create_test_grid();
        // Distance larger than grid size
        let nearby: Vec<_> = grid.nearby(2, 2, 10).collect();
        // Should include all cells in 5x5 grid
        assert_eq!(nearby.len(), 25);
    }

    #[test]
    fn test_pathfinding_simple() {
        let mut grid = GridWorld::new(5, 5);
        let mut pathfinder = PathFinder::new(5, 5);

        // Test simple path without obstacles
        let path = pathfinder
            .find_path(&grid, (0, 0), (2, 2))
            .expect("Should find a path");
        assert_eq!(path.len(), 5); // Should be [(0,0), (1,0), (2,0), (2,1), (2,2)]

        // Add some obstacles and test path around them
        grid.set(1, 1, CellState::blocked());
        let path = pathfinder
            .find_path(&grid, (0, 0), (2, 2))
            .expect("Should find a path");
        assert!(path.len() > 0); // Should find a path around the obstacle
    }

    #[test]
    fn test_pathfinding_blocked() {
        let mut grid = GridWorld::new(3, 3);
        let mut pathfinder = PathFinder::new(3, 3);

        // Create a wall of blocked cells
        grid.set(1, 0, CellState::blocked());
        grid.set(1, 1, CellState::blocked());
        grid.set(1, 2, CellState::blocked());

        // Try to find path through wall
        let path = pathfinder.find_path(&grid, (0, 1), (2, 1));
        assert!(path.is_none()); // Should not find a path
    }
}
