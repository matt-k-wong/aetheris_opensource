#[derive(Debug, Clone, Copy)]
pub enum MusEvent {
    NoteOff { chan: u8, note: u8 },
    NoteOn { chan: u8, note: u8, vol: Option<u8> },
    PitchBend { chan: u8, val: u8 },
    System { chan: u8, ctrl: u8 },
    Controller { chan: u8, ctrl: u8, val: u8 },
    End,
}

#[derive(Clone)]
pub struct MusSequencer {
    pub data: Vec<u8>,
    pub cursor: usize,
    pub finished: bool,
}

impl MusSequencer {
    pub fn new(data: Vec<u8>) -> Self {
        let mut seq = Self {
            data,
            cursor: 0,
            finished: false,
        };
        if seq.data.len() > 6 {
            let score_start = u16::from_le_bytes([seq.data[6], seq.data[7]]) as usize;
            seq.cursor = score_start;
        }
        seq
    }

    pub fn restart(&mut self) {
        if self.data.len() > 6 {
            self.cursor = u16::from_le_bytes([self.data[6], self.data[7]]) as usize;
        } else {
            self.cursor = 0;
        }
        self.finished = false;
    }

    pub fn next_event(&mut self) -> Option<(u32, MusEvent)> {
        if self.finished || self.cursor >= self.data.len() {
            return None;
        }
        let b = self.data[self.cursor];
        self.cursor += 1;
        let last = (b & 0x80) != 0;
        let etype = (b & 0x70) >> 4;
        let chan = b & 0x0F;
        let event = match etype {
            0 => MusEvent::NoteOff {
                chan,
                note: self.read_u8() & 0x7F,
            },
            1 => {
                let note = self.read_u8();
                let vol = if (note & 0x80) != 0 {
                    Some(self.read_u8() & 0x7F)
                } else {
                    None
                };
                MusEvent::NoteOn {
                    chan,
                    note: note & 0x7F,
                    vol,
                }
            }
            2 => MusEvent::PitchBend {
                chan,
                val: self.read_u8(),
            },
            3 => MusEvent::System {
                chan,
                ctrl: self.read_u8(),
            },
            4 => MusEvent::Controller {
                chan,
                ctrl: self.read_u8(),
                val: self.read_u8(),
            },
            6 => {
                self.finished = true;
                MusEvent::End
            }
            _ => MusEvent::End,
        };
        let mut delta = 0u32;
        if last {
            loop {
                let d = self.read_u8();
                delta = (delta << 7) | (d & 0x7F) as u32;
                if (d & 0x80) == 0 {
                    break;
                }
            }
        }
        Some((delta, event))
    }

    fn read_u8(&mut self) -> u8 {
        if self.cursor >= self.data.len() {
            return 0;
        }
        let b = self.data[self.cursor];
        self.cursor += 1;
        b
    }
}

#[derive(Clone)]
pub struct MidiSequencer {
    pub events: Vec<(u32, MusEvent)>,
    pub current_idx: usize,
    pub finished: bool,
}

impl MidiSequencer {
    pub fn new(data: Vec<u8>) -> Self {
        if data.len() < 14 {
            return Self {
                events: vec![],
                current_idx: 0,
                finished: true,
            };
        }

        let format = u16::from_be_bytes([data[8], data[9]]);
        let num_tracks = u16::from_be_bytes([data[10], data[11]]);
        let division = u16::from_be_bytes([data[12], data[13]]);
        let ppqn = if (division & 0x8000) == 0 {
            division as f64
        } else {
            120.0
        };

        // 1. Find all MTrk chunks
        let mut track_offsets = Vec::new();
        let mut i = 14;
        while i < data.len() - 8 {
            if data[i] == b'M' && data[i + 1] == b'T' && data[i + 2] == b'r' && data[i + 3] == b'k'
            {
                let len = u32::from_be_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]])
                    as usize;
                track_offsets.push((i + 8, i + 8 + len));
                i += 8 + len;
            } else {
                i += 1;
            }
        }

        // 2. Parse all events into a unified list (absolute_ticks, data_bytes)
        #[derive(Clone, Debug)]
        struct RawMidiEvent {
            abs_ticks: u32,
            status: u8,
            data_bytes: Vec<u8>,
        }
        let mut all_events = Vec::new();

        for (start, end) in track_offsets {
            let mut cursor = start;
            let end_limit = end.min(data.len());
            let mut abs_ticks = 0;
            let mut running_status = 0u8;

            let read_u8 = |c: &mut usize| -> u8 {
                if *c < end_limit {
                    let b = data[*c];
                    *c += 1;
                    b
                } else {
                    0
                }
            };

            let read_var_len = |c: &mut usize| -> u32 {
                let mut val = 0u32;
                for _ in 0..4 {
                    let b = read_u8(c);
                    val = (val << 7) | (b & 0x7F) as u32;
                    if (b & 0x80) == 0 {
                        break;
                    }
                }
                val
            };

            while cursor < end_limit {
                let delta = read_var_len(&mut cursor);
                abs_ticks += delta;

                let mut status = read_u8(&mut cursor);
                if status < 0x80 {
                    status = running_status;
                    cursor -= 1;
                } else {
                    running_status = status;
                }

                if status == 0xFF {
                    let meta_type = read_u8(&mut cursor);
                    let len = read_var_len(&mut cursor) as usize;
                    let mut meta_data = Vec::new();
                    for _ in 0..len {
                        meta_data.push(read_u8(&mut cursor));
                    }

                    if meta_type == 0x2F {
                        break;
                    } // End of track

                    all_events.push(RawMidiEvent {
                        abs_ticks,
                        status: 0xFF,
                        data_bytes: std::iter::once(meta_type)
                            .chain(meta_data.into_iter())
                            .collect(),
                    });
                } else if status == 0xF0 || status == 0xF7 {
                    let len = read_var_len(&mut cursor) as usize;
                    cursor += len;
                } else {
                    let event_type = status & 0xF0;
                    let mut ev_data = Vec::new();
                    match event_type {
                        0xC0 | 0xD0 => {
                            ev_data.push(read_u8(&mut cursor));
                        }
                        _ => {
                            ev_data.push(read_u8(&mut cursor));
                            ev_data.push(read_u8(&mut cursor));
                        }
                    }
                    all_events.push(RawMidiEvent {
                        abs_ticks,
                        status,
                        data_bytes: ev_data,
                    });
                }
            }
        }

        // 3. Sort events by absolute ticks
        all_events.sort_by_key(|e| e.abs_ticks);

        // 4. Time conversion & emit MusEvents
        let mut out_events = Vec::new();
        let mut current_tempo_us = 500_000.0;
        let mut last_abs_ticks = 0;
        let mut absolute_seconds = 0.0;
        let mut last_140hz_tick = 0;

        for ev in all_events {
            let tick_diff = ev.abs_ticks - last_abs_ticks;
            absolute_seconds += (tick_diff as f64) * (current_tempo_us / ppqn) / 1_000_000.0;
            last_abs_ticks = ev.abs_ticks;

            if ev.status == 0xFF {
                // Meta event
                if ev.data_bytes.len() >= 4 && ev.data_bytes[0] == 0x51 {
                    // Set Tempo
                    let t1 = ev.data_bytes[1] as u32;
                    let t2 = ev.data_bytes[2] as u32;
                    let t3 = ev.data_bytes[3] as u32;
                    current_tempo_us = ((t1 << 16) | (t2 << 8) | t3) as f64;
                }
                continue;
            }

            let event_type = ev.status & 0xF0;
            let chan = ev.status & 0x0F;
            if chan == 9 {
                continue;
            } // OPL3 bridge currently asserts standard channels; ignore drums for now.

            let mut mapped_event = None;
            match event_type {
                0x80 => {
                    mapped_event = Some(MusEvent::NoteOff {
                        chan,
                        note: ev.data_bytes[0],
                    });
                }
                0x90 => {
                    let note = ev.data_bytes[0];
                    let vel = ev.data_bytes[1];
                    mapped_event = Some(if vel == 0 {
                        MusEvent::NoteOff { chan, note }
                    } else {
                        MusEvent::NoteOn {
                            chan,
                            note,
                            vol: Some(vel),
                        }
                    });
                }
                0xB0 => {
                    let ctrl = ev.data_bytes[0];
                    let val = ev.data_bytes[1];
                    let mapped_ctrl = if ctrl == 7 { 3 } else { ctrl };
                    mapped_event = Some(MusEvent::Controller {
                        chan,
                        ctrl: mapped_ctrl,
                        val,
                    });
                }
                0xC0 => {
                    mapped_event = Some(MusEvent::Controller {
                        chan,
                        ctrl: 0,
                        val: ev.data_bytes[0],
                    });
                }
                0xE0 => {
                    let msb = ev.data_bytes[1] as u16;
                    let val = (msb & 0x7F) << 1;
                    mapped_event = Some(MusEvent::PitchBend {
                        chan,
                        val: val as u8,
                    });
                }
                _ => {}
            }

            if let Some(mus_ev) = mapped_event {
                let target_140hz = (absolute_seconds * 140.0).round() as u32;
                let delta_140hz = target_140hz.saturating_sub(last_140hz_tick);
                last_140hz_tick = target_140hz;
                out_events.push((delta_140hz, mus_ev));
            }
        }

        out_events.push((0, MusEvent::End));

        Self {
            events: out_events,
            current_idx: 0,
            finished: false,
        }
    }

    pub fn restart(&mut self) {
        self.current_idx = 0;
        self.finished = false;
    }

    pub fn next_event(&mut self) -> Option<(u32, MusEvent)> {
        if self.current_idx >= self.events.len() {
            self.finished = true;
            return None;
        }
        let ev = self.events[self.current_idx].clone();
        self.current_idx += 1;
        Some(ev)
    }
}
