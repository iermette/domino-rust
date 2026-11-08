use serde::{Deserialize, Serialize};

use crate::tile::{Pip, Tile};

/// Extrémité du plateau où l'on pose un domino.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" | "gauche" => Some(Side::Left),
            "right" | "droite" => Some(Side::Right),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// La chaîne de dominos posés sur la table.
///
/// Chaque domino est stocké dans son **orientation d'affichage**
/// `(face gauche, face droite)` ; un double a ses deux faces égales
/// (affiché obliquement, comme sur une vraie table).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Board {
    /// Dominos dans l'ordre de la chaîne, du plus à gauche au plus à droite.
    pub tiles: Vec<(Pip, Pip)>,
}

impl Board {
    pub fn new() -> Self {
        Self { tiles: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Face ouverte à gauche (None si plateau vide).
    pub fn left_end(&self) -> Option<Pip> {
        self.tiles.first().map(|t| t.0)
    }

    /// Face ouverte à droite (None si plateau vide).
    pub fn right_end(&self) -> Option<Pip> {
        self.tiles.last().map(|t| t.1)
    }

    /// Peut-on poser `tile` sur `side` ?
    pub fn can_place(&self, tile: Tile, side: Side) -> bool {
        match (self.left_end(), self.right_end()) {
            (None, None) => true,
            (Some(l), _) if side == Side::Left => tile.matches(l),
            (_, Some(r)) if side == Side::Right => tile.matches(r),
            _ => false,
        }
    }

    /// Pose `tile` sur `side` (à appeler seulement si `can_place`).
    pub fn place(&mut self, tile: Tile, side: Side) {
        match (self.left_end(), self.right_end()) {
            (None, None) => self.tiles.push((tile.a, tile.b)),
            (Some(l), _) if side == Side::Left => {
                let other = tile.other_face(l).unwrap_or(tile.a);
                self.tiles.insert(0, (other, l));
            }
            (_, Some(r)) if side == Side::Right => {
                let other = tile.other_face(r).unwrap_or(tile.a);
                self.tiles.push((r, other));
            }
            _ => {}
        }
    }

    /// Face « ouverte » résultant de la pose de `tile` sur `side`.
    pub fn resulting_open_face(&self, tile: Tile, side: Side) -> Option<Pip> {
        if self.tiles.is_empty() {
            Some(if side == Side::Left { tile.a } else { tile.b })
        } else if side == Side::Left {
            self.left_end().and_then(|l| tile.other_face(l))
        } else {
            self.right_end().and_then(|r| tile.other_face(r))
        }
    }

    /// Combien de fois la face `pip` apparaît sur le plateau.
    pub fn count_pip(&self, pip: Pip) -> u8 {
        let mut n = 0u8;
        for (l, r) in &self.tiles {
            if *l == pip {
                n += 1;
            }
            if *r == pip {
                n += 1;
            }
        }
        n
    }

    pub fn reset(&mut self) {
        self.tiles.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premier_dominos_ouvre_les_deux_extremites() {
        let mut b = Board::new();
        assert!(b.can_place(Tile::new(3, 5).unwrap(), Side::Left));
        b.place(Tile::new(3, 5).unwrap(), Side::Left);
        assert_eq!(b.left_end(), Some(Pip(3)));
        assert_eq!(b.right_end(), Some(Pip(5)));
    }

    #[test]
    fn pose_gauche_insere_en_tete_de_chaine() {
        let mut b = Board::new();
        b.place(Tile::new(5, 6).unwrap(), Side::Right);
        // Pose 3-5 à gauche : la face 5 touche la face 5 existante.
        assert!(b.can_place(Tile::new(3, 5).unwrap(), Side::Left));
        b.place(Tile::new(3, 5).unwrap(), Side::Left);
        assert_eq!(b.tiles, vec![(Pip(3), Pip(5)), (Pip(5), Pip(6))]);
        assert_eq!(b.left_end(), Some(Pip(3)));
        assert_eq!(b.right_end(), Some(Pip(6)));
    }

    #[test]
    fn pose_droite_pousse_en_fin_de_chaine() {
        let mut b = Board::new();
        b.place(Tile::new(3, 5).unwrap(), Side::Right);
        assert!(b.can_place(Tile::new(5, 2).unwrap(), Side::Right));
        b.place(Tile::new(5, 2).unwrap(), Side::Right);
        assert_eq!(b.tiles, vec![(Pip(3), Pip(5)), (Pip(5), Pip(2))]);
        assert_eq!(b.right_end(), Some(Pip(2)));
    }

    #[test]
    fn pose_invalides_rejetees() {
        let mut b = Board::new();
        b.place(Tile::new(3, 5).unwrap(), Side::Right);
        assert!(!b.can_place(Tile::new(1, 2).unwrap(), Side::Left));
        assert!(!b.can_place(Tile::new(1, 2).unwrap(), Side::Right));
    }

    #[test]
    fn double_se_pose_aussi() {
        let mut b = Board::new();
        b.place(Tile::new(4, 4).unwrap(), Side::Right);
        assert!(b.can_place(Tile::new(4, 1).unwrap(), Side::Left));
        b.place(Tile::new(4, 1).unwrap(), Side::Left);
        assert_eq!(b.tiles[0], (Pip(1), Pip(4)));
    }

    #[test]
    fn count_pip() {
        let mut b = Board::new();
        b.place(Tile::new(4, 4).unwrap(), Side::Right);
        b.place(Tile::new(4, 1).unwrap(), Side::Left);
        assert_eq!(b.count_pip(Pip(4)), 3);
        assert_eq!(b.count_pip(Pip(1)), 1);
        assert_eq!(b.count_pip(Pip(2)), 0);
    }

    #[test]
    fn resulting_open_face() {
        let mut b = Board::new();
        b.place(Tile::new(3, 5).unwrap(), Side::Right);
        b.place(Tile::new(5, 6).unwrap(), Side::Right);
        // Extrémités : 3 à gauche, 6 à droite.
        assert_eq!(
            b.resulting_open_face(Tile::new(3, 2).unwrap(), Side::Left),
            Some(Pip(2))
        );
        assert_eq!(
            b.resulting_open_face(Tile::new(6, 1).unwrap(), Side::Right),
            Some(Pip(1))
        );
        // Pose 6-1 à droite : la face ouverte devient 1.
        b.place(Tile::new(6, 1).unwrap(), Side::Right);
        assert_eq!(b.right_end(), Some(Pip(1)));
    }
}
