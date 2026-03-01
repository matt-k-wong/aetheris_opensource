pub mod classic_engine;
pub mod visual_test;

use crate::infrastructure::PerformanceProfiler;

use std::any::Any;

/// Abstract entity representation for rendering without engine-specific knowledge
pub trait AetherisEntity {
    fn position(&self) -> glam::Vec2;
    fn z(&self) -> f32;
    fn get_sprites(&self, viewer_pos: glam::Vec2, frame_count: u64) -> Vec<String>;
    fn should_draw(&self) -> bool {
        true
    }
    fn is_spectral(&self) -> bool {
        false
    }
}

/// Abstract player representation for rendering
pub trait AetherisPlayer {
    fn position(&self) -> glam::Vec2;
    fn z(&self) -> f32;
    fn angle(&self) -> f32;
    fn fov(&self) -> f32;
    fn damage_flash(&self) -> f32;
    fn bonus_flash(&self) -> f32;
    fn invuln_timer(&self) -> u32;
    fn radsuit_timer(&self) -> u32;
}

/// The VisualBridge trait defines the interface for any rendering engine.
/// It separates the simulation state from the actual visualization passes.
pub trait VisualBridge {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Renders the 3D world (walls, sectors) and abstract entities.
    fn render_scene(
        &mut self,
        world: &crate::simulation::WorldState,
        entities: &[&dyn AetherisEntity],
        player: &dyn AetherisPlayer,
        profiler: &mut PerformanceProfiler,
    ) -> anyhow::Result<()>;

    /// Renders the 2D heads-up display (health, ammo, face).
    fn render_hud(&mut self, world: &crate::simulation::WorldState) -> anyhow::Result<()>;

    /// Renders the 2D automap.
    fn render_automap(&mut self, world: &crate::simulation::WorldState) -> anyhow::Result<()>;

    /// Handles input for renderer-specific features (zoom, pan).
    fn handle_input(&mut self, actions: &std::collections::HashSet<crate::simulation::GameAction>);

    /// Finalizes the frame and presents it to the window.
    fn present(&mut self) -> anyhow::Result<()>;

    /// Called when a new map is loaded to allow renderers to rebuild static geometry.
    fn on_map_loaded(&mut self, world: &crate::simulation::WorldState);

    /// Handles window resize events.
    /// If resize_buffer is true, the internal render resolution is also changed.
    fn handle_resize(&mut self, width: u32, height: u32, resize_buffer: bool);

    /// Captures the current frame and saves it to a file.
    fn take_screenshot(&mut self, path: &str) -> anyhow::Result<()>;
}
