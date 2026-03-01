use binrw::{BinRead, binread, io::Cursor};
use std::collections::HashMap;
use std::io::Read;

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadHeader {
    pub wad_type: [u8; 4], // IWAD or PWAD
    pub num_lumps: u32,
    pub directory_offset: u32,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadDirectoryEntry {
    pub file_pos: u32,
    pub size: u32,
    #[br(map = |x: [u8; 8]| String::from_utf8_lossy(&x).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string().to_uppercase())]
    pub name: String,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadVertex {
    pub x: i16,
    pub y: i16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadLinedef {
    pub start_vertex: u16,
    pub end_vertex: u16,
    pub flags: u16,
    pub special_type: u16,
    pub sector_tag: u16,
    pub sidenum_front: u16,
    pub sidenum_back: u16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadSidedef {
    pub texture_offset_x: i16,
    pub texture_offset_y: i16,
    pub texture_upper: [u8; 8],
    pub texture_lower: [u8; 8],
    pub texture_middle: [u8; 8],
    pub sector_id: u16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadSector {
    pub floor_height: i16,
    pub ceiling_height: i16,
    pub texture_floor: [u8; 8],
    pub texture_ceiling: [u8; 8],
    pub light_level: i16,
    pub special_type: i16,
    pub tag: i16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadPalette {
    pub colors: [[u8; 3]; 256],
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadThing {
    pub x: i16,
    pub y: i16,
    pub angle: u16,
    pub kind: u16,
    pub flags: u16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadNode {
    pub x: i16,
    pub y: i16,
    pub dx: i16,
    pub dy: i16,
    pub bbox_right: [i16; 4],
    pub bbox_left: [i16; 4],
    pub child_right: u16,
    pub child_left: u16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadSubsector {
    pub seg_count: u16,
    pub first_seg_index: u16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadSeg {
    pub start_vertex: u16,
    pub end_vertex: u16,
    pub angle: i16,
    pub linedef_index: u16,
    pub direction: i16, // 0 = same as linedef, 1 = opposite
    pub offset: i16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadPatchHeader {
    pub width: u16,
    pub height: u16,
    pub left_offset: i16,
    pub top_offset: i16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadMapTexture {
    #[br(map = |x: [u8; 8]| String::from_utf8_lossy(&x).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string())]
    pub name: String,
    pub masked: u32,
    pub width: u16,
    pub height: u16,
    pub column_directory: u32, // Obsolete
    pub patch_count: u16,
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct WadMapPatch {
    pub origin_x: i16,
    pub origin_y: i16,
    pub patch_index: u16,
    pub step_dir: u16,
    pub colormap: u16,
}

pub struct WadLoader {
    pub header: WadHeader,
    pub directory: Vec<WadDirectoryEntry>,
    pub data: Vec<u8>,
    lump_index: HashMap<String, usize>, // Cache for O(1) lump lookups
}

impl WadLoader {
    pub fn new(data: Vec<u8>) -> anyhow::Result<Self> {
        log::info!(
            "WadLoader: Loading data buffer of size {} bytes",
            data.len()
        );
        let mut cursor = Cursor::new(&data);
        let header = WadHeader::read(&mut cursor)?;
        log::info!(
            "WadLoader: Header parsed: {:?}, num_lumps: {}, dir_offset: {}",
            String::from_utf8_lossy(&header.wad_type),
            header.num_lumps,
            header.directory_offset
        );

        if header.directory_offset as usize >= data.len() {
            anyhow::bail!(
                "WadLoader: Directory offset {} is outside buffer size {}",
                header.directory_offset,
                data.len()
            );
        }

        cursor.set_position(header.directory_offset as u64);
        let mut directory = Vec::new();
        for i in 0..header.num_lumps {
            match WadDirectoryEntry::read(&mut cursor) {
                Ok(entry) => directory.push(entry),
                Err(e) => {
                    anyhow::bail!(
                        "WadLoader: Failed to parse directory entry {} at offset {}: {:?}",
                        i,
                        cursor.position(),
                        e
                    );
                }
            }
        }

        // Build lump index for O(1) lookups
        let mut lump_index = HashMap::new();
        for (idx, entry) in directory.iter().enumerate() {
            lump_index.insert(entry.name.clone(), idx);
        }
        log::info!("WadLoader: Built index for {} lumps", lump_index.len());

        Ok(Self {
            header,
            directory,
            data,
            lump_index,
        })
    }

    pub fn get_lump_data(&self, name: &str) -> Option<&[u8]> {
        // Use hash map index for O(1) lookup instead of O(n) scan
        self.lump_index
            .get(name)
            .and_then(|&idx| self.directory.get(idx))
            .map(|e| &self.data[e.file_pos as usize..(e.file_pos + e.size) as usize])
    }
    pub fn extract_music(&self, name: &str) -> Option<Vec<u8>> {
        log::info!("WadLoader: Extracting music lump '{}'", name);
        let data = self.get_lump_data(name)?;
        log::info!("WadLoader: Lump '{}' size: {} bytes", name, data.len());

        if data.starts_with(b"MUS\x1A") {
            log::info!("WadLoader: Lump '{}' identified as MUS format", name);
            match self.mus_to_midi(data) {
                Ok(midi) => {
                    log::info!(
                        "WadLoader: Successfully converted MUS to MIDI ({} bytes)",
                        midi.len()
                    );
                    Some(midi)
                }
                Err(e) => {
                    log::error!(
                        "WadLoader: Failed to convert MUS to MIDI for '{}': {:?}",
                        name,
                        e
                    );
                    None
                }
            }
        } else if data.starts_with(b"MThd") {
            log::info!(
                "WadLoader: Lump '{}' identified as standard MIDI format",
                name
            );
            Some(data.to_vec())
        } else {
            log::warn!(
                "WadLoader: Lump '{}' has unknown music format (header: {:02X?})",
                name,
                &data[..data.len().min(4)]
            );
            None
        }
    }

    fn mus_to_midi(&self, mus_data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if mus_data.len() < 16 {
            anyhow::bail!("MUS data too short");
        }
        let mut midi = Vec::new();

        // Header
        midi.extend_from_slice(b"MThd");
        midi.extend_from_slice(&6u32.to_be_bytes());
        midi.extend_from_slice(&0u16.to_be_bytes()); // Type 0
        midi.extend_from_slice(&1u16.to_be_bytes()); // 1 Track
        midi.extend_from_slice(&140u16.to_be_bytes()); // 140 ticks per quarter note

        midi.extend_from_slice(b"MTrk");
        let track_len_pos = midi.len();
        midi.extend_from_slice(&0u32.to_be_bytes());

        let mut cursor = Cursor::new(mus_data);
        cursor.set_position(4); // Skip "MUS\x1a"
        let _score_len: u16 = BinRead::read_le(&mut cursor)?;
        let score_start: u16 = BinRead::read_le(&mut cursor)?;
        let _channels: u16 = BinRead::read_le(&mut cursor)?;
        let _sec_channels: u16 = BinRead::read_le(&mut cursor)?;
        let _instr_count: u16 = BinRead::read_le(&mut cursor)?;

        let mut cur_pos = score_start as usize;
        let mut delta_time = 0u32;
        let mut last_status = [0u8; 16];

        loop {
            if cur_pos >= mus_data.len() {
                break;
            }
            let b = mus_data[cur_pos];
            cur_pos += 1;
            let last = (b & 0x80) != 0;
            let event_type = (b & 0x70) >> 4;
            let channel = b & 0x0F;
            let midi_chan = if channel == 15 { 9 } else { channel }; // 15 is percussion in MUS

            let mut out_event = Vec::new();
            match event_type {
                0 => {
                    // Release Note
                    let key = mus_data[cur_pos];
                    cur_pos += 1;
                    out_event.push(0x80 | midi_chan);
                    out_event.push(key & 0x7F);
                    out_event.push(0);
                }
                1 => {
                    // Play Note
                    let key = mus_data[cur_pos];
                    cur_pos += 1;
                    let vol = if (key & 0x80) != 0 {
                        let v = mus_data[cur_pos];
                        cur_pos += 1;
                        v & 0x7F
                    } else {
                        last_status[channel as usize]
                    };
                    last_status[channel as usize] = vol;
                    out_event.push(0x90 | midi_chan);
                    out_event.push(key & 0x7F);
                    out_event.push(vol);
                }
                2 => {
                    // Pitch Bend
                    let bend = mus_data[cur_pos];
                    cur_pos += 1;
                    out_event.push(0xE0 | midi_chan);
                    out_event.push(0);
                    out_event.push(bend >> 1);
                }
                3 => {
                    // System Event
                    let cc = mus_data[cur_pos];
                    cur_pos += 1;
                    let midi_cc = match cc {
                        0 => 120, // All sounds off
                        1 => 121, // Reset controllers
                        2 => 123, // All notes off
                        3 => 126, // Mono on
                        4 => 127, // Poly on
                        _ => 0,
                    };
                    if midi_cc != 0 {
                        out_event.push(0xB0 | midi_chan);
                        out_event.push(midi_cc);
                        out_event.push(0);
                    }
                }
                4 => {
                    // Controller
                    let cc_raw = mus_data[cur_pos];
                    cur_pos += 1;
                    let val = mus_data[cur_pos];
                    cur_pos += 1;
                    if cc_raw == 0 {
                        // MUS Controller 0 is Program Change
                        out_event.push(0xC0 | midi_chan);
                        out_event.push(val & 0x7F);
                    } else {
                        let cc = match cc_raw {
                            1 => 1,
                            2 => 7,
                            3 => 10,
                            4 => 11,
                            5 => 91,
                            6 => 93,
                            7 => 64,
                            8 => 67,
                            9 => 120,
                            _ => 0,
                        };
                        if cc != 0 || cc_raw == 1 {
                            out_event.push(0xB0 | midi_chan);
                            out_event.push(cc);
                            out_event.push(val & 0x7F);
                        }
                    }
                }
                6 => {
                    // End
                    break;
                }
                _ => {}
            }

            // Write VLQ delta
            let mut d = delta_time;
            let mut buffer = [0u8; 4];
            let mut pos = 0;
            buffer[pos] = (d & 0x7f) as u8;
            while d > 0x7f {
                d >>= 7;
                pos += 1;
                buffer[pos] = (0x80 | (d & 0x7f)) as u8;
            }
            for i in (0..=pos).rev() {
                midi.push(buffer[i]);
            }
            midi.extend_from_slice(&out_event);

            delta_time = 0;
            if last {
                loop {
                    let b_del = mus_data[cur_pos];
                    cur_pos += 1;
                    delta_time = (delta_time << 7) | (b_del & 0x7F) as u32;
                    if (b_del & 0x80) == 0 {
                        break;
                    }
                }
            }
        }

        midi.extend_from_slice(&[0, 0xFF, 0x2F, 0]);
        let total_len = (midi.len() - track_len_pos - 4) as u32;
        midi[track_len_pos..track_len_pos + 4].copy_from_slice(&total_len.to_be_bytes());
        Ok(midi)
    }

    pub fn load_textures(&self, world: &mut crate::simulation::WorldState) -> anyhow::Result<()> {
        // Debug: write early to confirm this function is called
        let _ = std::fs::write("/tmp/doom_debug.txt", "load_textures() called\n");
        log::info!("WadLoader: Starting texture load...");
        let palette_data = self
            .get_lump_data("PLAYPAL")
            .ok_or_else(|| anyhow::anyhow!("PLAYPAL not found"))?;
        let palette = WadPalette::read(&mut Cursor::new(palette_data))?;

        let pnames_data = self
            .get_lump_data("PNAMES")
            .ok_or_else(|| anyhow::anyhow!("PNAMES not found"))?;
        let mut pnames_cursor = Cursor::new(pnames_data);
        let num_pnames: u32 = BinRead::read_le(&mut pnames_cursor)?;
        let mut pnames = Vec::new();
        for _ in 0..num_pnames {
            let mut name = [0u8; 8];
            pnames_cursor.read_exact(&mut name)?;
            pnames.push(
                String::from_utf8_lossy(&name)
                    .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                    .to_string()
                    .to_uppercase(),
            );
        }
        log::info!("WadLoader: Loaded {} patch names from PNAMES", pnames.len());

        let mut tex_count = 0;
        for lump_name in ["TEXTURE1", "TEXTURE2"] {
            if let Some(texture_data) = self.get_lump_data(lump_name) {
                log::info!("WadLoader: Found lump {}", lump_name);
                let mut tex_cursor = Cursor::new(texture_data);
                let num_textures: u32 = match BinRead::read_le(&mut tex_cursor) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let mut offsets = Vec::new();
                for _ in 0..num_textures {
                    if let Ok(off) = u32::read_le(&mut tex_cursor) {
                        offsets.push(off);
                    }
                }

                for offset in offsets {
                    if offset as usize >= texture_data.len() {
                        continue;
                    }
                    tex_cursor.set_position(offset as u64);
                    if let Ok(map_tex) = WadMapTexture::read(&mut tex_cursor) {
                        let mut texture_pixels =
                            vec![0u8; map_tex.width as usize * map_tex.height as usize * 4];
                        let mut texture_pixels_indexed =
                            vec![-1i16; map_tex.width as usize * map_tex.height as usize];
                        // Initialize with opaque black (alpha=255) — wall textures should never be transparent
                        for p in texture_pixels.chunks_exact_mut(4) {
                            p.copy_from_slice(&[0, 0, 0, 255]);
                        }

                        for _ in 0..map_tex.patch_count {
                            if let Ok(map_patch) = WadMapPatch::read(&mut tex_cursor) {
                                if (map_patch.patch_index as usize) < pnames.len() {
                                    let patch_name = &pnames[map_patch.patch_index as usize];
                                    if let Some(patch_data) = self.get_lump_data(patch_name) {
                                        let _ = self.composite_patch_indexed(
                                            &mut texture_pixels,
                                            &mut texture_pixels_indexed,
                                            map_tex.width,
                                            map_tex.height,
                                            patch_data,
                                            map_patch.origin_x,
                                            map_patch.origin_y,
                                            &palette,
                                        );
                                    }
                                }
                            }
                        }

                        world.textures.insert(
                            map_tex.name.clone().to_uppercase(),
                            crate::simulation::Texture {
                                name: map_tex.name,
                                width: map_tex.width as u32,
                                height: map_tex.height as u32,
                                left_offset: 0,
                                top_offset: 0,
                                pixels: texture_pixels,
                                pixels_indexed: texture_pixels_indexed,
                            },
                        );
                        tex_count += 1;
                    }
                }
            }
        }
        log::info!("WadLoader: Successfully loaded {} wall textures", tex_count);

        // Add a DEBUG fallback texture
        let mut debug_pixels = vec![0u8; 64 * 64 * 4];
        let mut debug_idx = vec![-1i16; 64 * 64];
        for y in 0..64 {
            for x in 0..64 {
                let cell = (x / 8 + y / 8) % 2 == 0;
                let col = if cell {
                    [255, 0, 255, 255]
                } else {
                    [0, 0, 0, 255]
                };
                let idx = if cell { 176 } else { 0 }; // 176 is bright red in most DOOM palettes
                debug_pixels[(y * 64 + x) * 4..((y * 64 + x) * 4 + 4)].copy_from_slice(&col);
                debug_idx[y * 64 + x] = idx as i16;
            }
        }
        world.textures.insert(
            "DEBUG".to_string(),
            crate::simulation::Texture {
                name: "DEBUG".into(),
                width: 64,
                height: 64,
                left_offset: 0,
                top_offset: 0,
                pixels: debug_pixels,
                pixels_indexed: debug_idx,
            },
        );

        self.load_flats(world, &palette)?;
        self.load_sprites(world, &palette)?;
        self.load_ui_assets(world, &palette)?;
        self.load_lighting(world)?;

        // Debug: Export ALL loaded textures to file
        if let Ok(mut f) = std::fs::File::create("/tmp/doom_all_textures.txt") {
            use std::io::Write;
            let _ = writeln!(f, "Total Textures: {}", world.textures.len());
            let mut names: Vec<_> = world.textures.keys().collect();
            names.sort();
            for name in names {
                let _ = writeln!(f, "  {}", name);
            }
        }

        Ok(())
    }

    fn load_ui_assets(
        &self,
        world: &mut crate::simulation::WorldState,
        palette: &WadPalette,
    ) -> anyhow::Result<()> {
        let ui_lumps = [
            // HUD
            "STBAR", "STARMS", "PISGA0", "PISGB0", "PISFA0", "PISFB0", "SHTGA0", "SHTGB0", "SHTFA0",
            "SHTFB0", "SHTFC0", "SHTFD0", "CHGGA0", "CHGGB0", "CHGFA0", "CHGFB0", "MISGA0",
            "MISGB0", "MISFA0", "MISFB0", // Rocket Launcher
            "PLSGA0", "PLSGB0", "PLSFA0", "PLSFB0", // Plasma Rifle
            "BFGGA0", "BFGGB0", "BFGFA0", "BFGFB0", // BFG9000
            "SAWGA0", "SAWGB0", "SAWFA0", "SAWFB0", // Chainsaw
            "STGNUM0", "STGNUM1", "STGNUM2", "STGNUM3", "STGNUM4", "STGNUM5", "STGNUM6", "STGNUM7",
            "STGNUM8", "STGNUM9", "STTNUM0", "STTNUM1", "STTNUM2", "STTNUM3", "STTNUM4", "STTNUM5",
            "STTNUM6", "STTNUM7", "STTNUM8", "STTNUM9", "STTPRCNT", "STTMINUS", "STYSNUM0",
            "STYSNUM1", "STYSNUM2", "STYSNUM3", "STYSNUM4", "STYSNUM5", "STYSNUM6", "STYSNUM7",
            "STYSNUM8", "STYSNUM9", "AMMNUM0", "AMMNUM1", "AMMNUM2", "AMMNUM3", "AMMNUM4",
            "AMMNUM5", "AMMNUM6", "AMMNUM7", "AMMNUM8", "AMMNUM9", "STKEYS0", "STKEYS1", "STKEYS2",
            "STKEYS3", "STKEYS4", "STKEYS5", "STFST00", "STFST01", "STFST02", "STFDEAD0",
            "STFTOP0", "STFBANY", "STFST10", "STFST11", "STFST12", "STFST20", "STFST21", "STFST22",
            "STFST30", "STFST31", "STFST32", "STFST40", "STFST41", "STFST42", "STFOUCH0",
            "STFEVL0", "STFKILL0", // Menu graphics
            "M_DOOM", "M_NGAME", "M_LOADG", "M_SAVEG", "M_OPTION", "M_QUITG", "M_RDTHIS",
            "M_EPISOD", "M_EPI1", "M_EPI2", "M_EPI3", "M_SKULL1", "M_SKULL2", "M_THERMM",
            "M_THERML", "M_THERMR", "M_THERMO", "M_OPTTTL", "M_MSENS", "M_SVOL", "M_SFXVOL",
            "M_MUSVOL", "M_ENDGAM", "M_MESSG", "M_DETAIL", "M_SCRNSZ", "M_SGTTL", "M_LGTTL",
            "M_SKILL", "M_NMARE", "M_JKILL", "M_ROUGH", "M_HURT", "M_ULTRA", "TITLEPIC", "CREDIT",
            "HELP1", "HELP2", "WIMAP0", "WIMINUS", "WIPCNT", "WICOLON", "WINUM0", "WINUM1",
            "WINUM2", "WINUM3", "WINUM4", "WINUM5", "WINUM6", "WINUM7", "WINUM8", "WINUM9",
            "WIKIL", "WIITM", "WISCR", "WIPAR", "WIPLAT", "WIVICTO", "WINOSTAT", "WILV00",
            "WILV01", "WILV02", "WILV03", "WILV04", "WILV05", "WILV06", "WILV07", "WILV08",
        ];
        let mut ui_count = 0;
        for name in ui_lumps {
            if world.textures.contains_key(name) {
                continue;
            }
            if let Some(data) = self.get_lump_data(name) {
                let mut cursor = Cursor::new(data);
                if let Ok(header) = WadPatchHeader::read(&mut cursor) {
                    let mut pixels = vec![0u8; header.width as usize * header.height as usize * 4];
                    let mut pixels_idx =
                        vec![-1i16; header.width as usize * header.height as usize];

                    let _ = self.composite_patch_indexed(
                        &mut pixels,
                        &mut pixels_idx,
                        header.width,
                        header.height,
                        data,
                        0,
                        0,
                        palette,
                    );

                    world.textures.insert(
                        name.to_uppercase(),
                        crate::simulation::Texture {
                            name: name.to_string(),
                            width: header.width as u32,
                            height: header.height as u32,
                            left_offset: header.left_offset as i32,
                            top_offset: header.top_offset as i32,
                            pixels,
                            pixels_indexed: pixels_idx,
                        },
                    );
                    log::info!(
                        "WadLoader: Loaded UI asset {} ({}x{})",
                        name,
                        header.width,
                        header.height
                    );
                    ui_count += 1;
                }
            } else {
                log::warn!("WadLoader: UI Lump {} not found", name);
            }
        }

        // Load UI Font (STCFN033 to STCFN121)
        for c in 33..=121 {
            let name = format!("STCFN{:03}", c);
            if world.textures.contains_key(&name) {
                continue;
            }
            if let Some(data) = self.get_lump_data(&name) {
                let mut cursor = Cursor::new(data);
                if let Ok(header) = WadPatchHeader::read(&mut cursor) {
                    let mut pixels = vec![0u8; header.width as usize * header.height as usize * 4];
                    let mut pixels_idx =
                        vec![-1i16; header.width as usize * header.height as usize];

                    let _ = self.composite_patch_indexed(
                        &mut pixels,
                        &mut pixels_idx,
                        header.width,
                        header.height,
                        data,
                        0,
                        0,
                        palette,
                    );

                    world.textures.insert(
                        name.to_uppercase(),
                        crate::simulation::Texture {
                            name: name.to_string(),
                            width: header.width as u32,
                            height: header.height as u32,
                            left_offset: header.left_offset as i32,
                            top_offset: header.top_offset as i32,
                            pixels,
                            pixels_indexed: pixels_idx,
                        },
                    );
                    ui_count += 1;
                }
            }
        }

        log::info!("WadLoader: Loaded {} UI assets", ui_count);
        Ok(())
    }

    fn mirror_pixels(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
        let mut mirrored = vec![0u8; pixels.len()];
        for y in 0..height {
            for x in 0..width {
                let src_off = (y * width + x) as usize * 4;
                let dst_off = (y * width + (width - 1 - x)) as usize * 4;
                mirrored[dst_off..dst_off + 4].copy_from_slice(&pixels[src_off..src_off + 4]);
            }
        }
        mirrored
    }

    fn mirror_pixels_indexed(pixels_indexed: &[i16], width: u32, height: u32) -> Vec<i16> {
        let mut mirrored = vec![-1i16; pixels_indexed.len()];
        for y in 0..height {
            for x in 0..width {
                let src_off = (y * width + x) as usize;
                let dst_off = (y * width + (width - 1 - x)) as usize;
                mirrored[dst_off] = pixels_indexed[src_off];
            }
        }
        mirrored
    }

    fn load_sprites(
        &self,
        world: &mut crate::simulation::WorldState,
        palette: &WadPalette,
    ) -> anyhow::Result<()> {
        let start_idx = self
            .directory
            .iter()
            .position(|e| e.name == "S_START" || e.name == "SS_START");
        let end_idx = self
            .directory
            .iter()
            .position(|e| e.name == "S_END" || e.name == "SS_END");

        let (start, end) = match (start_idx, end_idx) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                log::warn!("WadLoader: Sprite markers not found");
                return Ok(());
            }
        };

        let map_lumps = [
            "THINGS", "LINEDEFS", "SIDEDEFS", "VERTEXES", "SEGS", "SSECTORS", "NODES", "SECTORS",
            "REJECT", "BLOCKMAP",
        ];
        for i in (start + 1)..end {
            let entry = &self.directory[i];
            if entry.size < 8 || map_lumps.contains(&entry.name.as_str()) {
                continue;
            }

            let patch_data =
                &self.data[entry.file_pos as usize..(entry.file_pos + entry.size) as usize];
            let mut cursor = Cursor::new(patch_data);
            let header = match WadPatchHeader::read(&mut cursor) {
                Ok(h) => h,
                Err(_) => continue,
            };

            let mut pixels = vec![0u8; header.width as usize * header.height as usize * 4];
            let mut pixels_indexed = vec![-1i16; header.width as usize * header.height as usize];
            for p in pixels.chunks_exact_mut(4) {
                p.copy_from_slice(&[0, 0, 0, 0]);
            }
            if let Err(_) = self.composite_patch_indexed(
                &mut pixels,
                &mut pixels_indexed,
                header.width,
                header.height,
                patch_data,
                0,
                0,
                palette,
            ) {
                continue;
            }

            let tex = crate::simulation::Texture {
                name: entry.name.clone(),
                width: header.width as u32,
                height: header.height as u32,
                left_offset: header.left_offset as i32,
                top_offset: header.top_offset as i32,
                pixels: pixels.clone(),
                pixels_indexed: pixels_indexed.clone(),
            };

            // Support XXXXAx and XXXXAxBy formats
            let name = entry.name.to_uppercase();

            if name.len() == 6 {
                // XXXXAx
                world.textures.insert(name.clone(), tex.clone());
            } else if name.len() == 8 {
                // XXXXAxBy - By is a mirrored version of Ax
                let first = &name[0..6];
                let second = format!("{}{}", &name[0..4], &name[6..8]);

                world.textures.insert(first.to_string(), tex.clone());

                let mirrored_pixels =
                    Self::mirror_pixels(&pixels, header.width as u32, header.height as u32);
                let mirrored_pixels_indexed = Self::mirror_pixels_indexed(
                    &pixels_indexed,
                    header.width as u32,
                    header.height as u32,
                );
                let mirrored_tex = crate::simulation::Texture {
                    name: second.clone(),
                    width: tex.width,
                    height: tex.height,
                    left_offset: tex.width as i32 - tex.left_offset,
                    top_offset: tex.top_offset,
                    pixels: mirrored_pixels,
                    pixels_indexed: mirrored_pixels_indexed,
                };
                world.textures.insert(second, mirrored_tex);
            } else {
                world.textures.insert(name, tex);
            }
        }
        let sprite_count = if end > start { end - start - 1 } else { 0 };

        let sprite_dir = std::path::Path::new("/tmp/doom_sprites");
        let _ = std::fs::create_dir_all(sprite_dir);
        for (name, tex) in &world.textures {
            // Only export sprites (typically 4-8 character names ending with letter+number)
            if name.len() <= 8 && tex.width > 0 && tex.height > 0 {
                let path = sprite_dir.join(format!("{}.png", name));
                if let Some(img) =
                    image::RgbaImage::from_raw(tex.width, tex.height, tex.pixels.clone())
                {
                    let _ = img.save(&path);
                }
            }
        }
        log::info!(
            "WadLoader: Loaded {} sprites (exported to /tmp/doom_sprites/)",
            sprite_count
        );
        Ok(())
    }

    pub fn load_sounds(&self) -> std::collections::HashMap<String, Vec<u8>> {
        let mut sounds = std::collections::HashMap::new();

        let sound_lumps: Vec<_> = self
            .directory
            .iter()
            .filter(|e| e.name.starts_with("DS"))
            .collect();

        for entry in sound_lumps {
            if entry.size < 8 {
                continue;
            }
            let data = &self.data[entry.file_pos as usize..(entry.file_pos + entry.size) as usize];
            sounds.insert(entry.name.clone(), data.to_vec());
        }
        log::info!("WadLoader: Loaded {} sound lumps", sounds.len());
        sounds
    }

    fn load_flats(
        &self,
        world: &mut crate::simulation::WorldState,
        palette: &WadPalette,
    ) -> anyhow::Result<()> {
        let markers = [
            ("F_START", "F_END"),
            ("FF_START", "FF_END"),
            ("F1_START", "F1_END"),
            ("F2_START", "F2_END"),
        ];

        let mut flat_count = 0;
        for (start_marker, end_marker) in markers {
            let start_idx = self.directory.iter().position(|e| e.name == start_marker);
            let end_idx = self.directory.iter().position(|e| e.name == end_marker);

            if let (Some(s), Some(e)) = (start_idx, end_idx) {
                for i in (s + 1)..e {
                    let lump = &self.directory[i];
                    if lump.size != 4096 {
                        continue;
                    }

                    let flat_data = self.get_lump_data(lump.name.as_str()).unwrap();
                    let mut pixels = vec![0u8; 64 * 64 * 4];
                    let mut pixels_idx = vec![-1i16; 64 * 64];
                    for (j, &pixel_idx) in flat_data.iter().enumerate() {
                        if j >= 64 * 64 {
                            break;
                        }
                        pixels_idx[j] = pixel_idx as i16;
                        let color = palette.colors[pixel_idx as usize];
                        pixels[j * 4..j * 4 + 4]
                            .copy_from_slice(&[color[0], color[1], color[2], 255]);
                    }
                    world.textures.insert(
                        lump.name.clone().to_uppercase(),
                        crate::simulation::Texture {
                            name: lump.name.clone(),
                            width: 64,
                            height: 64,
                            left_offset: 0,
                            top_offset: 0,
                            pixels,
                            pixels_indexed: pixels_idx,
                        },
                    );
                    flat_count += 1;
                }
            }
        }
        log::info!("WadLoader: Loaded {} flats", flat_count);
        Ok(())
    }

    fn composite_patch_indexed(
        &self,
        dest: &mut [u8],
        dest_idx: &mut [i16],
        dest_w: u16,
        dest_h: u16,
        patch_data: &[u8],
        origin_x: i16,
        origin_y: i16,
        palette: &WadPalette,
    ) -> anyhow::Result<()> {
        let mut cursor = Cursor::new(patch_data);
        let header = WadPatchHeader::read(&mut cursor)?;

        let mut column_offsets = Vec::new();
        for _ in 0..header.width {
            column_offsets.push(u32::read_le(&mut cursor)?);
        }

        for (x, &col_offset) in column_offsets.iter().enumerate() {
            let target_x = origin_x + x as i16;
            if target_x < 0 || target_x >= dest_w as i16 {
                continue;
            }

            cursor.set_position(col_offset as u64);
            loop {
                let top_delta: u8 = BinRead::read_le(&mut cursor)?;
                if top_delta == 0xFF {
                    break;
                }

                let length: u8 = BinRead::read_le(&mut cursor)?;
                let _unused: u8 = BinRead::read_le(&mut cursor)?; // Padding

                for i in 0..length {
                    let pixel_idx: u8 = BinRead::read_le(&mut cursor)?;
                    let target_y = origin_y + top_delta as i16 + i as i16;

                    if target_y >= 0 && target_y < dest_h as i16 {
                        let idx = target_y as usize * dest_w as usize + target_x as usize;
                        dest_idx[idx] = pixel_idx as i16;

                        let color = palette.colors[pixel_idx as usize];
                        let offset = idx * 4;
                        dest[offset..offset + 4]
                            .copy_from_slice(&[color[0], color[1], color[2], 255]);
                    }
                }
                let _unused_end: u8 = BinRead::read_le(&mut cursor)?; // Padding
            }
        }
        Ok(())
    }

    /// Finds a map by name (e.g. "E1M1") and its subsequent lumps.
    pub fn load_map(&self, map_name: &str) -> anyhow::Result<crate::simulation::WorldState> {
        let map_index = self
            .directory
            .iter()
            .position(|e| e.name == map_name)
            .ok_or_else(|| anyhow::anyhow!("Map not found"))?;

        // Order: THINGS, LINEDEFS, SIDEDEFS, VERTEXES, SEGS, SSECTORS, NODES, SECTORS, REJECT, BLOCKMAP
        let thing_data = &self.directory[map_index + 1];
        let linedef_data = &self.directory[map_index + 2];
        let sidedef_data = &self.directory[map_index + 3];
        let vertex_data = &self.directory[map_index + 4];
        let seg_data = &self.directory[map_index + 5];
        let ssector_data = &self.directory[map_index + 6];
        let node_data = &self.directory[map_index + 7];
        let sector_data = &self.directory[map_index + 8];

        let mut world = crate::simulation::WorldState::new();

        // ... vertices and things parsing same as before ... (I'll keep the full block for clarity)

        // Parse Vertices
        let mut v_cursor = Cursor::new(
            &self.data
                [vertex_data.file_pos as usize..(vertex_data.file_pos + vertex_data.size) as usize],
        );
        while v_cursor.position() < vertex_data.size as u64 {
            let v = WadVertex::read(&mut v_cursor)?;
            world.vertices.push(glam::Vec2::new(v.x as f32, v.y as f32));
        }

        // Parse Segs
        let mut seg_cursor = Cursor::new(
            &self.data[seg_data.file_pos as usize..(seg_data.file_pos + seg_data.size) as usize],
        );
        while seg_cursor.position() < seg_data.size as u64 {
            let s = WadSeg::read(&mut seg_cursor)?;
            world.segs.push(crate::simulation::Seg {
                start_idx: s.start_vertex as usize,
                end_idx: s.end_vertex as usize,
                linedef_idx: s.linedef_index as usize,
                side: s.direction as usize,
                offset: s.offset as f32,
            });
        }

        // Parse Subsectors
        let mut ssec_cursor = Cursor::new(
            &self.data[ssector_data.file_pos as usize
                ..(ssector_data.file_pos + ssector_data.size) as usize],
        );
        while ssec_cursor.position() < ssector_data.size as u64 {
            let ss = WadSubsector::read(&mut ssec_cursor)?;
            world.subsectors.push(crate::simulation::Subsector {
                seg_count: ss.seg_count as usize,
                first_seg_idx: ss.first_seg_index as usize,
            });
        }

        // Parse Nodes
        let mut n_cursor = Cursor::new(
            &self.data[node_data.file_pos as usize..(node_data.file_pos + node_data.size) as usize],
        );
        while n_cursor.position() < node_data.size as u64 {
            let n = WadNode::read(&mut n_cursor)?;
            world.nodes.push(crate::simulation::BspNode {
                x: n.x as f32,
                y: n.y as f32,
                dx: n.dx as f32,
                dy: n.dy as f32,
                bbox_right: [
                    n.bbox_right[0] as f32,
                    n.bbox_right[1] as f32,
                    n.bbox_right[2] as f32,
                    n.bbox_right[3] as f32,
                ],
                bbox_left: [
                    n.bbox_left[0] as f32,
                    n.bbox_left[1] as f32,
                    n.bbox_left[2] as f32,
                    n.bbox_left[3] as f32,
                ],
                child_right: n.child_right,
                child_left: n.child_left,
            });
        }

        // Parse Sectors
        let mut sec_cursor = Cursor::new(
            &self.data
                [sector_data.file_pos as usize..(sector_data.file_pos + sector_data.size) as usize],
        );
        while sec_cursor.position() < sector_data.size as u64 {
            let s = WadSector::read(&mut sec_cursor)?;
            let mut sector = crate::simulation::Sector {
                floor_height: s.floor_height as f32,
                ceiling_height: s.ceiling_height as f32,
                light_level: (s.light_level as f32 / 255.0).clamp(0.0, 1.0),
                texture_floor: String::from_utf8_lossy(&s.texture_floor)
                    .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                    .to_string()
                    .to_uppercase(),
                texture_ceiling: String::from_utf8_lossy(&s.texture_ceiling)
                    .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                    .to_string()
                    .to_uppercase(),
                tag: s.tag,
                action: crate::simulation::SectorAction::None,
                special_type: s.special_type as u16,
                secret_found: false,
            };

            if sector.special_type == 9 {
                world.total_secrets += 1;
            }

            // Initialize Light Animations
            match sector.special_type {
                1 => {
                    // Blink Off (Random)
                    sector.action = crate::simulation::SectorAction::Light {
                        effect: crate::simulation::LightEffect::BlinkOff,
                        timer: 1.0,
                        base_light: sector.light_level,
                        alt_light: 0.1,
                    };
                }
                2 | 4 => {
                    // Strobe Fast
                    sector.action = crate::simulation::SectorAction::Light {
                        effect: crate::simulation::LightEffect::StrobeFast,
                        timer: 0.1,
                        base_light: sector.light_level,
                        alt_light: 0.1,
                    };
                }
                3 | 13 => {
                    // Strobe Slow
                    sector.action = crate::simulation::SectorAction::Light {
                        effect: crate::simulation::LightEffect::StrobeSlow,
                        timer: 0.5,
                        base_light: sector.light_level,
                        alt_light: 0.1,
                    };
                }
                17 => {
                    // Flicker
                    sector.action = crate::simulation::SectorAction::Light {
                        effect: crate::simulation::LightEffect::Flicker,
                        timer: 0.1,
                        base_light: sector.light_level,
                        alt_light: (sector.light_level - 0.2).max(0.0),
                    };
                }
                8 => {
                    // Glow
                    sector.action = crate::simulation::SectorAction::Light {
                        effect: crate::simulation::LightEffect::Glow,
                        timer: 0.1,
                        base_light: sector.light_level,
                        alt_light: (sector.light_level - 0.3).max(0.0),
                    };
                }
                _ => {}
            }

            world.sectors.push(sector);
        }

        // Parse Sidedefs
        let mut sidedefs = Vec::new();
        let mut side_cursor = Cursor::new(
            &self.data[sidedef_data.file_pos as usize
                ..(sidedef_data.file_pos + sidedef_data.size) as usize],
        );
        while side_cursor.position() < sidedef_data.size as u64 {
            sidedefs.push(WadSidedef::read(&mut side_cursor)?);
        }

        // Parse Linedefs
        let mut l_cursor = Cursor::new(
            &self.data[linedef_data.file_pos as usize
                ..(linedef_data.file_pos + linedef_data.size) as usize],
        );
        while l_cursor.position() < linedef_data.size as u64 {
            let l = WadLinedef::read(&mut l_cursor)?;

            let load_side =
                |sidenum: u16, sidedefs: &[WadSidedef]| -> Option<crate::simulation::Sidedef> {
                    if sidenum == 0xFFFF {
                        return None;
                    }
                    let side = &sidedefs[sidenum as usize];
                    let mid = String::from_utf8_lossy(&side.texture_middle)
                        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                        .to_string()
                        .to_uppercase();
                    let up = String::from_utf8_lossy(&side.texture_upper)
                        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                        .to_string()
                        .to_uppercase();
                    let low = String::from_utf8_lossy(&side.texture_lower)
                        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                        .to_string()
                        .to_uppercase();

                    Some(crate::simulation::Sidedef {
                        texture_middle: if mid != "-" { Some(mid) } else { None },
                        texture_upper: if up != "-" { Some(up) } else { None },
                        texture_lower: if low != "-" { Some(low) } else { None },
                        sector_idx: side.sector_id as usize,
                        x_offset: side.texture_offset_x as f32,
                        y_offset: side.texture_offset_y as f32,
                    })
                };

            let front = load_side(l.sidenum_front, &sidedefs);
            let back = load_side(l.sidenum_back, &sidedefs);

            world.linedefs.push(crate::simulation::LineDefinition {
                start_idx: l.start_vertex as usize,
                end_idx: l.end_vertex as usize,
                sector_front: front.as_ref().map(|s| s.sector_idx),
                sector_back: back.as_ref().map(|s| s.sector_idx),
                front,
                back,
                special_type: l.special_type,
                sector_tag: l.sector_tag,
                flags: l.flags,
                activated: false,
            });
        }

        // Parse Things (Must happen AFTER Sectors, Segs, Subsectors, Nodes, and Linedefs are fully loaded so we can find altitude)
        let mut t_cursor = Cursor::new(
            &self.data
                [thing_data.file_pos as usize..(thing_data.file_pos + thing_data.size) as usize],
        );
        while t_cursor.position() < thing_data.size as u64 {
            let t = WadThing::read(&mut t_cursor)?;
            let angle_rad = t.angle as f32 * std::f32::consts::PI / 180.0;

            // Special handling for player starts (Kinds 1-4 and 11)
            if matches!(t.kind, 1..=4 | 11) {
                if t.kind == 1 {
                    world.player.position = glam::Vec2::new(t.x as f32, t.y as f32);
                    world.player.angle = angle_rad;
                    world.player_start_pos = world.player.position;
                }
                continue; // DO NOT add to world.things
            }

            let pos = glam::Vec2::new(t.x as f32, t.y as f32);

            // Find the sector floor height for proper Z positioning
            let z = if let Some(sector_idx) = world.find_sector_at(pos) {
                world.sectors[sector_idx].floor_height
            } else {
                0.0 // Default to 0 if sector not found
            };

            let thing = crate::simulation::Thing {
                position: pos,
                z,
                angle: angle_rad,
                kind: t.kind,
                flags: t.flags,
                health: 100.0, // Initialized later by game logic
                picked_up: false,
                state_idx: 0, // Initialized later by game logic
                ai_timer: 0,
                target_thing_idx: None,
                attack_cooldown: 0,
            };

            world.things.push(thing);
        }

        log::info!(
            "WadLoader: Loaded map '{}' with {} vertices, {} linedefs, {} sectors, {} nodes",
            map_name,
            world.vertices.len(),
            world.linedefs.len(),
            world.sectors.len(),
            world.nodes.len()
        );

        // Build adjacency map for sound propagation
        world.adjacent_sectors = vec![Vec::new(); world.sectors.len()];
        for line in &world.linedefs {
            if let (Some(f), Some(b)) = (line.sector_front, line.sector_back) {
                if !world.adjacent_sectors[f].contains(&b) {
                    world.adjacent_sectors[f].push(b);
                }
                if !world.adjacent_sectors[b].contains(&f) {
                    world.adjacent_sectors[b].push(f);
                }
            }
        }

        // Initialize player Z to sector floor height
        if let Some(sid) = world.find_sector_at(world.player.position) {
            world.player.z = world.sectors[sid].floor_height;
            log::info!(
                "WadLoader: Player Z initialized to sector floor: {}",
                world.player.z
            );
        }

        Ok(world)
    }

    pub fn load_lighting(&self, world: &mut crate::simulation::WorldState) -> anyhow::Result<()> {
        if let Some(data) = self.get_lump_data("COLORMAP") {
            world.colormap = data.to_vec();
            log::info!("WadLoader: Loaded COLORMAP lump ({} bytes)", data.len());
        }
        if let Some(data) = self.get_lump_data("PLAYPAL") {
            let mut cursor = Cursor::new(data);
            let num_palettes = data.len() / 768;
            for _ in 0..num_palettes {
                let mut palette = vec![0u8; 768];
                if cursor.read_exact(&mut palette).is_ok() {
                    world.palettes.push(palette);
                }
            }
            log::info!(
                "WadLoader: Loaded {} palettes from PLAYPAL",
                world.palettes.len()
            );
        }

        // Safety: Ensure at least one palette exists
        if world.palettes.is_empty() {
            log::warn!("WadLoader: No palettes loaded, creating grayscale default.");
            let mut gray = vec![0u8; 768];
            for i in 0..256 {
                gray[i * 3] = i as u8;
                gray[i * 3 + 1] = i as u8;
                gray[i * 3 + 2] = i as u8;
            }
            world.palettes.push(gray);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_wad_header_parsing() {
        let mut data = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&10u32.to_le_bytes()); // 10 lumps
        data.extend_from_slice(&100u32.to_le_bytes()); // offset 100

        let mut cursor = Cursor::new(data);
        let header = WadHeader::read(&mut cursor).unwrap();

        assert_eq!(&header.wad_type, b"IWAD");
        assert_eq!(header.num_lumps, 10);
        assert_eq!(header.directory_offset, 100);
    }

    #[test]
    fn test_wad_loader_basic() {
        // Create a minimal valid WAD in memory
        let mut data = vec![0u8; 200];
        data[0..4].copy_from_slice(b"IWAD");
        data[4..8].copy_from_slice(&1u32.to_le_bytes()); // 1 lump
        data[8..12].copy_from_slice(&12u32.to_le_bytes()); // dir at 12

        // Directory entry
        data[12..16].copy_from_slice(&64u32.to_le_bytes()); // file pos 64
        data[16..20].copy_from_slice(&4u32.to_le_bytes()); // size 4
        data[20..28].copy_from_slice(b"TESTLUMP");

        // Lump data
        data[64..68].copy_from_slice(&[1, 2, 3, 4]);

        let loader = WadLoader::new(data).unwrap();
        assert_eq!(loader.directory.len(), 1);
        assert_eq!(loader.directory[0].name, "TESTLUMP");

        let lump = loader.get_lump_data("TESTLUMP").unwrap();
        assert_eq!(lump, &[1, 2, 3, 4]);
    }
}
