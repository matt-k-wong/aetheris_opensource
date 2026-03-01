#[cfg(feature = "opl_music")]
use crate::infrastructure::music_parser::{MidiSequencer, MusEvent, MusSequencer};
#[cfg(feature = "opl_music")]
use opl::chip::Chip;
#[cfg(feature = "opl_music")]
use rodio::source::Source;

#[cfg(feature = "opl_music")]
enum SequencerType {
    Mus(MusSequencer),
    Midi(MidiSequencer),
}

#[cfg(feature = "opl_music")]
impl SequencerType {
    fn next_event(&mut self) -> Option<(u32, MusEvent)> {
        match self {
            SequencerType::Mus(s) => s.next_event(),
            SequencerType::Midi(s) => s.next_event(),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            SequencerType::Mus(s) => s.finished,
            SequencerType::Midi(s) => s.finished,
        }
    }

    fn restart(&mut self) {
        match self {
            SequencerType::Mus(s) => s.restart(),
            SequencerType::Midi(s) => s.restart(),
        }
    }
}

// OPL3 uses 18 channels. We allocate channels so we can map MIDI notes to OPL channels.
#[cfg(feature = "opl_music")]
#[derive(Clone, Copy)]
struct ChannelAllocation {
    midi_channel: u8,
    midi_note: u8,
    instrument_id: u16,
    active: bool,
    time_assigned: u64,
}

// DOOM's GENMIDI lump provides 175 instruments. Each instrument has 2 operator settings.
// OPL2 uses 2 operators per channel. Doom patches can be either single-voice (1 OPL channel)
// or double-voice (2 OPL channels). For simplicity, we'll start with single-voice mapping.
#[cfg(feature = "opl_music")]
#[derive(Clone, Copy, Default)]
struct OplInstrument {
    flags: u16,
    fine_tuning: u8,
    fixed_note: u8,

    // Voice 0 Modulator
    mod_20: u8,
    mod_60: u8,
    mod_80: u8,
    mod_e0: u8,
    mod_40_ksl: u8,
    mod_40_tl: u8,
    feedback: u8,

    // Voice 0 Carrier
    car_20: u8,
    car_60: u8,
    car_80: u8,
    car_e0: u8,
    car_40_ksl: u8,
    car_40_tl: u8,

    base_note_offset: i16,
}

#[cfg(feature = "opl_music")]
struct GenMidi {
    instruments: Vec<OplInstrument>,
}

#[cfg(feature = "opl_music")]
impl GenMidi {
    fn new(data: &[u8]) -> Self {
        // GENMIDI starts with "GENMIDI\0", then 175 instrument definitions.
        // Each instrument is 36 bytes.
        let mut instruments = vec![OplInstrument::default(); 175];
        if data.len() < 8 + 175 * 36 {
            return Self { instruments };
        }

        let mut pos = 8;
        for i in 0..175 {
            let inst = &mut instruments[i];
            inst.flags = u16::from_le_bytes([data[pos], data[pos + 1]]);
            inst.fine_tuning = data[pos + 2];
            inst.fixed_note = data[pos + 3];

            // Voice 0 Modulator
            inst.mod_20 = data[pos + 4];
            inst.mod_60 = data[pos + 5];
            inst.mod_80 = data[pos + 6];
            inst.mod_e0 = data[pos + 7];
            inst.mod_40_ksl = data[pos + 8];
            inst.mod_40_tl = data[pos + 9];

            inst.feedback = data[pos + 10];

            // Voice 0 Carrier
            inst.car_20 = data[pos + 11];
            inst.car_60 = data[pos + 12];
            inst.car_80 = data[pos + 13];
            inst.car_e0 = data[pos + 14];
            inst.car_40_ksl = data[pos + 15];
            inst.car_40_tl = data[pos + 16];

            // Unused byte at 17
            inst.base_note_offset = i16::from_le_bytes([data[pos + 18], data[pos + 19]]);

            // 20-35 is Voice 1 (for 4OP / Double Voice), ignored for now to map OPL2.

            pos += 36;
        }
        Self { instruments }
    }
}

#[cfg(feature = "opl_music")]
pub struct Opl3Bridge {
    opl: Chip,
    seq: SequencerType,
    genmidi: GenMidi,

    samples_until_seq: f64,
    temp_buffer: Vec<i16>,
    current_idx: usize,
    sample_rate: u32,

    allocations: [ChannelAllocation; 18],
    chan_patch: [u8; 16],
    chan_vol: [f32; 16],
    chan_bend: [f32; 16],
    time: u64,
}

#[cfg(feature = "opl_music")]
impl Opl3Bridge {
    pub fn new(mus_data: Vec<u8>, genmidi_data: Vec<u8>) -> Self {
        let mut opl = Chip::new(44100);
        opl.setup();
        let genmidi = GenMidi::new(&genmidi_data);

        let seq = if mus_data.starts_with(b"MThd") {
            log::info!("Opl3Bridge: Detected MThd standard MIDI track");
            SequencerType::Midi(MidiSequencer::new(mus_data))
        } else {
            log::info!("Opl3Bridge: Detected classic MUS\\x1A proprietary track");
            SequencerType::Mus(MusSequencer::new(mus_data))
        };

        // Initialize OPL settings (enable OPL3, etc)
        // ... (We'll send basic init commands)

        Self {
            opl,
            seq,
            genmidi,
            samples_until_seq: 0.0,
            temp_buffer: vec![0; 256],
            current_idx: 256,
            sample_rate: 44100,
            allocations: [ChannelAllocation {
                midi_channel: 0,
                midi_note: 0,
                instrument_id: 0,
                active: false,
                time_assigned: 0,
            }; 18],
            chan_patch: [0; 16],
            chan_vol: [1.0; 16],
            chan_bend: [1.0; 16],
            time: 0,
        }
    }

    fn write_reg(&mut self, reg: u16, val: u8) {
        self.opl.write_reg(reg as u32, val);
    }

    fn get_op_offsets(opl_chan: usize) -> (u16, u16) {
        let bank = if opl_chan < 9 { 0x000 } else { 0x100 };
        let c = opl_chan % 9;
        let set = c / 3;
        let offset = c % 3;
        let base = bank + (set * 8) as u16 + offset as u16;
        (base, base + 3)
    }

    fn assign_instrument(&mut self, opl_chan: usize, inst_id: u16) {
        if inst_id as usize >= self.genmidi.instruments.len() {
            return;
        }
        let inst = self.genmidi.instruments[inst_id as usize];
        let (op1, op2) = Self::get_op_offsets(opl_chan);

        self.write_reg(0x20 + op1, inst.mod_20);
        self.write_reg(0x20 + op2, inst.car_20);

        self.write_reg(0x40 + op1, (inst.mod_40_ksl & 0xC0) | 0x3F); // Mute initially (tl)
        self.write_reg(0x40 + op2, (inst.car_40_ksl & 0xC0) | 0x3F);

        self.write_reg(0x60 + op1, inst.mod_60);
        self.write_reg(0x60 + op2, inst.car_60);

        self.write_reg(0x80 + op1, inst.mod_80);
        self.write_reg(0x80 + op2, inst.car_80);

        self.write_reg(0xE0 + op1, inst.mod_e0 & 7); // Mask waveform
        self.write_reg(0xE0 + op2, inst.car_e0 & 7);

        let bank = if opl_chan < 9 { 0x000 } else { 0x100 };
        let c = (opl_chan % 9) as u16;
        self.write_reg(0xC0 + bank + c, inst.feedback | 0x30); // 0x30 = output to both L+R
    }

    fn note_on(&mut self, chan: u8, note: u8, vol: u8) {
        let inst_id = if chan == 15 {
            // In DOOM, drum patches are located at index 128 through 174 (note - 35 + 128)
            let n = if note < 35 { 0 } else { note - 35 };
            if 128 + n < 175 { 128 + n as u16 } else { 128 }
        } else {
            self.chan_patch[chan as usize] as u16
        };

        // Find free channel
        let mut best_chan = 0;
        let mut oldest_time = u64::MAX;

        for i in 0..9 {
            if !self.allocations[i].active {
                best_chan = i;
                break;
            }
            if self.allocations[i].time_assigned < oldest_time {
                oldest_time = self.allocations[i].time_assigned;
                best_chan = i;
            }
        }

        // Kill previous note on this channel just in case
        let bank = if best_chan < 9 { 0x000 } else { 0x100 };
        let c = (best_chan % 9) as u16;
        self.write_reg(0xB0 + bank + c, 0);

        self.allocations[best_chan] = ChannelAllocation {
            midi_channel: chan,
            midi_note: note,
            instrument_id: inst_id,
            active: true,
            time_assigned: self.time,
        };

        self.assign_instrument(best_chan, inst_id);

        let inst = self.genmidi.instruments[inst_id as usize];
        // Compute frequency and best block
        let mut adj_note = note as i32;
        if (inst.flags & 1) == 0 {
            // Not fixed pitch
            adj_note += inst.base_note_offset as i32;
        } else {
            adj_note = inst.fixed_note as i32;
        }
        while adj_note < 0 {
            adj_note += 12;
        }
        while adj_note > 127 {
            adj_note -= 12;
        }

        let freq = 440.0
            * 2.0f32.powf(((adj_note as i32) - 69) as f32 / 12.0)
            * self.chan_bend[chan as usize];
        let mut block = 0;
        let mut fnum = (freq * 1048576.0 / 49716.0) as u32; // fnum for block 0
        while fnum >= 1024 && block < 7 {
            fnum >>= 1;
            block += 1;
        }
        let fnum_u16 = fnum.min(1023) as u16;

        self.write_reg(0xA0 + bank + c, (fnum_u16 & 0xFF) as u8);
        self.write_reg(
            0xB0 + bank + c,
            0x20 | (block << 2) | ((fnum_u16 >> 8) & 0x03) as u8,
        ); // 0x20 = Key-On

        // Output Total Level mapping
        let (op1, op2) = Self::get_op_offsets(best_chan);
        let atten = (127 - vol.min(127)) / 2; // e.g., vol=127 -> atten=0. vol=0 -> atten=63

        // Additive synthesis (FM bit == 1) means both ops produce sound. Otherwise only OP2 produces sound.
        if (inst.feedback & 1) == 1 && inst.mod_40_tl != 0x3F {
            let base_tl0 = inst.mod_40_tl & 0x3F;
            let new_tl0 = (base_tl0 + atten).min(63);
            self.write_reg(0x40 + op1, (inst.mod_40_ksl & 0xC0) | new_tl0);
        } else {
            // Reapply original OP1 TL
            self.write_reg(
                0x40 + op1,
                (inst.mod_40_ksl & 0xC0) | (inst.mod_40_tl & 0x3F),
            );
        }
        let base_tl1 = inst.car_40_tl & 0x3F;
        let new_tl1 = (base_tl1 + atten).min(63);
        self.write_reg(0x40 + op2, (inst.car_40_ksl & 0xC0) | new_tl1);
    }

    fn note_off(&mut self, chan: u8, note: u8) {
        for i in 0..9 {
            if self.allocations[i].active
                && self.allocations[i].midi_channel == chan
                && self.allocations[i].midi_note == note
            {
                self.allocations[i].active = false;
                let bank = if i < 9 { 0x000 } else { 0x100 };
                let c = (i % 9) as u16;
                // Read from OPL isn't supported directly, so we just blindly clear key-on
                self.write_reg(0xB0 + bank + c, 0); // Clear Key-On
            }
        }
    }
}

#[cfg(feature = "opl_music")]
impl Iterator for Opl3Bridge {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_idx >= self.temp_buffer.len() {
            while self.samples_until_seq <= 0.0 && !self.seq.is_finished() {
                if let Some((delta, event)) = self.seq.next_event() {
                    match event {
                        MusEvent::Controller { chan, ctrl, val } => {
                            if ctrl == 0 {
                                self.chan_patch[chan as usize] = val;
                            } else if ctrl == 3 {
                                self.chan_vol[chan as usize] = val as f32 / 127.0;
                            }
                        }
                        MusEvent::NoteOn { chan, note, vol } => {
                            if let Some(0) = vol {
                                self.note_off(chan, note);
                            } else if let Some(v) = vol {
                                let adj_v = (v as f32 * self.chan_vol[chan as usize]) as u8;
                                self.note_on(chan, note, adj_v);
                            }
                        }
                        MusEvent::NoteOff { chan, note } => self.note_off(chan, note),
                        MusEvent::PitchBend { chan, val } => {
                            let semitones = ((val as f32) - 128.0) / 64.0;
                            self.chan_bend[chan as usize] = 2.0f32.powf(semitones / 12.0);
                            for i in 0..9 {
                                if self.allocations[i].active
                                    && self.allocations[i].midi_channel == chan
                                {
                                    let note = self.allocations[i].midi_note;
                                    let inst = self.genmidi.instruments
                                        [self.allocations[i].instrument_id as usize];
                                    let mut adj_note = note as i32;
                                    if (inst.flags & 1) == 0 {
                                        adj_note += inst.base_note_offset as i32;
                                    } else {
                                        adj_note = inst.fixed_note as i32;
                                    }
                                    while adj_note < 0 {
                                        adj_note += 12;
                                    }
                                    while adj_note > 127 {
                                        adj_note -= 12;
                                    }

                                    let freq = 440.0
                                        * 2.0f32.powf(((adj_note as i32) - 69) as f32 / 12.0)
                                        * self.chan_bend[chan as usize];
                                    let mut block = 0;
                                    let mut fnum = (freq * 1048576.0 / 49716.0) as u32;
                                    while fnum >= 1024 && block < 7 {
                                        fnum >>= 1;
                                        block += 1;
                                    }
                                    let fnum_u16 = fnum.min(1023) as u16;
                                    let bank = if i < 9 { 0x000 } else { 0x100 };
                                    let c = (i % 9) as u16;
                                    self.write_reg(0xA0 + bank + c, (fnum_u16 & 0xFF) as u8);
                                    self.write_reg(
                                        0xB0 + bank + c,
                                        0x20 | (block << 2) | ((fnum_u16 >> 8) & 0x03) as u8,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                    self.samples_until_seq += (delta as f64 * self.sample_rate as f64) / 140.0;
                } else {
                    self.samples_until_seq = 1000000.0;
                }
            }

            // OplEmu fills a mono slice of i32 samples because OPL3 is disabled
            let mut buf_i32 = vec![0; self.temp_buffer.len()];
            self.opl
                .generate_block_2(self.temp_buffer.len(), &mut buf_i32);
            for i in 0..self.temp_buffer.len() {
                self.temp_buffer[i] = buf_i32[i].clamp(-32768, 32767) as i16;
            }
            self.samples_until_seq -= self.temp_buffer.len() as f64;
            self.current_idx = 0;
            self.time += 1;
        }
        // Close `if self.current_idx >= self.temp_buffer.len()`

        #[cfg(feature = "opl_music")]
        {
            if self.seq.is_finished() {
                self.seq.restart();
            }
        }

        // Boost output gain significantly (OPL3 naturally has very low amplitude on many patches)
        let sample = (self.temp_buffer[self.current_idx] as f32 / 32768.0) * 8.0;
        self.current_idx += 1;
        Some(sample)
    }
}

#[cfg(feature = "opl_music")]
impl Source for Opl3Bridge {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

pub struct MusicEngine {
    handle: rodio::OutputStreamHandle,
    sink: rodio::Sink,
}

impl MusicEngine {
    pub fn new(handle: &rodio::OutputStreamHandle) -> anyhow::Result<Self> {
        let sink = rodio::Sink::try_new(handle)?;
        Ok(Self {
            handle: handle.clone(),
            sink,
        })
    }

    pub fn play_map_music(
        &mut self,
        loader: &crate::assets::wad::WadLoader,
        map_name: &str,
    ) -> anyhow::Result<()> {
        let music_lump = format!("D_{}", map_name);
        #[allow(unused_variables)]
        if let Some(mus_data) = loader.get_lump_data(&music_lump) {
            #[cfg(feature = "opl_music")]
            {
                if let Some(genmidi_data) = loader.get_lump_data("GENMIDI") {
                    log::info!("Music: Booting OPL3 Bridge for track {}...", music_lump);
                    let source = Opl3Bridge::new(mus_data.to_vec(), genmidi_data.to_vec());
                    self.sink = rodio::Sink::try_new(&self.handle)?;
                    self.sink.append(source);
                    self.sink.play();
                } else {
                    log::warn!("Music: GENMIDI lump not found! Cannot initialize OPL instruments.");
                }
            }
            #[cfg(not(feature = "opl_music"))]
            {
                // Silently sink the data
                log::info!(
                    "Music: Track {} found, but 'opl_music' feature is disabled.",
                    music_lump
                );
                self.sink = rodio::Sink::try_new(&self.handle)?;
            }
        } else {
            log::warn!("Music: Track {} not found", music_lump);
        }
        Ok(())
    }

    pub fn set_volume(&self, volume: u32) {
        self.sink.set_volume(volume as f32 / 100.0);
    }
    pub fn update(&self, _frame_count: u64) {}
}
