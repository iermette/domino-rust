//! Domino — logique de jeu pure (aucune I/O), testable et déterministe.

pub mod board;
pub mod bot;
pub mod game;
pub mod tile;

pub use board::{Board, Side};
pub use game::{
    BotLevel, Game, GameError, Phase, Player, PlayerKind, RoundKind, RoundResult,
};
pub use tile::{all_tiles, Pip, Tile};
