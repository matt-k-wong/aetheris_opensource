//! Savegame checksum validation for authenticity
//! Vanilla Doom used checksums to prevent savegame tampering

/// Calculate a stable checksum for savegame data using FNV-1a
/// FNV-1a is a simple, fast, and stable hash algorithm that produces
/// consistent results across Rust versions and platforms.
pub fn calculate_checksum(data: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Savegame header with checksum for validation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavegameHeader {
    pub version: u32,
    pub checksum: u64,
    pub timestamp: u64,
    pub map_name: String,
    pub player_health: i32,
    pub player_armor: i32,
}

impl SavegameHeader {
    pub const CURRENT_VERSION: u32 = 1;

    /// Create a new savegame header with calculated checksum
    pub fn new(data: &str, map_name: &str, player_health: i32, player_armor: i32) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            checksum: calculate_checksum(data),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            map_name: map_name.to_string(),
            player_health,
            player_armor,
        }
    }

    /// Validate that the data matches the stored checksum
    pub fn validate(&self, data: &str) -> bool {
        self.checksum == calculate_checksum(data)
    }

    /// Check if the savegame version is compatible
    pub fn is_compatible(&self) -> bool {
        self.version == Self::CURRENT_VERSION
    }
}

/// Wrapper for savegame data with header
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavegameWithChecksum {
    pub header: SavegameHeader,
    pub data: String,
}

impl SavegameWithChecksum {
    /// Create a new savegame with checksum from world data
    pub fn new(world_data: &str, map_name: &str, player_health: i32, player_armor: i32) -> Self {
        let header = SavegameHeader::new(world_data, map_name, player_health, player_armor);
        Self {
            header,
            data: world_data.to_string(),
        }
    }

    /// Validate the savegame checksum and version
    pub fn validate(&self) -> Result<(), SavegameError> {
        if !self.header.is_compatible() {
            return Err(SavegameError::VersionMismatch {
                expected: SavegameHeader::CURRENT_VERSION,
                found: self.header.version,
            });
        }

        if !self.header.validate(&self.data) {
            return Err(SavegameError::ChecksumMismatch);
        }

        Ok(())
    }

    /// Extract the world data if validation passes
    pub fn extract_data(&self) -> Result<String, SavegameError> {
        self.validate()?;
        Ok(self.data.clone())
    }
}

/// Errors that can occur when loading savegames
#[derive(Debug, Clone)]
pub enum SavegameError {
    ChecksumMismatch,
    VersionMismatch { expected: u32, found: u32 },
    DeserializeError(String),
}

impl std::fmt::Display for SavegameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SavegameError::ChecksumMismatch => {
                write!(
                    f,
                    "Savegame checksum mismatch - data may be corrupted or tampered with"
                )
            }
            SavegameError::VersionMismatch { expected, found } => {
                write!(
                    f,
                    "Savegame version mismatch: expected {}, found {}",
                    expected, found
                )
            }
            SavegameError::DeserializeError(e) => {
                write!(f, "Failed to deserialize savegame: {}", e)
            }
        }
    }
}

impl std::error::Error for SavegameError {}

/// Demo (input recording) checksum for validation
/// Demos recorded the player's inputs and had checksums to ensure integrity
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DemoHeader {
    pub version: u32,
    pub checksum: u64,
    pub map_name: String,
    pub skill_level: u8,
    pub player_class: u8, // For Heretic/Hexen compatibility
    pub total_tics: u32,
}

impl DemoHeader {
    pub const CURRENT_VERSION: u32 = 1;
    pub const DEMO_MARKER: &[u8] = b"DOOMDEMO";

    /// Calculate checksum from demo input data using FNV-1a
    pub fn calculate_checksum(inputs: &[DemoInput]) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET;
        for input in inputs {
            // Hash each field's bytes
            for byte in &input.game_tic.to_le_bytes() {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(FNV_PRIME);
            }
            hash ^= input.forward_move as u8 as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
            hash ^= input.side_move as u8 as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
            for byte in &input.turn_angle.to_le_bytes() {
                hash ^= *byte as u64;
                hash = hash.wrapping_mul(FNV_PRIME);
            }
            hash ^= input.buttons as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}

/// Single demo input frame (35Hz tick)
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Hash)]
pub struct DemoInput {
    pub game_tic: u32,
    pub forward_move: i8, // Forward/backward movement
    pub side_move: i8,    // Strafe movement
    pub turn_angle: i16,  // Turning angle
    pub buttons: u8,      // Fire/use buttons
}

/// Complete demo recording with inputs and header
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DemoRecording {
    pub header: DemoHeader,
    pub inputs: Vec<DemoInput>,
}

impl DemoRecording {
    /// Validate the demo checksum
    pub fn validate(&self) -> Result<(), DemoError> {
        if self.header.version != DemoHeader::CURRENT_VERSION {
            return Err(DemoError::VersionMismatch {
                expected: DemoHeader::CURRENT_VERSION,
                found: self.header.version,
            });
        }

        let calculated = DemoHeader::calculate_checksum(&self.inputs);
        if calculated != self.header.checksum {
            return Err(DemoError::ChecksumMismatch);
        }

        // Verify input count matches recorded total
        if self.inputs.len() as u32 != self.header.total_tics {
            return Err(DemoError::LengthMismatch {
                expected: self.header.total_tics,
                found: self.inputs.len() as u32,
            });
        }

        Ok(())
    }
}

/// Errors that can occur with demo files
#[derive(Debug, Clone)]
pub enum DemoError {
    ChecksumMismatch,
    VersionMismatch { expected: u32, found: u32 },
    LengthMismatch { expected: u32, found: u32 },
    InvalidFormat,
}

impl std::fmt::Display for DemoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DemoError::ChecksumMismatch => write!(f, "Demo checksum mismatch"),
            DemoError::VersionMismatch { expected, found } => {
                write!(
                    f,
                    "Demo version mismatch: expected {}, found {}",
                    expected, found
                )
            }
            DemoError::LengthMismatch { expected, found } => {
                write!(
                    f,
                    "Demo length mismatch: expected {} tics, found {}",
                    expected, found
                )
            }
            DemoError::InvalidFormat => write!(f, "Invalid demo file format"),
        }
    }
}

impl std::error::Error for DemoError {}

/// Helper functions for savegame/demo I/O
pub mod io {
    use super::*;
    use std::io::Write;

    /// Save a world state to file with checksum
    pub fn save_with_checksum(
        path: &str,
        world: &crate::simulation::WorldState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Serialize world state (without textures since they're skipped)
        let json = serde_json::to_string(world)?;

        // Create wrapped savegame with checksum
        let savegame = SavegameWithChecksum::new(
            &json,
            "E1M1", // TODO: get actual map name from world
            world.player.health as i32,
            world.player.armor as i32,
        );

        // Serialize to JSON and write
        let save_json = serde_json::to_string(&savegame)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(save_json.as_bytes())?;

        log::info!("Saved game to {} with checksum validation", path);
        Ok(())
    }

    /// Load a world state from file with checksum validation
    pub fn load_with_checksum(path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let savegame: SavegameWithChecksum = serde_json::from_str(&content)?;

        // Validate checksum
        savegame.validate()?;

        log::info!("Loaded game from {} - checksum validated", path);
        Ok(savegame.data)
    }

    /// Quick save with checksum
    pub fn quick_save(
        world: &crate::simulation::WorldState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        save_with_checksum("savegame.json", world)
    }

    /// Quick load with checksum validation
    pub fn quick_load() -> Result<String, Box<dyn std::error::Error>> {
        load_with_checksum("savegame.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_calculation() {
        let data1 = "test data";
        let data2 = "test data";
        let data3 = "different data";

        let checksum1 = calculate_checksum(data1);
        let checksum2 = calculate_checksum(data2);
        let checksum3 = calculate_checksum(data3);

        assert_eq!(checksum1, checksum2); // Same data = same checksum
        assert_ne!(checksum1, checksum3); // Different data = different checksum
    }

    #[test]
    fn test_savegame_validation() {
        let data = "{\"test\": \"data\"}";
        let savegame = SavegameWithChecksum::new(data, "E1M1", 100, 50);

        // Should validate successfully
        assert!(savegame.validate().is_ok());
        assert_eq!(savegame.extract_data().unwrap(), data);
    }

    #[test]
    fn test_savegame_tampering_detection() {
        let data = "{\"test\": \"data\"}";
        let mut savegame = SavegameWithChecksum::new(data, "E1M1", 100, 50);

        // Tamper with the data
        savegame.data = "{\"test\": \"tampered\"}".to_string();

        // Should fail validation
        assert!(matches!(
            savegame.validate(),
            Err(SavegameError::ChecksumMismatch)
        ));
    }

    #[test]
    fn test_demo_checksum() {
        let inputs = vec![
            DemoInput {
                game_tic: 0,
                forward_move: 10,
                side_move: 0,
                turn_angle: 0,
                buttons: 0,
            },
            DemoInput {
                game_tic: 1,
                forward_move: 10,
                side_move: 0,
                turn_angle: 5,
                buttons: 1,
            },
        ];

        let checksum1 = DemoHeader::calculate_checksum(&inputs);
        let checksum2 = DemoHeader::calculate_checksum(&inputs);

        assert_eq!(checksum1, checksum2); // Deterministic

        // Different inputs = different checksum
        let different_inputs = vec![DemoInput {
            game_tic: 0,
            forward_move: 20,
            side_move: 0,
            turn_angle: 0,
            buttons: 0,
        }];
        let checksum3 = DemoHeader::calculate_checksum(&different_inputs);
        assert_ne!(checksum1, checksum3);
    }
}
