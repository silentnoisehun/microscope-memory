// modules/commands.rs — Command Definitions & Mapping
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandType {
    // Microscope Commands
    Recall,
    Remember,
    Find,
    Look,
    // Bicska Commands
    Mutate,
    Crispr,
    Read,
    Pipeline,
    // Consciousness Commands
    Hebbian,
    Mirror,
    Resonance,
    Archetype,
    Patterns,
    Dream,
    // System Commands
    Status,
    Doctor,
    // Unknown
    Unknown,
}

impl From<u8> for CommandType {
    fn from(val: u8) -> Self {
        match val {
            1 => CommandType::Recall,
            2 => CommandType::Remember,
            3 => CommandType::Find,
            4 => CommandType::Look,
            5 => CommandType::Mutate,
            6 => CommandType::Crispr,
            7 => CommandType::Read,
            8 => CommandType::Pipeline,
            9 => CommandType::Hebbian,
            10 => CommandType::Mirror,
            11 => CommandType::Resonance,
            12 => CommandType::Archetype,
            13 => CommandType::Patterns,
            14 => CommandType::Dream,
            15 => CommandType::Status,
            16 => CommandType::Doctor,
            _ => CommandType::Unknown,
        }
    }
}

impl From<CommandType> for u8 {
    fn from(cmd: CommandType) -> Self {
        match cmd {
            CommandType::Recall => 1,
            CommandType::Remember => 2,
            CommandType::Find => 3,
            CommandType::Look => 4,
            CommandType::Mutate => 5,
            CommandType::Crispr => 6,
            CommandType::Read => 7,
            CommandType::Pipeline => 8,
            CommandType::Hebbian => 9,
            CommandType::Mirror => 10,
            CommandType::Resonance => 11,
            CommandType::Archetype => 12,
            CommandType::Patterns => 13,
            CommandType::Dream => 14,
            CommandType::Status => 15,
            CommandType::Doctor => 16,
            CommandType::Unknown => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandIntent {
    pub cmd: CommandType,
    pub args: String,
}

pub fn parse_intent(raw_cmd: u8, raw_args: &[u8]) -> CommandIntent {
    CommandIntent {
        cmd: CommandType::from(raw_cmd),
        args: String::from_utf8_lossy(raw_args).trim().to_string(),
    }
}
