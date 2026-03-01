use crate::infrastructure::PerformanceProfiler;
use crate::presentation::VisualBridge;
use crate::simulation::{LineDefinition, Sector, WorldState};
use glam::Vec2;
use pixels::{Pixels, SurfaceTexture};
use rand::Rng;
use winit::window::Window;

pub struct ClassicSoftwareEngine {
    pixels: Pixels,
    width: u32,
    height: u32,
    melt_state: Option<MeltState>,
    prev_frame: Vec<u8>,
    map_scale: f32,
}

struct MeltState {
    column_offsets: Vec<i32>,
    speed: Vec<i32>,
    progress: u32,
}

impl MeltState {
    fn new(width: u32, _height: u32) -> Self {
        let mut rng = rand::thread_rng();
        let mut column_offsets = Vec::with_capacity(width as usize);
        let mut speed = Vec::with_capacity(width as usize);
        for _ in 0..width {
            column_offsets.push(-(rng.gen_range(0..16) as i32));
            speed.push(rng.gen_range(1..4) as i32);
        }
        Self {
            column_offsets,
            speed,
            progress: 0,
        }
    }
    fn update(&mut self, height: u32) -> bool {
        let mut active = false;
        for i in 0..self.column_offsets.len() {
            if self.column_offsets[i] < height as i32 {
                self.column_offsets[i] += self.speed[i];
                self.speed[i] += 1;
                active = true;
            }
        }
        self.progress += 1;
        active
    }
    fn apply(&self, frame: &mut [u8], prev_frame: &[u8], width: u32, height: u32) {
        for x in 0..width as usize {
            let offset = self.column_offsets[x];
            if offset <= 0 {
                for y in 0..height as usize {
                    let off = (y * width as usize + x) * 4;
                    frame[off..off + 4].copy_from_slice(&prev_frame[off..off + 4]);
                }
            } else if offset < height as i32 {
                for y in (offset as usize..height as usize).rev() {
                    let dst_off = (y * width as usize + x) * 4;
                    let src_off = ((y - offset as usize) * width as usize + x) * 4;
                    frame[dst_off..dst_off + 4].copy_from_slice(&prev_frame[src_off..src_off + 4]);
                }
            }
        }
    }
}

struct RenderContext<'a> {
    width: u32,
    height: u32,
    stride: u32,
    x_off: u32,
    y_off: u32,
    frame: &'a mut [u8],
    upper_clip: Vec<i32>,
    lower_clip: Vec<i32>,
    depth_buffer: Vec<f32>,
    depth_buffer_2d: Vec<f32>,
    rem: u32,
    eye_z: f32,
    fov: f32,
    world: &'a WorldState,
    gamma: f32,
}

impl ClassicSoftwareEngine {
    pub fn new(window: &Window, width: u32, height: u32) -> anyhow::Result<Self> {
        let surface_texture = SurfaceTexture::new(width, height, window);
        let pixels = Pixels::new(width, height, surface_texture)?;
        let frame_size = (width * height * 4) as usize;
        Ok(Self {
            pixels,
            width,
            height,
            melt_state: None,
            prev_frame: vec![0; frame_size],
            map_scale: 0.15,
        })
    }

    fn draw_wall_column(
        ctx: &mut RenderContext,
        x: u32,
        dist: f32,
        tex_name: &str,
        low: f32,
        high: f32,
        light: f32,
        u: f32,
        y_offset: f32,
    ) {
        let tex = if let Some(t) = ctx.world.textures.get(tex_name) {
            t
        } else {
            return;
        };
        let center = ctx.height as f32 / 2.0;
        let scale = (ctx.height as f32 / dist).min(2000.0);
        let wall_top_y = (center - (high - ctx.eye_z) * scale) as i32;
        let wall_bot_y = (center - (low - ctx.eye_z) * scale) as i32;

        let draw_start = wall_top_y.max(ctx.upper_clip[x as usize]);
        let draw_end = wall_bot_y.min(ctx.lower_clip[x as usize]);

        if draw_start >= draw_end {
            return;
        }

        let tx = (u as i32).rem_euclid(tex.width as i32) as u32;
        let palette = &ctx.world.palettes[ctx.world.current_palette_idx];
        let cmap = &ctx.world.colormap[((((1.0 - (light * ctx.gamma).min(1.0)) * 31.0)
            as usize)
            .min(31)
            + (dist as usize / 64))
            .min(31)
            * 256..];

        for y in draw_start..draw_end {
            let world_y_dist = (y - wall_top_y) as f32 / scale;
            let ty = ((world_y_dist + y_offset) as i32).rem_euclid(tex.height as i32) as u32;
            let p_idx = tex.pixels_indexed[(ty * tex.width + tx) as usize];
            if p_idx != -1 {
                let off = ((y + ctx.y_off as i32) as usize * ctx.stride as usize
                    + (x as i32 + ctx.x_off as i32) as usize)
                    * 4;
                if off + 3 < ctx.frame.len() {
                    let poff = cmap[p_idx as usize] as usize * 3;
                    ctx.frame[off..off + 3].copy_from_slice(&palette[poff..poff + 3]);
                    ctx.frame[off + 3] = 255;
                    ctx.depth_buffer_2d[(y + ctx.y_off as i32) as usize * ctx.stride as usize
                        + (x as i32 + ctx.x_off as i32) as usize] = dist;
                }
            }
        }
    }

    fn draw_flats_column(
        ctx: &mut RenderContext,
        x: u32,
        y_start: i32,
        y_end: i32,
        sector: &Sector,
        ray_dir: glam::Vec2,
        angle_offset: f32,
        is_ceiling: bool,
    ) {
        if y_start >= y_end {
            return;
        }
        let center = ctx.height as f32 / 2.0;
        let cos_offset = angle_offset.cos();
        let tex_name = if is_ceiling {
            &sector.texture_ceiling
        } else {
            &sector.texture_floor
        };
        if is_ceiling && tex_name == "F_SKY1" {
            Self::draw_sky_column(ctx, x, y_start, y_end, ray_dir);
            return;
        }
        let tex = if let Some(t) = ctx.world.textures.get(tex_name) {
            Some(t)
        } else {
            log::warn!("Missing flat texture: {}, falling back to DEBUG", tex_name);
            ctx.world.textures.get("DEBUG")
        };
        let height_diff = if is_ceiling {
            sector.ceiling_height - ctx.eye_z
        } else {
            ctx.eye_z - sector.floor_height
        };
        for y in y_start..y_end {
            if y < 0 || y >= ctx.height as i32 {
                continue;
            }
            let div = if is_ceiling {
                center - y as f32
            } else {
                y as f32 - center
            };
            if div <= 0.0 {
                continue;
            }
            let z = (height_diff * ctx.height as f32 * 0.5) / div;
            if z <= 0.0 || z > 4000.0 {
                continue;
            }
            let d = z / cos_offset;
            let p_x = ctx.world.player.position.x + ray_dir.x * d;
            let p_y = ctx.world.player.position.y + ray_dir.y * d;
            let off = ((y + ctx.y_off as i32) as usize * ctx.stride as usize
                + (x + ctx.x_off) as usize)
                * 4;
            let mut p_idx = -1i16;
            if let Some(t) = tex {
                let tx = (p_x as i32).rem_euclid(t.width as i32) as u32;
                let ty = (p_y as i32).rem_euclid(t.height as i32) as u32;
                p_idx = t.pixels_indexed[(ty * t.width + tx) as usize];
            }
            if p_idx != -1 {
                let cmap = &ctx.world.colormap[((((1.0 - (sector.light_level * ctx.gamma).min(1.0))
                    * 31.0) as usize)
                    .min(31)
                    + (d as usize / 64))
                    .min(31) * 256..];
                let poff = cmap[p_idx as usize] as usize * 3;
                ctx.frame[off..off + 3].copy_from_slice(
                    &ctx.world.palettes[ctx.world.current_palette_idx][poff..poff + 3],
                );
                ctx.frame[off + 3] = 255;
            }
        }
    }

    fn draw_sky_column(
        ctx: &mut RenderContext,
        x: u32,
        y_start: i32,
        y_end: i32,
        ray_dir: glam::Vec2,
    ) {
        if let Some(tex) = ctx.world.textures.get("SKY1") {
            let angle = ray_dir.y.atan2(ray_dir.x);
            let u = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
            let tx = (u * 4.0 * tex.width as f32) as u32 % tex.width;
            let palette = &ctx.world.palettes[ctx.world.current_palette_idx];
            for y in y_start..y_end {
                if y < 0 || y >= ctx.height as i32 {
                    continue;
                }
                let ty = (y as f32 / ctx.height as f32 * tex.height as f32) as u32 % tex.height;
                let off = ((y + ctx.y_off as i32) as usize * ctx.stride as usize
                    + (x + ctx.x_off) as usize)
                    * 4;
                let p_idx = tex.pixels_indexed[(ty * tex.width + tx) as usize];
                if p_idx != -1 {
                    let poff = p_idx as usize * 3;
                    ctx.frame[off..off + 3].copy_from_slice(&palette[poff..poff + 3]);
                    ctx.frame[off + 3] = 255;
                }
            }
        }
    }

    fn render_bsp_node(ctx: &mut RenderContext, node_idx: u16) {
        if ctx.rem == 0 {
            return;
        }
        if node_idx & 0x8000 != 0 {
            Self::render_subsector(ctx, (node_idx & 0x7FFF) as usize);
            return;
        }
        let node = &ctx.world.nodes[node_idx as usize];
        let side = (ctx.world.player.position.x - node.x) * node.dy
            - (ctx.world.player.position.y - node.y) * node.dx;
        if side <= 0.0 {
            Self::render_bsp_node(ctx, node.child_left);
            Self::render_bsp_node(ctx, node.child_right);
        } else {
            Self::render_bsp_node(ctx, node.child_right);
            Self::render_bsp_node(ctx, node.child_left);
        }
    }

    fn render_subsector(ctx: &mut RenderContext, sub_idx: usize) {
        let sub = &ctx.world.subsectors[sub_idx];
        for i in 0..sub.seg_count {
            let seg = &ctx.world.segs[sub.first_seg_idx + i];
            let line = &ctx.world.linedefs[seg.linedef_idx];
            let sidedef = if seg.side == 0 {
                line.front.as_ref()
            } else {
                line.back.as_ref()
            };
            let ox = seg.offset + sidedef.map(|s| s.x_offset).unwrap_or(0.0);
            let oy = sidedef.map(|s| s.y_offset).unwrap_or(0.0);

            let s = ctx.world.vertices[seg.start_idx];
            let e = ctx.world.vertices[seg.end_idx];

            let (x_start, x_end, dist_start, dist_end, u_start, u_end, sx_s, sx_e) =
                Self::calculate_seg_span(ctx, s, e, ox);
            if x_start >= x_end {
                continue;
            }
            let (front, back_sector_opt) = if seg.side == 0 {
                (
                    &ctx.world.sectors[line.sector_front.unwrap_or(0)],
                    line.sector_back.map(|s| &ctx.world.sectors[s]),
                )
            } else {
                (
                    &ctx.world.sectors[line.sector_back.unwrap_or(0)],
                    line.sector_front.map(|s| &ctx.world.sectors[s]),
                )
            };
            if let Some(back) = back_sector_opt {
                Self::draw_portal_span(
                    ctx, x_start, x_end, dist_start, dist_end, u_start, u_end, line, front, back,
                    oy, sx_s, sx_e, sidedef,
                );
            } else {
                Self::draw_solid_wall_span(
                    ctx, x_start, x_end, dist_start, dist_end, u_start, u_end, line, front, oy,
                    sx_s, sx_e, sidedef,
                );
            }
        }
    }

    fn calculate_seg_span(
        ctx: &RenderContext,
        s: Vec2,
        e: Vec2,
        u_offset: f32,
    ) -> (i32, i32, f32, f32, f32, f32, f32, f32) {
        let (p_pos, p_angle, screen_w) = (
            ctx.world.player.position,
            ctx.world.player.angle,
            ctx.width as f32,
        );

        let (dx1, dy1) = (s.x - p_pos.x, s.y - p_pos.y);
        let (dx2, dy2) = (e.x - p_pos.x, e.y - p_pos.y);

        // Standard 2D rotation matching Doom's coordinate system (0=East, 90=North)
        // Y becomes depth Z, X becomes horizontal screen X
        let rz1 = dx1 * p_angle.cos() + dy1 * p_angle.sin();
        let rx1 = dx1 * p_angle.sin() - dy1 * p_angle.cos();
        let rz2 = dx2 * p_angle.cos() + dy2 * p_angle.sin();
        let rx2 = dx2 * p_angle.sin() - dy2 * p_angle.cos();

        let near = 0.01f32;
        let line_len = (e - s).length();
        let (mut x1, mut z1, mut x2, mut z2, mut u1, mut u2) =
            (rx1, rz1, rx2, rz2, u_offset, u_offset + line_len);

        if z1 < near && z2 < near {
            return (0, 0, 0., 0., 0., 0., 0., 0.);
        } else if z1 < near {
            let intersect_scale = (near - z1) / (z2 - z1);
            x1 = x1 + intersect_scale * (x2 - x1);
            z1 = near;
            u1 = u1 + intersect_scale * (u2 - u1);
        } else if z2 < near {
            let intersect_scale = (near - z2) / (z1 - z2);
            x2 = x2 + intersect_scale * (x1 - x2);
            z2 = near;
            u2 = u2 + intersect_scale * (u1 - u2);
        }

        let sx1 =
            ctx.width as f32 / 2.0 + (x1 * ctx.width as f32 * 0.5) / (z1 * (ctx.fov * 0.5).tan());
        let sx2 =
            ctx.width as f32 / 2.0 + (x2 * ctx.width as f32 * 0.5) / (z2 * (ctx.fov * 0.5).tan());

        (
            sx1.round() as i32,
            sx2.round() as i32,
            z1,
            z2,
            u1,
            u2,
            sx1,
            sx2,
        )
    }

    fn draw_portal_span(
        ctx: &mut RenderContext,
        x_start: i32,
        x_end: i32,
        ds: f32,
        de: f32,
        us: f32,
        ue: f32,
        line: &LineDefinition,
        front: &Sector,
        back: &Sector,
        oy: f32,
        sx_start_f: f32,
        sx_end_f: f32,
        sidedef: Option<&crate::simulation::engine::Sidedef>,
    ) {
        let center = ctx.height as f32 / 2.0;

        let draw_start = x_start.max(0);
        let draw_end = x_end.min(ctx.width as i32);
        let inv_z1 = 1.0 / ds.max(0.0001);
        let inv_z2 = 1.0 / de.max(0.0001);
        let uoz1 = us * inv_z1;
        let uoz2 = ue * inv_z2;
        let sx_s = sx_start_f;
        let sx_e = sx_end_f;
        let dx = sx_e - sx_s;

        for x in draw_start..draw_end {
            if ctx.upper_clip[x as usize] >= ctx.lower_clip[x as usize] {
                continue;
            }
            let t = (x as f32 - sx_s) / dx;
            let inv_z = inv_z1 + t * (inv_z2 - inv_z1);
            let uoz = uoz1 + t * (uoz2 - uoz1);
            let d = 1.0 / inv_z;
            let u = uoz * d;

            let d = d.max(0.001);
            // DO NOT write to 1D depth_buffer here! Portals have vertical gaps and should not horizontally occlude an entire sprite column!
            let scale = (ctx.height as f32 / d).min(2000.0);
            let angle_offset = ((x as f32 - ctx.width as f32 * 0.5)
                / (ctx.width as f32 / (2.0 * (ctx.fov * 0.5).tan())))
            .atan();
            let ray_dir = Vec2::new(
                (ctx.world.player.angle - angle_offset).cos(),
                (ctx.world.player.angle - angle_offset).sin(),
            );
            let (pfc, pff) = (
                (center - (front.ceiling_height - ctx.eye_z) * scale) as i32,
                (center - (front.floor_height - ctx.eye_z) * scale) as i32,
            );
            Self::draw_flats_column(
                ctx,
                x as u32,
                ctx.upper_clip[x as usize],
                pfc,
                front,
                ray_dir,
                angle_offset,
                true,
            );
            ctx.upper_clip[x as usize] = ctx.upper_clip[x as usize].max(pfc.max(0));
            Self::draw_flats_column(
                ctx,
                x as u32,
                pff,
                ctx.lower_clip[x as usize],
                front,
                ray_dir,
                angle_offset,
                false,
            );
            ctx.lower_clip[x as usize] = ctx.lower_clip[x as usize].min(pff.min(ctx.height as i32));
            if front.ceiling_height > back.ceiling_height {
                if let Some(fs) = sidedef {
                    if let Some(tex) = &fs.texture_upper {
                        Self::draw_wall_column(
                            ctx,
                            x as u32,
                            d,
                            tex,
                            back.ceiling_height,
                            front.ceiling_height,
                            front.light_level,
                            u,
                            oy,
                        );
                    }
                }
                ctx.upper_clip[x as usize] = ctx.upper_clip[x as usize]
                    .max((center - (back.ceiling_height - ctx.eye_z) * scale) as i32);
            }
            if front.floor_height < back.floor_height {
                if let Some(fs) = sidedef {
                    if let Some(tex) = &fs.texture_lower {
                        Self::draw_wall_column(
                            ctx,
                            x as u32,
                            d,
                            tex,
                            front.floor_height,
                            back.floor_height,
                            front.light_level,
                            u,
                            oy,
                        );
                    }
                }
                ctx.lower_clip[x as usize] = ctx.lower_clip[x as usize]
                    .min((center - (back.floor_height - ctx.eye_z) * scale) as i32);
            }
            if ctx.upper_clip[x as usize] >= ctx.lower_clip[x as usize] && ctx.rem > 0 {
                ctx.rem -= 1;
            }
        }
    }

    fn draw_solid_wall_span(
        ctx: &mut RenderContext,
        x_start: i32,
        x_end: i32,
        ds: f32,
        de: f32,
        us: f32,
        ue: f32,
        line: &LineDefinition,
        front: &Sector,
        oy: f32,
        sx_start_f: f32,
        sx_end_f: f32,
        sidedef: Option<&crate::simulation::engine::Sidedef>,
    ) {
        let center = ctx.height as f32 / 2.0;

        let draw_start = x_start.max(0);
        let draw_end = x_end.min(ctx.width as i32);
        let inv_z1 = 1.0 / ds.max(0.0001);
        let inv_z2 = 1.0 / de.max(0.0001);
        let uoz1 = us * inv_z1;
        let uoz2 = ue * inv_z2;
        let sx_s = sx_start_f;
        let sx_e = sx_end_f;
        let dx = sx_e - sx_s;

        for x in draw_start..draw_end {
            if ctx.upper_clip[x as usize] >= ctx.lower_clip[x as usize] {
                continue;
            }
            let t = (x as f32 - sx_s) / dx;
            let inv_z = inv_z1 + t * (inv_z2 - inv_z1);
            let uoz = uoz1 + t * (uoz2 - uoz1);
            let d = 1.0 / inv_z;
            let u = uoz * d;

            let d = d.max(0.001);
            ctx.depth_buffer[x as usize] = d;
            let scale = (ctx.height as f32 / d).min(2000.0);
            let angle_offset = ((x as f32 - ctx.width as f32 * 0.5)
                / (ctx.width as f32 / (2.0 * (ctx.fov * 0.5).tan())))
            .atan();
            let ray_dir = Vec2::new(
                (ctx.world.player.angle - angle_offset).cos(),
                (ctx.world.player.angle - angle_offset).sin(),
            );
            let (pc, pf) = (
                (center - (front.ceiling_height - ctx.eye_z) * scale) as i32,
                (center - (front.floor_height - ctx.eye_z) * scale) as i32,
            );

            Self::draw_flats_column(
                ctx,
                x as u32,
                ctx.upper_clip[x as usize],
                pc,
                front,
                ray_dir,
                angle_offset,
                true,
            );
            Self::draw_flats_column(
                ctx,
                x as u32,
                pf,
                ctx.lower_clip[x as usize],
                front,
                ray_dir,
                angle_offset,
                false,
            );
            if let Some(fs) = sidedef {
                if let Some(tex) = &fs.texture_middle {
                    Self::draw_wall_column(
                        ctx,
                        x as u32,
                        d,
                        tex,
                        front.floor_height,
                        front.ceiling_height,
                        front.light_level,
                        u,
                        oy,
                    );
                }
            }
            if ctx.upper_clip[x as usize] < ctx.lower_clip[x as usize] && ctx.rem > 0 {
                ctx.upper_clip[x as usize] = ctx.lower_clip[x as usize];
                ctx.rem -= 1;
            }
        }
    }

    fn draw_sprites(
        ctx: &mut RenderContext,
        entities: &[&dyn crate::presentation::AetherisEntity],
    ) {
        let (p_pos, p_angle, center_y) = (
            ctx.world.player.position,
            ctx.world.player.angle,
            ctx.height as f32 / 2.0,
        );
        let mut sorted: Vec<_> = entities.iter().collect();
        sorted.sort_by(|a, b| {
            (b.position() - p_pos)
                .length_squared()
                .partial_cmp(&(a.position() - p_pos).length_squared())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for thing in sorted {
            if !thing.should_draw() {
                continue;
            }
            let (dx, dy) = (thing.position().x - p_pos.x, thing.position().y - p_pos.y);
            let dist = dx * p_angle.cos() + dy * p_angle.sin();
            if dist < 8.0 {
                continue;
            }
            let names = thing.get_sprites(p_pos, ctx.world.frame_count);
            let tex = if let Some(t) = names.iter().find_map(|n| ctx.world.textures.get(n)) {
                t
            } else {
                continue;
            };
            let (tx, sc) = (
                dx * p_angle.sin() - dy * p_angle.cos(),
                (ctx.height as f32 / dist).min(2000.0),
            );
            let scale_x = (ctx.width as f32 / 2.0) / (ctx.fov / 2.0).tan();
            let screen_x = (ctx.width as f32 / 2.0) + (tx / dist) * scale_x;

            // Sprite offset math using DOOM patch properties
            let (th, tw) = (tex.height as f32, tex.width as f32);
            let (left_off, top_off) = (tex.left_offset as f32, tex.top_offset as f32);

            let sw = tw * sc;
            let dxs = (screen_x - left_off * sc) as i32;
            let dxe = dxs + sw as i32;

            let sty = (center_y - (thing.z() + top_off - ctx.eye_z) * sc) as i32;
            let sby = sty + (th * sc) as i32;
            let palette = &ctx.world.palettes[ctx.world.current_palette_idx];

            let s_light = ctx
                .world
                .find_sector_at(thing.position())
                .map(|s| ctx.world.sectors[s].light_level)
                .unwrap_or(1.0);
            let cmap = &ctx.world.colormap[((((1.0 - (s_light * ctx.gamma).min(1.0)) * 31.0)
                as usize)
                .min(31)
                + (dist as usize / 64))
                .min(31)
                * 256..];
            for sx in dxs..dxe {
                if sx < 0 || sx >= ctx.width as i32 || dist >= ctx.depth_buffer[sx as usize] {
                    continue;
                }
                let (u, ys, ye) = (
                    (sx - dxs) as f32 / (dxe - dxs) as f32,
                    sty.max(0),
                    sby.min(ctx.height as i32),
                );
                let (tx, shf) = (
                    ((u * tex.width as f32) as u32 % tex.width),
                    (sby - sty) as f32,
                );
                for sy in ys..ye {
                    if sy < 0 || sy >= ctx.height as i32 {
                        continue;
                    }
                    let idx2d = (sy + ctx.y_off as i32) as usize * ctx.stride as usize
                        + (sx + ctx.x_off as i32) as usize;
                    if dist >= ctx.depth_buffer_2d[idx2d] {
                        continue;
                    }

                    let ty = (((sy - sty) as f32 / shf).clamp(0.0, 1.0) * tex.height as f32) as u32
                        % tex.height;
                    let p_idx = tex.pixels_indexed[(ty * tex.width + tx) as usize];
                    if p_idx != -1 {
                        let off = idx2d * 4;
                        if thing.is_spectral() {
                            let (fx, fy) = (
                                (sx + (ctx.world.frame_count % 7) as i32 - 3)
                                    .max(0)
                                    .min((ctx.width - 1) as i32),
                                (sy + (ctx.world.frame_count % 3) as i32 - 1)
                                    .max(0)
                                    .min((ctx.height - 1) as i32),
                            );
                            let foff = ((fy + ctx.y_off as i32) as usize * ctx.stride as usize
                                + (fx + ctx.x_off as i32) as usize)
                                * 4;
                            if foff + 3 < ctx.frame.len() {
                                for i in 0..3 {
                                    ctx.frame[off + i] = (ctx.frame[foff + i] as f32 * 0.4) as u8;
                                }
                                ctx.frame[off + 3] = 255;
                            }
                        } else {
                            let poff = cmap[p_idx as usize] as usize * 3;
                            ctx.frame[off..off + 3].copy_from_slice(&palette[poff..poff + 3]);
                            ctx.frame[off + 3] = 255;
                        }
                    }
                }
            }
        }
    }

    fn draw_weapon(frame: &mut [u8], world: &WorldState, w: usize, h: usize) {
        let (bp, sc) = (world.player.bob_phase, w as f32 / 320.0);
        let (bx, by) = (
            (bp.sin() * 16.0 * sc) as i32,
            (bp.sin().abs() * 8.0 * sc) as i32,
        );
        let wn = match world.player.current_weapon {
            crate::simulation::WeaponType::Pistol => {
                if matches!(world.player.weapon_state, crate::simulation::WeaponState::Firing(f) if f > 2)
                {
                    "PISFA0"
                } else {
                    "PISGA0"
                }
            }
            crate::simulation::WeaponType::Shotgun => match world.player.weapon_state {
                crate::simulation::WeaponState::Firing(f) if f > 6 => "SHTFA0",
                crate::simulation::WeaponState::Firing(f) if f > 4 => "SHTFB0",
                crate::simulation::WeaponState::Firing(f) if f > 2 => "SHTFC0",
                _ => "SHTGA0",
            },
            crate::simulation::WeaponType::Chaingun => match world.player.weapon_state {
                crate::simulation::WeaponState::Firing(f) if f % 2 == 0 => "CHGFA0",
                crate::simulation::WeaponState::Firing(_) => "CHGFB0",
                _ => "CHGGA0",
            },
            _ => "PISGA0",
        };
        if let Some(tex) = world.textures.get(wn) {
            let (tw, th) = (
                (tex.width as f32 * sc) as usize,
                (tex.height as f32 * sc) as usize,
            );
            let sbh = world
                .textures
                .get("STBAR")
                .map(|st| (st.height as f32 * sc) as usize)
                .unwrap_or(0);
            let (xs, ys) = (
                (w as i32 / 2) - (tw as i32 / 2) + bx,
                (h as i32 - sbh as i32) - (th as i32) + by + (24.0 * sc) as i32,
            );
            let palette = &world.palettes[world.current_palette_idx];
            for ty in 0..th {
                for tx in 0..tw {
                    let (fx, fy) = (xs + tx as i32, ys + ty as i32);
                    if fx < 0 || fx >= w as i32 || fy < 0 || fy >= h as i32 {
                        continue;
                    }
                    let (tex_x, tex_y) = (
                        (tx * tex.width as usize) / tw,
                        (ty * tex.height as usize) / th,
                    );
                    let p_idx = tex.pixels_indexed[tex_y * tex.width as usize + tex_x];
                    if p_idx != -1 {
                        let poff = world.colormap[p_idx as usize] as usize * 3;
                        let off = (fy as usize * w + fx as usize) * 4;
                        frame[off..off + 3].copy_from_slice(&palette[poff..poff + 3]);
                        frame[off + 3] = 255;
                    }
                }
            }
        }
    }

    fn draw_screen_flash(frame: &mut [u8], color: [u8; 3], alpha: f32) {
        if alpha <= 0.01 {
            return;
        }
        for pixel in frame.chunks_exact_mut(4) {
            for i in 0..3 {
                pixel[i] = (pixel[i] as f32 * (1.0 - alpha) + color[i] as f32 * alpha) as u8;
            }
        }
    }

    fn draw_hud_num(
        frame: &mut [u8],
        world: &WorldState,
        val: i32,
        right_x: usize,
        y_pos: usize,
        screen_w: usize,
        screen_h: usize,
        prefix: &str,
        sc: f32,
    ) {
        let s = val.to_string();
        let mut total_width_tex = 0;
        let mut char_widths_tex = Vec::with_capacity(s.len());
        for c in s.chars() {
            let name = if c == '%' {
                "STTPRCNT".into()
            } else {
                format!("{}{}", prefix, c)
            };
            if let Some(tex) = world.textures.get(&name) {
                total_width_tex += tex.width as usize;
                char_widths_tex.push(tex.width as usize);
            } else {
                char_widths_tex.push(0);
            }
        }
        let mut cx = right_x.saturating_sub((total_width_tex as f32 * sc) as usize);
        for (i, c) in s.chars().enumerate() {
            let name = if c == '%' {
                "STTPRCNT".into()
            } else {
                format!("{}{}", prefix, c)
            };
            if let Some(tex) = world.textures.get(&name) {
                let tw = (tex.width as f32 * sc) as usize;
                let th = (tex.height as f32 * sc) as usize;
                for y in 0..th {
                    for x in 0..tw {
                        let (px, py) = (cx + x, y_pos + y);
                        if px >= screen_w || py >= screen_h {
                            continue;
                        }
                        let tex_x = (x as f32 / sc) as usize;
                        let tex_y = (y as f32 / sc) as usize;
                        if tex_y >= tex.height as usize || tex_x >= tex.width as usize {
                            continue;
                        }
                        let p_idx = tex.pixels_indexed[tex_y * tex.width as usize + tex_x];
                        if p_idx != -1 {
                            let off = (py * screen_w + px) * 4;
                            let poff = world.colormap[p_idx as usize] as usize * 3;
                            frame[off..off + 3].copy_from_slice(
                                &world.palettes[world.current_palette_idx][poff..poff + 3],
                            );
                            frame[off + 3] = 255;
                        }
                    }
                }
                cx += (char_widths_tex[i] as f32 * sc) as usize;
            }
        }
    }

    fn draw_hud_text(
        frame: &mut [u8],
        world: &WorldState,
        text: &str,
        x_pos: usize,
        y_pos: usize,
        screen_w: usize,
        screen_h: usize,
        color: [u8; 4],
        sc: f32,
    ) {
        let mut cx = x_pos;
        for c in text.to_ascii_uppercase().chars() {
            if let Some(tex) = world.textures.get(&format!("STCFN{:03}", c as u32)) {
                let tw = (tex.width as f32 * sc) as usize;
                let th = (tex.height as f32 * sc) as usize;
                for y in 0..th {
                    for x in 0..tw {
                        let (px, py) = (cx + x, y_pos + y);
                        if px >= screen_w || py >= screen_h {
                            continue;
                        }
                        let tex_x = (x as f32 / sc) as usize;
                        let tex_y = (y as f32 / sc) as usize;
                        if tex_y >= tex.height as usize || tex_x >= tex.width as usize {
                            continue;
                        }
                        if tex.pixels_indexed[tex_y * tex.width as usize + tex_x] != -1 {
                            let tex_off = (tex_y * tex.width as usize + tex_x) * 4;
                            let off = (py * screen_w + px) * 4;
                            let src_r = tex.pixels[tex_off] as f32;
                            let src_g = tex.pixels[tex_off + 1] as f32;
                            let src_b = tex.pixels[tex_off + 2] as f32;

                            if src_r < 40.0 && src_g < 40.0 && src_b < 40.0 {
                                frame[off..off + 3].copy_from_slice(&[
                                    src_r as u8,
                                    src_g as u8,
                                    src_b as u8,
                                ]);
                            } else {
                                let lum = src_r / 200.0;
                                frame[off] = (color[0] as f32 * lum).min(255.0) as u8;
                                frame[off + 1] = (color[1] as f32 * lum).min(255.0) as u8;
                                frame[off + 2] = (color[2] as f32 * lum).min(255.0) as u8;
                            }
                            frame[off + 3] = 255;
                        }
                    }
                }
                cx += tw;
            } else {
                cx += (8.0 * sc) as usize;
            }
        }
    }

    fn draw_face(frame: &mut [u8], world: &WorldState, w: usize, h: usize, hy: usize, sc: f32) {
        let hp = world.player.health as i32;
        let tier = if hp >= 80 {
            "STFST0"
        } else if hp >= 60 {
            "STFST1"
        } else if hp >= 40 {
            "STFST2"
        } else if hp >= 20 {
            "STFST3"
        } else {
            "STFST4"
        };
        let face_name = if world.player.health <= 0.0 {
            "STFDEAD0".into()
        } else if world.player.invuln_timer > 0 {
            "STFGOD0".into()
        } else if world.player.damage_flash > 0.3 {
            format!(
                "STFOUCH{}",
                if world.frame_count % 8 < 4 { "0" } else { "1" }
            )
        } else if matches!(
            world.player.weapon_state,
            crate::simulation::WeaponState::Firing(_)
        ) {
            format!(
                "STFKILL{}",
                if world.frame_count % 4 < 2 { "0" } else { "1" }
            )
        } else if world.player.bonus_flash > 0.0 {
            format!(
                "STFEVL{}",
                if world.frame_count % 8 < 4 { "0" } else { "1" }
            )
        } else {
            format!(
                "{}{}",
                tier,
                if world.frame_count % 100 > 95 {
                    "1"
                } else {
                    "0"
                }
            )
        };
        if let Some(face) = world.textures.get(&face_name) {
            let (fw, fh) = (
                (face.width as f32 * sc) as usize,
                (face.height as f32 * sc) as usize,
            );
            let (fxs, fys) = (w / 2 - fw / 2, hy + (2.0 * sc) as usize);
            for ty in 0..fh {
                for tx in 0..fw {
                    let (px, py) = (fxs + tx, fys + ty);
                    if px >= w || py >= h {
                        continue;
                    }
                    let (tex_x, tex_y) = (
                        (tx * face.width as usize) / fw,
                        (ty * face.height as usize) / fh,
                    );
                    let to = (tex_y * face.width as usize + tex_x) * 4;
                    let fo = (py * w + px) * 4;
                    if fo + 4 <= frame.len()
                        && to + 3 < face.pixels.len()
                        && face.pixels[to + 3] > 0
                    {
                        frame[fo..fo + 4].copy_from_slice(&face.pixels[to..to + 4]);
                    }
                }
            }
        }
    }

    fn draw_line(
        frame: &mut [u8],
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        w: usize,
        h: usize,
        color: [u8; 4],
    ) {
        let (dx, dy) = ((x2 - x1).abs(), -(y2 - y1).abs());
        let (sx, sy) = (if x1 < x2 { 1 } else { -1 }, if y1 < y2 { 1 } else { -1 });
        let (mut err, mut x, mut y) = (dx + dy, x1, y1);
        loop {
            if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                let off = (y as usize * w + x as usize) * 4;
                frame[off..off + 4].copy_from_slice(&color);
            }
            if x == x2 && y == y2 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
}

impl VisualBridge for ClassicSoftwareEngine {
    fn render_scene(
        &mut self,
        world: &WorldState,
        entities: &[&dyn crate::presentation::AetherisEntity],
        player: &dyn crate::presentation::AetherisPlayer,
        _profiler: &mut PerformanceProfiler,
    ) -> anyhow::Result<()> {
        let (w, h) = (self.width, self.height);

        let frame = self.pixels.frame_mut();
        if world.is_intermission {
            frame.fill(0);
            return Ok(());
        }
        frame.fill(0);
        let eye_z = player.z() + 28.0; // Assume player view height
        let mut ctx = RenderContext {
            width: w,
            height: h,
            stride: w,
            x_off: 0,
            y_off: 0,
            frame,
            upper_clip: vec![0; w as usize],
            lower_clip: vec![h as i32; w as usize],
            depth_buffer: vec![4000.0; w as usize],
            depth_buffer_2d: vec![4000.0; (w * h) as usize],
            rem: w,
            eye_z,
            fov: player.fov(),
            world,
            gamma: 1.2,
        };
        if !world.nodes.is_empty() {
            Self::render_bsp_node(&mut ctx, (world.nodes.len() - 1) as u16);
        }
        Self::draw_sprites(&mut ctx, entities);
        Ok(())
    }

    fn render_hud(&mut self, world: &WorldState) -> anyhow::Result<()> {
        let frame = self.pixels.frame_mut();
        let (w, h) = (self.width as usize, self.height as usize);
        let sc = w as f32 / 320.0;
        if world.is_intermission {
            if let Some(map_tex) = world.textures.get("WIMAP0") {
                for y in 0..h {
                    for x in 0..w {
                        let (tx, ty) = (
                            (x * map_tex.width as usize) / w,
                            (y * map_tex.height as usize) / h,
                        );
                        let (to, fo) = ((ty * map_tex.width as usize + tx) * 4, (y * w + x) * 4);
                        if fo + 4 <= frame.len() && to + 4 <= map_tex.pixels.len() {
                            frame[fo..fo + 4].copy_from_slice(&map_tex.pixels[to..to + 4]);
                        }
                    }
                }
            } else {
                frame.fill(0);
            }
            let labels = [
                ("KILLS", world.monsters_killed, world.total_monsters),
                ("ITEMS", world.items_collected, world.total_items),
                ("SECRET", world.secrets_found, world.total_secrets),
            ];
            for (i, (label, val, total)) in labels.iter().enumerate() {
                let ly = h / 3 + i * 30;
                if world.intermission_tic > i as u32 * 35 {
                    Self::draw_hud_text(frame, world, label, w / 4, ly, w, h, [255, 0, 0, 255], sc);
                    let t_val = if world.intermission_tic > i as u32 * 35 + 10 {
                        ((world.intermission_tic - (i as u32 * 35 + 10)) as f32 / 35.0).min(1.0)
                            * (*val as f32)
                    } else {
                        0.0
                    };
                    let pct = if *total > 0 {
                        (t_val / *total as f32 * 100.0) as i32
                    } else {
                        0
                    };
                    Self::draw_hud_num(frame, world, pct, w * 3 / 4, ly, w, h, "STTNUM", sc);
                }
            }
            Self::draw_hud_text(
                frame,
                world,
                "LEVEL COMPLETE",
                w / 2 - (14.0 * 8.0 * sc) as usize / 2,
                h / 6,
                w,
                h,
                [255, 255, 0, 255],
                sc,
            );

            if (world.intermission_tic / 20) % 2 == 0 {
                Self::draw_hud_text(
                    frame,
                    world,
                    "PRESS FIRE TO CONTINUE",
                    w / 2 - (22.0 * 8.0 * sc) as usize / 2,
                    h - 40,
                    w,
                    h,
                    [200, 200, 200, 255],
                    sc,
                );
            }
            return Ok(());
        }
        Self::draw_weapon(frame, world, w, h);
        let mut msg_y = 10;
        for msg in &world.hud_messages {
            Self::draw_hud_text(
                frame,
                world,
                &msg.text,
                w / 2 - ((msg.text.len() as f32 / 2.0) * 8.0 * sc) as usize,
                msg_y,
                w,
                h,
                [msg.color[0], msg.color[1], msg.color[2], 255],
                sc,
            );
            msg_y += (14.0 * sc) as usize;
        }
        if world.player.damage_flash > 0.01 {
            Self::draw_screen_flash(
                frame,
                [255, 0, 0],
                (world.player.damage_flash * 0.5).min(0.5),
            );
        }
        if world.player.bonus_flash > 0.01 {
            Self::draw_screen_flash(
                frame,
                [255, 255, 0],
                (world.player.bonus_flash * 0.4).min(0.4),
            );
        }
        if world.player.invuln_timer > 0 && (world.frame_count / 8) % 2 == 0 {
            Self::draw_screen_flash(frame, [255, 255, 255], 0.3);
        }
        if world.player.radsuit_timer > 0 {
            Self::draw_screen_flash(frame, [0, 255, 0], 0.15);
        }
        match world.menu_state {
            crate::simulation::MenuState::Main
            | crate::simulation::MenuState::EpisodeSelect
            | crate::simulation::MenuState::DifficultySelect
            | crate::simulation::MenuState::Options
            | crate::simulation::MenuState::LoadGame
            | crate::simulation::MenuState::SaveGame => {
                for pixel in frame.chunks_exact_mut(4) {
                    for i in 0..3 {
                        pixel[i] = (pixel[i] as u16 / 3) as u8;
                    }
                }
                let (title, options) = match world.menu_state {
                    crate::simulation::MenuState::Main => (
                        "MAIN MENU",
                        vec!["New Game", "Load Game", "Options", "Quit Game"],
                    ),
                    crate::simulation::MenuState::EpisodeSelect => {
                        ("SELECT EPISODE", vec!["Knee-Deep in the Dead"])
                    }
                    crate::simulation::MenuState::DifficultySelect => (
                        "CHOOSE SKILL",
                        vec![
                            "I'm Too Young To Die.",
                            "Hey, Not Too Rough.",
                            "Hurt Me Plenty.",
                            "Ultra-Violence.",
                            "Nightmare!",
                        ],
                    ),
                    crate::simulation::MenuState::Options => ("OPTIONS", vec!["Sound", "Video"]),
                    _ => (
                        "SAVE/LOAD",
                        vec!["Slot 1", "Slot 2", "Slot 3", "Slot 4", "Slot 5", "Slot 6"],
                    ),
                };
                Self::draw_hud_text(
                    frame,
                    world,
                    title,
                    w / 2 - ((title.len() as f32 / 2.0) * 8.0 * sc) as usize,
                    h / 6,
                    w,
                    h,
                    [255, 0, 0, 255],
                    sc,
                );
                for (i, opt) in options.iter().enumerate() {
                    let color = if world.menu_selection == i {
                        [255, 255, 0, 255]
                    } else {
                        [180, 180, 180, 255]
                    };
                    if world.menu_selection == i {
                        Self::draw_hud_text(
                            frame,
                            world,
                            ">",
                            w / 2
                                - ((opt.len() as f32 / 2.0) * 8.0 * sc) as usize
                                - (16.0 * sc) as usize,
                            h / 2 + i * 30 - 60,
                            w,
                            h,
                            [255, 255, 0, 255],
                            sc,
                        );
                    }
                    Self::draw_hud_text(
                        frame,
                        world,
                        opt,
                        w / 2 - ((opt.len() as f32 / 2.0) * 8.0 * sc) as usize,
                        h / 2 + i * 30 - 60,
                        w,
                        h,
                        color,
                        sc,
                    );
                }
            }
            _ => {}
        }
        if world.is_paused {
            Self::draw_hud_text(
                frame,
                world,
                "PAUSED",
                w / 2 - (3.0 * 8.0 * sc) as usize,
                h / 2 - 10,
                w,
                h,
                [255, 255, 255, 255],
                sc,
            );
        }
        if world.player.health <= 0.0 {
            for pixel in frame.chunks_exact_mut(4) {
                pixel[0] = (pixel[0] as u16 + 120).min(255) as u8;
            }
        }
        if let Some(tex) = world.textures.get("STBAR") {
            let hud_h = (tex.height as f32 * sc) as usize;
            let hy = h.saturating_sub(hud_h);
            for y in 0..hud_h {
                for x in 0..w {
                    let (tx, ty) = (
                        (x * tex.width as usize) / w,
                        (y * tex.height as usize) / hud_h,
                    );
                    let (to, fo) = ((ty * tex.width as usize + tx) * 4, ((hy + y) * w + x) * 4);
                    if fo + 4 <= frame.len() && to + 4 <= tex.pixels.len() {
                        frame[fo..fo + 4].copy_from_slice(&tex.pixels[to..to + 4]);
                    }
                }
            }
            let _hn = hy + (16.0 * sc) as usize;
            // DOOM original placements (approx 3 pixels from the top of the HUD)
            let hn = hy + (3.0 * sc) as usize;
            Self::draw_hud_num(
                frame,
                world,
                world.player.health as i32,
                (90.0 * sc) as usize,
                hn,
                w,
                h,
                "STTNUM",
                sc,
            );
            // DOOM draws the percent symbol right after health/armor. STTPRCNT is its name.
            Self::draw_hud_text(
                frame,
                world,
                "%",
                (90.0 * sc) as usize,
                hn,
                w,
                h,
                [255, 255, 255, 255],
                sc,
            ); // Draw HUD Text is not used for big STTNUMs! Wait, we need to pass a string or special path to draw_hud_num.
            // Let's modify draw_hud_num to accept essentially format strings or strings.
            // Actually, DOOM literally just places % at X=90. Because draw_hud_num right-aligns from X=90, the numerals go left.
            // The % goes AT X=90.
            if let Some(tex) = world.textures.get("STTPRCNT") {
                let (tw, th) = (
                    (tex.width as f32 * sc) as usize,
                    (tex.height as f32 * sc) as usize,
                );
                for ty in 0..th {
                    for tx in 0..tw {
                        let (px, py) = ((90.0 * sc) as usize + tx, hn + ty);
                        if px < w && py < h {
                            let (tex_x, tex_y) = (
                                (tx * tex.width as usize) / tw,
                                (ty * tex.height as usize) / th,
                            );
                            let to = (tex_y * tex.width as usize + tex_x) * 4;
                            let fo = (py * w + px) * 4;
                            if fo + 4 <= frame.len()
                                && to + 3 < tex.pixels.len()
                                && tex.pixels[to + 3] > 0
                            {
                                frame[fo..fo + 4].copy_from_slice(&tex.pixels[to..to + 4]);
                            }
                        }
                    }
                }
            }

            Self::draw_hud_num(
                frame,
                world,
                world.player.armor as i32,
                (221.0 * sc) as usize,
                hn,
                w,
                h,
                "STTNUM",
                sc,
            );
            if let Some(tex) = world.textures.get("STTPRCNT") {
                let (tw, th) = (
                    (tex.width as f32 * sc) as usize,
                    (tex.height as f32 * sc) as usize,
                );
                for ty in 0..th {
                    for tx in 0..tw {
                        let (px, py) = ((221.0 * sc) as usize + tx, hn + ty);
                        if px < w && py < h {
                            let (tex_x, tex_y) = (
                                (tx * tex.width as usize) / tw,
                                (ty * tex.height as usize) / th,
                            );
                            let to = (tex_y * tex.width as usize + tex_x) * 4;
                            let fo = (py * w + px) * 4;
                            if fo + 4 <= frame.len()
                                && to + 3 < tex.pixels.len()
                                && tex.pixels[to + 3] > 0
                            {
                                frame[fo..fo + 4].copy_from_slice(&tex.pixels[to..to + 4]);
                            }
                        }
                    }
                }
            }

            let ai = crate::simulation::engine::weapon_ammo_type(world.player.current_weapon)
                .unwrap_or(0);
            Self::draw_hud_num(
                frame,
                world,
                world.player.ammo[ai] as i32,
                (44.0 * sc) as usize,
                hn,
                w,
                h,
                "STTNUM",
                sc,
            );

            // Right hand side small ammo counters
            let (sv, sy_start) = (288.0 * sc, hy + (5.0 * sc) as usize);
            let (mv, m_start) = (316.0 * sc, hy + (5.0 * sc) as usize);
            let step = (6.0 * sc) as usize;

            // Current ammo
            Self::draw_hud_num(
                frame,
                world,
                world.player.ammo[0] as i32,
                sv as usize,
                sy_start,
                w,
                h,
                "STYSNUM",
                sc,
            );
            Self::draw_hud_num(
                frame,
                world,
                world.player.ammo[1] as i32,
                sv as usize,
                sy_start + step,
                w,
                h,
                "STYSNUM",
                sc,
            );
            Self::draw_hud_num(
                frame,
                world,
                world.player.ammo[2] as i32,
                sv as usize,
                sy_start + step * 2,
                w,
                h,
                "STYSNUM",
                sc,
            );
            Self::draw_hud_num(
                frame,
                world,
                world.player.ammo[3] as i32,
                sv as usize,
                sy_start + step * 3,
                w,
                h,
                "STYSNUM",
                sc,
            );

            // Max ammo
            let max_ammo = [200, 50, 50, 300];
            Self::draw_hud_num(
                frame,
                world,
                max_ammo[0],
                mv as usize,
                m_start,
                w,
                h,
                "STYSNUM",
                sc,
            );
            Self::draw_hud_num(
                frame,
                world,
                max_ammo[1],
                mv as usize,
                m_start + step,
                w,
                h,
                "STYSNUM",
                sc,
            );
            Self::draw_hud_num(
                frame,
                world,
                max_ammo[2],
                mv as usize,
                m_start + step * 2,
                w,
                h,
                "STYSNUM",
                sc,
            );
            Self::draw_hud_num(
                frame,
                world,
                max_ammo[3],
                mv as usize,
                m_start + step * 3,
                w,
                h,
                "STYSNUM",
                sc,
            );

            Self::draw_face(frame, world, w, h, hy, sc);
        }

        Ok(())
    }

    fn render_automap(&mut self, world: &WorldState) -> anyhow::Result<()> {
        if !world.is_automap {
            return Ok(());
        }
        let mut frame = self.pixels.frame_mut();
        let (w, h) = (self.width as usize, self.height as usize);
        for pixel in frame.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0, 0, 0, 255]);
        }
        let (pp, sc) = (world.player.position, self.map_scale);
        let ts = |pos: Vec2| -> (i32, i32) {
            (
                (w as f32 / 2.0 + (pos.x - pp.x) * sc) as i32,
                (h as f32 / 2.0 - (pos.y - pp.y) * sc) as i32,
            )
        };
        for line in &world.linedefs {
            let ((x1, y1), (x2, y2)) = (
                ts(world.vertices[line.start_idx]),
                ts(world.vertices[line.end_idx]),
            );
            let color = if line.sector_back.is_none() {
                [255, 0, 0, 255]
            } else {
                [100, 100, 100, 255]
            };
            Self::draw_line(
                &mut frame,
                x1,
                y1,
                x2,
                y2,
                w,
                h,
                if line.special_type != 0 {
                    [255, 255, 0, 255]
                } else {
                    color
                },
            );
        }
        let ps = ts(pp);
        let tip = (
            ps.0 + (world.player.angle.cos() * 15.0) as i32,
            ps.1 - (world.player.angle.sin() * 15.0) as i32,
        );
        Self::draw_line(&mut frame, ps.0, ps.1, tip.0, tip.1, w, h, [0, 255, 0, 255]);
        for thing in &world.things {
            if thing.picked_up {
                continue;
            }
            let color = if matches!(thing.kind, 5 | 40) {
                [0, 0, 255, 255]
            } else if matches!(thing.kind, 6 | 39) {
                [255, 255, 0, 255]
            } else if matches!(thing.kind, 13 | 38) {
                [255, 0, 0, 255]
            } else {
                [0, 200, 0, 255]
            };
            let (sx, sy) = ts(thing.position);
            if sx > 0 && sx < w as i32 && sy > 0 && sy < h as i32 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let off = ((sy as i32 + dy) as usize * w + (sx as i32 + dx) as usize) * 4;
                        if off + 4 <= frame.len() {
                            frame[off..off + 4].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_input(&mut self, actions: &std::collections::HashSet<crate::simulation::GameAction>) {
        if actions.contains(&crate::simulation::GameAction::ZoomIn) {
            self.map_scale *= 1.05;
        }
        if actions.contains(&crate::simulation::GameAction::ZoomOut) {
            self.map_scale *= 0.95;
        }
        self.map_scale = self.map_scale.clamp(0.01, 2.0);
    }

    fn present(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut melt) = self.melt_state {
            let mut frame = self.pixels.frame_mut();
            melt.apply(&mut frame, &self.prev_frame, self.width, self.height);
            if !melt.update(self.height) {
                self.melt_state = None;
            }
        }
        self.pixels.render().map_err(|e| anyhow::anyhow!("{}", e))
    }

    fn on_map_loaded(&mut self, _world: &WorldState) {
        let frame = self.pixels.frame();
        let copy_len = frame.len().min(self.prev_frame.len());
        self.prev_frame[..copy_len].copy_from_slice(&frame[..copy_len]);
        // Only trigger melt if the previous frame actually had content (not startup)
        if self.prev_frame.iter().any(|&x| x > 0) {
            self.melt_state = Some(MeltState::new(self.width, self.height));
        }
    }

    fn handle_resize(&mut self, width: u32, height: u32, _resize_buffer: bool) {
        let _ = self.pixels.resize_surface(width, height);
    }
    fn take_screenshot(&mut self, path: &str) -> anyhow::Result<()> {
        let frame = self.pixels.frame();
        image::save_buffer(
            path,
            frame,
            self.width,
            self.height,
            image::ColorType::Rgba8,
        )?;
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
