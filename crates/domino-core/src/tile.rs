use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Un nombre sur une face de domino (0 à 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pip(pub u8);

impl Pip {
    pub fn new(value: u8) -> Option<Self> {
        (value <= 6).then_some(Pip(value))
    }

    pub fn value(self) -> u8 {
        self.0
    }

    pub fn as_points(self) -> u16 {
        self.0 as u16
    }
}

impl fmt::Display for Pip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Un domino. Stocké sous forme canonique : `a <= b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tile {
    pub a: Pip,
    pub b: Pip,
}

impl Tile {
    /// Crée un domino à partir de deux faces (l'ordre est sans importance).
    pub fn new(a: u8, b: u8) -> Option<Self> {
        let (x, y) = (Pip::new(a)?, Pip::new(b)?);
        if x <= y {
            Some(Self { a: x, b: y })
        } else {
            Some(Self { a: y, b: x })
        }
    }

    pub fn is_double(self) -> bool {
        self.a == self.b
    }

    /// Nombre de points du domino.
    pub fn points(self) -> u16 {
        self.a.as_points() + self.b.as_points()
    }

    pub fn matches(self, pip: Pip) -> bool {
        self.a == pip || self.b == pip
    }

    /// La face opposée à `pip` (si le domino correspond à `pip`).
    pub fn other_face(self, pip: Pip) -> Option<Pip> {
        if self.a == self.b {
            Some(self.a)
        } else if self.a == pip {
            Some(self.b)
        } else if self.b == pip {
            Some(self.a)
        } else {
            None
        }
    }

    /// Clé unique « a-b » (a <= b), ex. « 3-5 ».
    pub fn key(self) -> String {
        format!("{}-{}", self.a.value(), self.b.value())
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.a.value(), self.b.value())
    }
}

impl FromStr for Tile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(['-', ' ', '_']).collect();
        if parts.len() != 2 {
            return Err(format!("domino invalide : {s:?} (attendu « 3-5 »)"));
        }
        let a: u8 = parts[0]
            .trim()
            .parse()
            .map_err(|e| format!("face invalide : {e}"))?;
        let b: u8 = parts[1]
            .trim()
            .parse()
            .map_err(|e| format!("face invalide : {e}"))?;
        Self::new(a, b).ok_or_else(|| format!("faces hors plage 0-6 : {a}, {b}"))
    }
}

/// Le plateau double-six complet : 28 dominos uniques.
pub fn all_tiles() -> Vec<Tile> {
    let mut v = Vec::with_capacity(28);
    for a in 0..=6u8 {
        for b in a..=6u8 {
            v.push(Tile {
                a: Pip(a),
                b: Pip(b),
            });
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_de_28_dominos_uniques() {
        let tiles = all_tiles();
        assert_eq!(tiles.len(), 28);
        let mut set = std::collections::HashSet::new();
        for t in &tiles {
            assert!(t.a.value() <= 6 && t.b.value() <= 6);
            assert!(t.a <= t.b);
            set.insert(*t);
        }
        assert_eq!(set.len(), 28);
    }

    #[test]
    fn forme_canonique() {
        assert_eq!(Tile::new(5, 3).unwrap().a.value(), 3);
        assert_eq!(Tile::new(5, 3).unwrap().b.value(), 5);
        assert_eq!(Tile::new(6, 2).unwrap().key(), "2-6");
    }

    #[test]
    fn points_et_doubles() {
        assert_eq!(Tile::new(6, 6).unwrap().points(), 12);
        assert!(Tile::new(4, 4).unwrap().is_double());
        assert!(!Tile::new(4, 5).unwrap().is_double());
    }

    #[test]
    fn parse() {
        assert_eq!(Tile::from_str("3-5").unwrap(), Tile::new(3, 5).unwrap());
        assert_eq!(Tile::from_str("5_3").unwrap(), Tile::new(3, 5).unwrap());
        assert!(Tile::from_str("9-1").is_err());
        assert!(Tile::from_str("abc").is_err());
    }

    #[test]
    fn other_face() {
        let t = Tile::new(2, 6).unwrap();
        assert_eq!(t.other_face(Pip(6)), Some(Pip(2)));
        assert_eq!(t.other_face(Pip(2)), Some(Pip(6)));
        assert_eq!(t.other_face(Pip(3)), None);
        let d = Tile::new(4, 4).unwrap();
        assert_eq!(d.other_face(Pip(4)), Some(Pip(4)));
    }
}
