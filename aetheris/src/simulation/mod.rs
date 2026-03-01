pub mod engine;

// Re-export everything for backwards compatibility
pub use engine::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use std::collections::HashSet;

    #[test]
    fn test_line_intersection() {
        let hit = WorldState::intersect(
            Vec2::ZERO,
            Vec2::new(10., 0.),
            Vec2::new(5., -5.),
            Vec2::new(5., 5.),
        );
        assert_eq!(hit.unwrap(), Vec2::new(5., 0.));
    }
}
