use array2d::Array2D;
use bevy_ecs::system::Resource;

use crate::{Dir, Pos};

#[derive(Resource, Debug, Clone)]
pub struct GridWorld<CellState> {
    pub grid: Array2D<CellState>,
}

impl<CellState: PassableCell> GridWorld<CellState> {
    pub fn new(width: usize, height: usize, fill: CellState) -> Self {
        Self {
            grid: Array2D::filled_with(fill, width, height),
        }
    }

    pub fn find_path_adj(
        &self,
        start: impl Into<Pos>,
        goal: impl Into<Pos>,
    ) -> Option<Vec<Pos>> {
        PathFinder::new(self.width(), self.height()).find_path(
            self,
            start.into(),
            goal.into(),
            true,
        )
    }

    pub fn find_path(
        &self,
        start: impl Into<Pos>,
        goal: impl Into<Pos>,
    ) -> Option<Vec<Pos>> {
        PathFinder::new(self.width(), self.height()).find_path(
            self,
            start.into(),
            goal.into(),
            false,
        )
    }

    pub fn try_get(&self, pos: Pos) -> Option<&CellState> {
        self.grid.get(pos.x(), pos.y())
    }

    pub fn get(&self, pos: Pos) -> &CellState {
        self.get_tuple(pos.x(), pos.y())
    }

    pub fn get_tuple(&self, x: usize, y: usize) -> &CellState {
        self.grid.get(x, y).unwrap()
    }

    pub fn get_mut(&mut self, pos: Pos) -> &mut CellState {
        self.get_tuple_mut(pos.x(), pos.y())
    }

    pub fn get_tuple_mut(&mut self, x: usize, y: usize) -> &mut CellState {
        self.grid.get_mut(x, y).unwrap()
    }

    pub fn set_tuple(&mut self, x: usize, y: usize, state: CellState) {
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

    /// Returns cells in ascending order of Manhattan distance from the given
    /// position. Within each distance layer, cells are ordered in clockwise
    /// direction starting from north.
    pub fn nearby(
        &self,
        pos: Pos,
        max_dist: usize,
    ) -> impl Iterator<Item = (Pos, &CellState)> + '_ {
        let width = self.width();
        let height = self.height();
        let (center_x, center_y) = pos.as_isize();
        let max_dist = max_dist as isize;

        // Create a vector to hold all positions and their distances
        let mut positions = Vec::new();

        // For each distance layer from 0 to max_dist
        for dist in 0..=max_dist {
            if dist == 0 {
                // Center point (distance 0)
                if center_x >= 0
                    && center_y >= 0
                    && center_x < width as isize
                    && center_y < height as isize
                {
                    positions.push((
                        Pos((center_x as usize, center_y as usize)),
                        dist,
                        0, // Direction doesn't matter for center
                    ));
                }
                continue;
            }

            // For each distance layer > 0, we need to generate the diamond
            // shape We'll do this by iterating through all possible
            // offsets that sum to the current distance

            // Top edge of diamond (moving right)
            for dx in 0..dist {
                let dy = dist - dx;
                let x = center_x + dx;
                let y = center_y + dy;
                if x >= 0 && y >= 0 && x < width as isize && y < height as isize
                {
                    positions.push((
                        Pos((x as usize, y as usize)),
                        dist as isize,
                        1, // North-East quadrant
                    ));
                }
            }

            // Right edge of diamond (moving down)
            for dy in 0..dist {
                let dx = dist - dy;
                let x = center_x + dx;
                let y = center_y - dy;
                if x >= 0 && y >= 0 && x < width as isize && y < height as isize
                {
                    positions.push((
                        Pos((x as usize, y as usize)),
                        dist as isize,
                        2, // South-East quadrant
                    ));
                }
            }

            // Bottom edge of diamond (moving left)
            for dx in 0..dist {
                let dy = dist - dx;
                let x = center_x - dx;
                let y = center_y - dy;
                if x >= 0 && y >= 0 && x < width as isize && y < height as isize
                {
                    positions.push((
                        Pos((x as usize, y as usize)),
                        dist as isize,
                        3, // South-West quadrant
                    ));
                }
            }

            // Left edge of diamond (moving up)
            for dy in 0..dist {
                let dx = dist - dy;
                let x = center_x - dx;
                let y = center_y + dy;
                if x >= 0 && y >= 0 && x < width as isize && y < height as isize
                {
                    positions.push((
                        Pos((x as usize, y as usize)),
                        dist as isize,
                        4, // North-West quadrant
                    ));
                }
            }
        }

        // Sort by distance first, then by direction for deterministic ordering
        positions.sort_by_key(|(_, dist, dir)| (*dist, *dir));

        // Map to the expected return format
        positions.into_iter().map(move |(pos, _, _)| {
            (pos, self.grid.get(pos.x(), pos.y()).unwrap())
        })
    }

    pub fn find_nearby(
        &self,
        pos: Pos,
        dist: usize,
        pred: impl Fn(&CellState) -> bool,
    ) -> Option<(Pos, &CellState)> {
        self.nearby(pos, dist).find(|(_pos, state)| pred(state))
    }

    pub fn find_adj(
        &self,
        pos: Pos,
        pred: impl Fn(&CellState) -> bool,
    ) -> Option<Dir> {
        self.nearby(pos, 1)
            .find(|(_pos, state)| pred(state))
            .and_then(|(target, _)| pos.dir_to(&target))
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

pub trait PassableCell: Clone + Default {
    fn is_blocked(&self) -> bool;
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

    pub fn find_path<T: PassableCell>(
        &mut self,
        grid: &GridWorld<T>,
        start: Pos,
        goal: Pos,
        use_adjacent: bool,
    ) -> Option<Vec<Pos>> {
        use std::{
            cmp::Ordering,
            collections::{BinaryHeap, HashSet},
        };
        let start = start.0;
        let goal = goal.0;

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
            if use_adjacent && Pos(current).is_adjacent(&Pos(goal)) {
                return Some(self.reconstruct_path(current));
            } else if current == goal {
                return Some(self.reconstruct_path(current));
            }

            if !closed_set.insert(current) {
                continue;
            }

            // Check neighbors (using nearby with distance 1)
            for (pos, state) in grid.nearby(Pos(current), 1) {
                // Skip if neighbor is blocked or already evaluated
                if state.is_blocked() || closed_set.contains(&pos.0) {
                    continue;
                }
                let (nx, ny) = pos.0;

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

    #[derive(Clone, Default)]
    struct Cell {
        is_blocked: bool,
    }

    impl PassableCell for Cell {
        fn is_blocked(&self) -> bool {
            self.is_blocked
        }
    }

    fn create_test_grid() -> GridWorld<Cell> {
        // Create a 5x5 grid for testing
        let mut grid = GridWorld::new(5, 5, Cell::default());

        // Fill with unique values to make testing easier
        for x in 0..5 {
            for y in 0..5 {
                grid.set_tuple(x, y, Default::default());
            }
        }
        grid
    }

    #[test]
    fn test_nearby_distance_0() {
        let grid = create_test_grid();
        let nearby: Vec<_> = grid.nearby(Pos((2, 2)), 0).collect();
        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0].0, Pos((2, 2)));
    }

    #[test]
    fn test_nearby_distance_1() {
        let grid = create_test_grid();
        let positions: Vec<_> =
            grid.nearby(Pos((2, 2)), 1).map(|(pos, _)| pos).collect();
        // Should include center + 4 adjacent cells
        assert_eq!(positions.len(), 5);

        assert!(positions.contains(&Pos((2, 2)))); // center
        assert!(positions.contains(&Pos((1, 2)))); // left
        assert!(positions.contains(&Pos((3, 2)))); // right
        assert!(positions.contains(&Pos((2, 1)))); // down
        assert!(positions.contains(&Pos((2, 3)))); // up
    }

    #[test]
    fn test_nearby_corner() {
        let grid = create_test_grid();
        let nearby: Vec<_> = grid.nearby(Pos((0, 0)), 1).collect();
        // Corner should only have 3 cells (including itself)
        assert_eq!(nearby.len(), 3);

        let positions: Vec<_> = nearby.iter().map(|(pos, _)| *pos).collect();
        assert!(positions.contains(&Pos((0, 0)))); // corner
        assert!(positions.contains(&Pos((1, 0)))); // right
        assert!(positions.contains(&Pos((0, 1)))); // up
    }

    #[test]
    fn test_nearby_large_distance() {
        let grid = create_test_grid();
        // Distance larger than grid size
        let nearby: Vec<_> = grid.nearby(Pos((2, 2)), 10).collect();
        // Should include all cells in 5x5 grid
        assert_eq!(nearby.len(), 25);

        assert!(nearby
            .is_sorted_by_key(|(pos, _)| pos.manhattan_distance(&Pos((2, 2)))));
    }

    #[test]
    fn test_pathfinding_simple() {
        let mut grid = GridWorld::new(5, 5, Default::default());
        let mut pathfinder = PathFinder::new(5, 5);

        // Test simple path without obstacles
        let path = pathfinder
            .find_path(&grid, Pos((0, 0)), Pos((2, 2)), false)
            .expect("Should find a path");
        assert_eq!(path.len(), 5); // Should be [(0,0), (1,0), (2,0), (2,1), (2,2)]

        // Add some obstacles and test path around them
        grid.set_tuple(1, 1, Cell { is_blocked: true });
        let path = pathfinder
            .find_path(&grid, Pos((0, 0)), Pos((2, 2)), false)
            .expect("Should find a path");
        assert!(!path.is_empty()); // Should find a path around the obstacle
    }

    #[test]
    fn test_pathfinding_blocked() {
        let mut grid = GridWorld::new(3, 3, Default::default());
        let mut pathfinder = PathFinder::new(3, 3);

        let blocked = Cell { is_blocked: true };

        // Create a wall of blocked cells
        grid.set_tuple(1, 0, blocked.clone());
        grid.set_tuple(1, 1, blocked.clone());
        grid.set_tuple(1, 2, blocked);

        // Try to find path through wall
        let path = pathfinder.find_path(&grid, Pos((0, 1)), Pos((2, 1)), false);
        assert!(path.is_none()); // Should not find a path
    }

    #[test]
    fn test_nearby_in_order_distance_0() {
        let grid = create_test_grid();
        let nearby: Vec<_> = grid.nearby(Pos((2, 2)), 0).collect();
        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0].0, Pos((2, 2)));
    }

    #[test]
    fn test_nearby_in_order_distance_1() {
        let grid = create_test_grid();
        let positions: Vec<_> =
            grid.nearby(Pos((2, 2)), 1).map(|(pos, _)| pos).collect();
        // Should include center + 4 adjacent cells
        assert_eq!(positions.len(), 5);

        // Check that the center is first
        assert_eq!(positions[0], Pos((2, 2)));

        // Check that the other positions are at distance 1 and in clockwise
        // order North, East, South, West
        assert!(positions.contains(&Pos((2, 3)))); // North
        assert!(positions.contains(&Pos((3, 2)))); // East
        assert!(positions.contains(&Pos((2, 1)))); // South
        assert!(positions.contains(&Pos((1, 2)))); // West

        // Verify the ordering: center, then distance 1 in clockwise order
        assert_eq!(positions[0], Pos((2, 2))); // Center

        // The next 4 should be in clockwise order from North
        let distance_1_positions = &positions[1..5];
        let north_idx = distance_1_positions
            .iter()
            .position(|&p| p == Pos((2, 3)))
            .unwrap();
        let east_idx = distance_1_positions
            .iter()
            .position(|&p| p == Pos((3, 2)))
            .unwrap();
        let south_idx = distance_1_positions
            .iter()
            .position(|&p| p == Pos((2, 1)))
            .unwrap();
        let west_idx = distance_1_positions
            .iter()
            .position(|&p| p == Pos((1, 2)))
            .unwrap();

        // Check clockwise ordering (this might need adjustment based on your
        // exact implementation)
        assert!(north_idx < east_idx);
        assert!(east_idx < south_idx);
        assert!(south_idx < west_idx);
    }

    #[test]
    fn test_nearby_in_order_distance_2() {
        let grid = create_test_grid();
        let positions: Vec<_> =
            grid.nearby(Pos((2, 2)), 2).map(|(pos, _)| pos).collect();

        // Should include center + 4 adjacent + 8 at distance 2
        assert_eq!(positions.len(), 13);

        // Verify that positions are ordered by distance
        let manhattan_distances: Vec<_> = positions
            .iter()
            .map(|pos| {
                let dx = (pos.x() as isize - 2).abs() as usize;
                let dy = (pos.y() as isize - 2).abs() as usize;
                dx + dy
            })
            .collect();

        // Check that distances are non-decreasing
        for i in 1..manhattan_distances.len() {
            assert!(manhattan_distances[i] >= manhattan_distances[i - 1]);
        }

        // Check that we have the right number of cells at each distance
        assert_eq!(manhattan_distances.iter().filter(|&&d| d == 0).count(), 1); // 1 at distance 0
        assert_eq!(manhattan_distances.iter().filter(|&&d| d == 1).count(), 4); // 4 at distance 1
        assert_eq!(manhattan_distances.iter().filter(|&&d| d == 2).count(), 8); // 8
                                                                                // at
                                                                                // distance
                                                                                // 2
    }

    #[test]
    fn test_nearby_in_order_corner() {
        let grid = create_test_grid();
        let positions: Vec<_> =
            grid.nearby(Pos((0, 0)), 1).map(|(pos, _)| pos).collect();

        // Corner should only have 3 cells (including itself)
        assert_eq!(positions.len(), 3);

        // Check ordering: center first, then by distance
        assert_eq!(positions[0], Pos((0, 0))); // Corner itself

        // The next 2 should be at distance 1
        let distance_1_positions = &positions[1..3];
        assert!(distance_1_positions.contains(&Pos((1, 0)))); // East
        assert!(distance_1_positions.contains(&Pos((0, 1)))); // North
    }
}
