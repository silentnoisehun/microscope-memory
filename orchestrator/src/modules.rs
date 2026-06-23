// modules/modules.rs — Module Routing & Interface
use crate::commands::CommandType;

#[derive(Debug, Clone, Copy)]
pub enum ModuleTarget {
    Microscope,
    Bicska,
    Rongyasz,
    Alan,
    System,
}

pub fn route_command(cmd: &CommandType) -> ModuleTarget {
    match cmd {
        CommandType::Recall | CommandType::Remember | CommandType::Find | CommandType::Look => ModuleTarget::Microscope,
        CommandType::Mutate | CommandType::Crispr | CommandType::Read | CommandType::Pipeline => ModuleTarget::Bicska,
        CommandType::Hebbian | CommandType::Mirror | CommandType::Resonance | CommandType::Archetype | CommandType::Patterns | CommandType::Dream => ModuleTarget::Rongyasz,
        CommandType::Status | CommandType::Doctor => ModuleTarget::Alan,
        CommandType::Unknown => ModuleTarget::System,
    }
}
