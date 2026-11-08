use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::board::{Board, Side};
use crate::tile::{Tile, all_tiles};

/// Joueurs minimum à la table.
pub const MIN_PLAYERS: usize = 2;
/// Joueurs maximum à la table.
pub const MAX_PLAYERS: usize = 4;
/// Nombre de dominos remis à chaque joueur.
pub const HAND_SIZE: usize = 7;

/// Difficulté des bots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotLevel {
    /// Joue un domino jouable au hasard.
    Easy,
    /// Joue le domino jouable ayant la plus faible somme de points.
    Medium,
    /// Analyse les faces ouvertes et les copies restantes.
    Hard,
}

impl BotLevel {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "easy" | "facile" => Some(Self::Easy),
            "medium" | "moyen" => Some(Self::Medium),
            "hard" | "difficile" => Some(Self::Hard),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "facile",
            Self::Medium => "moyen",
            Self::Hard => "difficile",
        }
    }
}

/// Nature d'un siège à la table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerKind {
    Human,
    Bot(BotLevel),
}

/// Joueur (humain ou bot) assis à la table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub id: u8,
    pub name: String,
    pub kind: PlayerKind,
    pub hand: Vec<Tile>,
    /// Score cumulé sur la partie.
    pub score: u16,
    /// Vrai si un humain est réellement connecté sur ce siège.
    pub connected: bool,
}

/// Phase du jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Joueurs assis, aucune manche lancée.
    Waiting,
    /// Une manche est en cours.
    Playing,
    /// Manche terminée, en attente de la suivante.
    RoundOver,
    /// Partie terminée.
    MatchOver,
}

/// Comment la manche s'est terminée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundKind {
    /// Un joueur s'est délesté de sa main.
    Open,
    /// Plateau bloqué : plus personne ne peut jouer.
    Closed,
}

/// Résultat d'une manche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundResult {
    pub kind: RoundKind,
    pub winner: u8,
    /// Points restants par joueur `(id, pips)`.
    pub points: Vec<(u8, u16)>,
    /// Points attribués au gagnant.
    pub awarded: u16,
    /// Vrai si le gagnant a été départagé par « la main ».
    pub tie_break: bool,
}

/// Erreurs renvoyées par les actions de jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameError {
    NotYourTurn,
    TileNotInHand,
    CannotPlay,
    OpeningTileRequired,
    BoneyardEmpty,
    CanStillPlay,
    GameNotStarted,
    TableFull,
    PhaseMismatch,
    InvalidPlayer,
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::NotYourTurn => "Ce n'est pas votre tour.",
            Self::TileNotInHand => "Ce domino n'est pas dans votre main.",
            Self::CannotPlay => "Ce domino ne peut pas être posé sur cette extrémité.",
            Self::OpeningTileRequired => "En 1ʳᵉ manche, vous devez jouer le domino d'ouverture.",
            Self::BoneyardEmpty => "La pioche est vide.",
            Self::CanStillPlay => "Vous avez encore des dominos jouables.",
            Self::GameNotStarted => "La partie n'a pas encore commencé.",
            Self::TableFull => "La table est pleine.",
            Self::PhaseMismatch => "Action impossible dans la phase actuelle.",
            Self::InvalidPlayer => "Joueur inconnu.",
        };
        f.write_str(s)
    }
}

/// État complet d'une partie (le serveur est l'autorité).
#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub players: Vec<Player>,
    pub boneyard: Vec<Tile>,
    pub board: Board,
    pub phase: Phase,
    /// id du joueur dont c'est le tour.
    pub turn: u8,
    /// Manche en cours (1-based).
    pub round: u32,
    /// Détenteur de « la main » (ouvreur de la manche).
    pub opener: u8,
    /// Cible de points de la partie.
    pub target: u16,
    /// Nombre maximum de sièges à la table (2 à 4).
    pub max_players: usize,
    /// Vrai pendant la première main (l'ouvreur doit jouer le domino d'ouverture).
    pub first_hand: bool,
    /// Domino d'ouverture à jouer en 1ʳᵉ manche.
    pub opening_tile: Option<Tile>,
    pub round_result: Option<RoundResult>,
    pub match_winner: Option<u8>,
    /// Journal récent des événements.
    pub log: Vec<String>,
    #[serde(skip)]
    rng: StdRng,
}

impl Game {
    /// Crée une partie vide (aucun joueur assis).
    pub fn new(target: u16, seed: u64) -> Self {
        Self::with_players(Vec::new(), target, seed)
    }

    /// Crée une partie avec les joueurs donnés.
    pub fn with_players(players: Vec<Player>, target: u16, seed: u64) -> Self {
        Self {
            players,
            boneyard: Vec::new(),
            board: Board::new(),
            phase: Phase::Waiting,
            turn: 0,
            round: 0,
            opener: 0,
            target,
            max_players: MAX_PLAYERS,
            first_hand: true,
            opening_tile: None,
            round_result: None,
            match_winner: None,
            log: vec!["🁫 Table prête — lancez la partie.".into()],
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn push_log(&mut self, text: String) {
        self.log.push(text);
        if self.log.len() > 100 {
            self.log.drain(..self.log.len() - 100);
        }
    }

    pub fn player(&self, id: u8) -> Option<&Player> {
        self.players.get(id as usize)
    }

    /// Joueur suivant (sens antihoraire).
    pub fn next_id(&self, id: u8) -> u8 {
        let n = self.players.len();
        if n == 0 {
            return id;
        }
        ((id as usize + 1) % n) as u8
    }

    /// Assied un joueur à la table.
    pub fn add_player(&mut self, name: String, kind: PlayerKind) -> Result<u8, GameError> {
        if self.phase == Phase::Playing || self.phase == Phase::RoundOver {
            return Err(GameError::PhaseMismatch);
        }
        if self.players.len() >= self.max_players {
            return Err(GameError::TableFull);
        }
        let id = self.players.len() as u8;
        self.players.push(Player {
            id,
            name: name.clone(),
            kind,
            hand: Vec::new(),
            score: 0,
            connected: true,
        });
        self.push_log(format!("👋 {name} s'assoit à la table."));
        Ok(id)
    }

    /// Définit le nombre maximum de sièges (2 à 4).
    pub fn set_max_players(&mut self, n: usize) -> Result<(), GameError> {
        if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
            return Err(GameError::InvalidPlayer);
        }
        if n < self.players.len() {
            return Err(GameError::InvalidPlayer);
        }
        self.max_players = n;
        Ok(())
    }

    /// Retire un joueur (en attente ou fin de partie) et ré-indexe les ids.
    pub fn remove_player(&mut self, pos: usize) -> Option<Player> {
        if self.phase != Phase::Waiting && self.phase != Phase::MatchOver {
            return None;
        }
        if pos >= self.players.len() {
            return None;
        }
        let removed = self.players.remove(pos);
        let remap: HashMap<u8, u8> = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id, i as u8))
            .collect();
        for (i, p) in self.players.iter_mut().enumerate() {
            p.id = i as u8;
        }
        self.turn = remap.get(&self.turn).copied().unwrap_or(0);
        self.opener = remap.get(&self.opener).copied().unwrap_or(0);
        if let Some(rr) = self.round_result.as_mut() {
            if let Some(&n) = remap.get(&rr.winner) {
                rr.winner = n;
            }
            rr.points = rr
                .points
                .iter()
                .filter_map(|(id, pts)| remap.get(id).map(|n| (*n, *pts)))
                .collect();
        }
        self.match_winner = self.match_winner.and_then(|w| remap.get(&w).copied());
        Some(removed)
    }

    /// Lance la partie (distribue la 1ʳᵉ manche, remise à zéro des scores).
    pub fn start(&mut self) -> Result<(), GameError> {
        if self.phase != Phase::Waiting && self.phase != Phase::MatchOver {
            return Err(GameError::PhaseMismatch);
        }
        if self.players.len() < MIN_PLAYERS {
            return Err(GameError::GameNotStarted);
        }
        for p in &mut self.players {
            p.score = 0;
        }
        self.match_winner = None;
        self.round_result = None;
        self.board.reset();
        self.round = 1;
        self.first_hand = true;
        self.deal();
        self.phase = Phase::Playing;
        self.push_log(format!(
            "🚀 La partie commence — {} ouvre la 1ʳᵉ manche{}",
            self.player(self.opener).map(|p| p.name.clone()).unwrap_or_default(),
            self.opening_tile.map(|t| format!(" avec {t}")).unwrap_or_default()
        ));
        Ok(())
    }

    /// Mélange et distribue 7 dominos à chaque joueur.
    pub fn deal(&mut self) {
        self.boneyard = all_tiles();
        self.boneyard.shuffle(&mut self.rng);
        for p in &mut self.players {
            p.hand.clear();
        }
        for p in &mut self.players {
            let take = HAND_SIZE.min(self.boneyard.len());
            for _ in 0..take {
                if let Some(t) = self.boneyard.pop() {
                    p.hand.push(t);
                }
            }
        }
        if self.first_hand {
            // Ouvreur = détenteur du double le plus élevé,
            // sinon du domino le plus élevé (somme de points).
            let mut best: Option<(u32, u8, Tile)> = None;
            for p in &self.players {
                for &t in &p.hand {
                    let rank = if t.is_double() {
                        1000 + t.points() as u32
                    } else {
                        t.points() as u32
                    };
                    if best.map_or(true, |(b, _, _)| rank > b) {
                        best = Some((rank, p.id, t));
                    }
                }
            }
            if let Some((_, id, t)) = best {
                self.opener = id;
                self.opening_tile = Some(t);
            }
        }
        self.turn = self.opener;
    }

    /// Passe à la manche suivante : l'ouvreur tourne au joueur suivant.
    pub fn advance_round(&mut self) -> Result<(), GameError> {
        if self.phase != Phase::RoundOver {
            return Err(GameError::PhaseMismatch);
        }
        if self.players.is_empty() {
            return Err(GameError::GameNotStarted);
        }
        self.opener = self.next_id(self.opener);
        self.round += 1;
        self.first_hand = false;
        self.opening_tile = None;
        self.round_result = None;
        self.board.reset();
        self.deal();
        self.push_log(format!(
            "🔄 Manche {} — {} ouvre (domino au choix).",
            self.round,
            self.player(self.opener).map(|p| p.name.clone()).unwrap_or_default()
        ));
        self.phase = Phase::Playing;
        Ok(())
    }

    /// Coups légaux du joueur (restriction 1ʳᵉ main incluse).
    pub fn legal_moves(&self, id: u8) -> Vec<(Tile, Side)> {
        let Some(p) = self.player(id) else {
            return Vec::new();
        };
        let first_move = self.first_hand && self.board.is_empty() && id == self.opener;
        let mut out = Vec::new();
        for &t in &p.hand {
            if first_move && self.opening_tile != Some(t) {
                continue;
            }
            for side in [Side::Left, Side::Right] {
                if self.board.can_place(t, side) {
                    out.push((t, side));
                }
            }
        }
        out
    }

    /// Le joueur a-t-il au moins un domino posable ?
    pub fn can_play_any(&self, id: u8) -> bool {
        !self.legal_moves(id).is_empty()
    }

    /// Pose un domino sur le plateau.
    pub fn play(&mut self, id: u8, tile: Tile, side: Side) -> Result<(), GameError> {
        if self.phase != Phase::Playing {
            return Err(GameError::PhaseMismatch);
        }
        let Some(p) = self.players.get(id as usize) else {
            return Err(GameError::InvalidPlayer);
        };
        if self.turn != id {
            return Err(GameError::NotYourTurn);
        }
        if self.first_hand
            && self.board.is_empty()
            && id == self.opener
            && self.opening_tile != Some(tile)
        {
            return Err(GameError::OpeningTileRequired);
        }
        if !p.hand.iter().any(|t| *t == tile) {
            return Err(GameError::TileNotInHand);
        }
        if !self.board.can_place(tile, side) {
            return Err(GameError::CannotPlay);
        }
        self.board.place(tile, side);
        if let Some(p) = self.players.get_mut(id as usize) {
            if let Some(pos) = p.hand.iter().position(|t| *t == tile) {
                p.hand.remove(pos);
            }
        }
        self.push_log(format!(
            "{} pose {} ({})",
            self.players[id as usize].name,
            tile,
            side.as_str()
        ));
        self.turn = self.next_id(id);
        self.check_round_end();
        Ok(())
    }

    /// Pioche un domino (uniquement si on ne peut pas jouer).
    pub fn draw(&mut self, id: u8) -> Result<Tile, GameError> {
        if self.phase != Phase::Playing {
            return Err(GameError::PhaseMismatch);
        }
        if self.player(id).is_none() {
            return Err(GameError::InvalidPlayer);
        }
        if self.turn != id {
            return Err(GameError::NotYourTurn);
        }
        if self.can_play_any(id) {
            return Err(GameError::CanStillPlay);
        }
        let t = *self.boneyard.first().ok_or(GameError::BoneyardEmpty)?;
        self.boneyard.remove(0);
        if let Some(p) = self.players.get_mut(id as usize) {
            p.hand.push(t);
        }
        self.push_log(format!("{} pioche {}", self.players[id as usize].name, t));
        Ok(t)
    }

    /// Passe son tour (uniquement pioche vide ET aucun domino posable).
    pub fn pass(&mut self, id: u8) -> Result<(), GameError> {
        if self.phase != Phase::Playing {
            return Err(GameError::PhaseMismatch);
        }
        if self.player(id).is_none() {
            return Err(GameError::InvalidPlayer);
        }
        if self.turn != id {
            return Err(GameError::NotYourTurn);
        }
        if !self.boneyard.is_empty() {
            return Err(GameError::BoneyardEmpty);
        }
        if self.can_play_any(id) {
            return Err(GameError::CanStillPlay);
        }
        self.push_log(format!("{} passe", self.players[id as usize].name));
        self.turn = self.next_id(id);
        self.check_round_end();
        Ok(())
    }

    /// Vérifie la fin de manche (appelée après chaque action).
    pub fn check_round_end(&mut self) {
        if self.phase != Phase::Playing || self.players.is_empty() {
            return;
        }

        // 1) Manche ouverte : un joueur s'est délesté de sa main.
        if let Some(winner) = self.players.iter().find(|p| p.hand.is_empty()).map(|p| p.id) {
            let points = self.points_by_player();
            let awarded: u16 = points
                .iter()
                .filter(|(id, _)| *id != winner)
                .map(|(_, v)| *v)
                .sum();
            self.players[winner as usize].score =
                self.players[winner as usize].score.saturating_add(awarded);
            let name = self.players[winner as usize].name.clone();
            self.push_log(format!("🎉 {name} se déleste de sa main : +{awarded} pts !"));
            self.finish_round(RoundResult {
                kind: RoundKind::Open,
                winner,
                points,
                awarded,
                tie_break: false,
            });
            return;
        }

        // 2) Manche fermée : pioche vide et plus personne ne peut jouer.
        if self.boneyard.is_empty() && !self.players.iter().any(|p| self.can_play_any(p.id)) {
            let points = self.points_by_player();
            let min = points.iter().map(|(_, v)| *v).min().unwrap_or(0);
            let tied: Vec<u8> = points
                .iter()
                .filter(|(_, v)| *v == min)
                .map(|(id, _)| *id)
                .collect();
            let (winner, tie_break) = if tied.len() == 1 {
                (tied[0], false)
            } else {
                // « La main » gagne : l'ouvreur, sinon le plus proche dans le sens de jeu.
                let n = self.players.len() as i32;
                let mut best: (u8, i32) = (tied[0], i32::MAX);
                for i in tied {
                    let dist = (i as i32 - self.opener as i32).rem_euclid(n);
                    if dist < best.1 {
                        best = (i, dist);
                    }
                }
                (best.0, true)
            };
            let awarded: u16 = points
                .iter()
                .filter(|(id, _)| *id != winner)
                .map(|(_, v)| *v)
                .sum();
            self.players[winner as usize].score =
                self.players[winner as usize].score.saturating_add(awarded);
            let name = self.players[winner as usize].name.clone();
            self.push_log(format!(
                "🔒 Manche bloquée ! {name} garde le moins de points : +{awarded} pts{}",
                if tie_break { " (départagé par « la main »)" } else { "" }
            ));
            self.finish_round(RoundResult {
                kind: RoundKind::Closed,
                winner,
                points,
                awarded,
                tie_break,
            });
        }
    }

    fn points_by_player(&self) -> Vec<(u8, u16)> {
        self.players
            .iter()
            .map(|p| (p.id, p.hand.iter().map(|t| t.points()).sum::<u16>()))
            .collect()
    }

    fn finish_round(&mut self, result: RoundResult) {
        let winner = result.winner;
        let reached = self
            .players
            .get(winner as usize)
            .map(|p| p.score >= self.target)
            .unwrap_or(false);
        if reached {
            self.phase = Phase::MatchOver;
            self.match_winner = Some(winner);
            self.round_result = Some(result);
            let name = self.players[winner as usize].name.clone();
            self.push_log(format!("🏆 {name} remporte la partie !"));
        } else {
            self.phase = Phase::RoundOver;
            self.round_result = Some(result);
        }
    }
}
