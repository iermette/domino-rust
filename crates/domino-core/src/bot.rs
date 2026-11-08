use rand::rngs::StdRng;
use rand::Rng;

use crate::board::{Board, Side};
use crate::game::BotLevel;
use crate::tile::Tile;

/// Choisit un coup pour un bot parmi les `moves` légaux (non vides).
///
/// - **Easy** : coup aléatoire.
/// - **Medium** : domino jouable le moins chargé en points (doubles de préférence en cas d'égalité).
/// - **Hard** : pénalise l'ouverture d'un chiffre encore présent, l'ouverture d'un chiffre
///   élevé, et favorise la purge des points en fin de pioche.
pub fn choose_move(
    level: BotLevel,
    moves: &[(Tile, Side)],
    board: &Board,
    boneyard: usize,
    rng: &mut StdRng,
) -> (Tile, Side) {
    assert!(!moves.is_empty(), "aucun coup jouable");
    match level {
        BotLevel::Easy => moves[rng.random_range(0..moves.len())],
        BotLevel::Medium => {
            let best = moves.iter().map(|(t, _)| t.points()).min().unwrap();
            let opts: Vec<(Tile, Side)> = moves
                .iter()
                .copied()
                .filter(|(t, _)| t.points() == best)
                .collect();
            opts[rng.random_range(0..opts.len())]
        }
        BotLevel::Hard => {
            let mut best: Option<(f32, Tile, Side)> = None;
            for (t, side) in moves.iter().copied() {
                let open = board.resulting_open_face(t, side).unwrap_or(t.a);
                // Chiffre déjà bien représenté sur le plateau = moins risqué d'y ouvrir.
                let copies = board.count_pip(open) as f32;
                let open_cost = (7.0 - copies) * 0.6;
                // Ouvrir un chiffre élevé coûte cher en points s'il est joué.
                let high_cost = open.value() as f32 * 0.15;
                let mut cost = t.points() as f32 + open_cost + high_cost;
                if t.is_double() {
                    cost -= 0.3; // purger les doubles tôt
                }
                if boneyard <= 2 {
                    cost += t.points() as f32 * 0.4; // fin de pioche : se débarrasser des points
                }
                if best.map_or(true, |(b, _, _)| cost < b) {
                    best = Some((cost, t, side));
                }
            }
            best.map(|(_, t, s)| (t, s))
                .expect("au moins un coup")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::all_tiles;
    use rand::SeedableRng;

    fn board_with_end(pip: u8) -> Board {
        let mut b = Board::new();
        b.place(Tile::new(0, pip).unwrap(), Side::Right);
        b
    }

    #[test]
    fn easy_toujours_jouable() {
        let mut rng = StdRng::seed_from_u64(1);
        let board = board_with_end(4);
        let mut moves = Vec::new();
        for t in all_tiles().iter().copied() {
            for s in [Side::Left, Side::Right] {
                if board.can_place(t, s) {
                    moves.push((t, s));
                }
            }
        }
        assert!(!moves.is_empty());
        for _ in 0..50 {
            let (t, s) = choose_move(BotLevel::Easy, &moves, &board, 10, &mut rng);
            assert!(board.can_place(t, s));
        }
    }

    #[test]
    fn medium_préfère_les_petites_sommes() {
        let mut rng = StdRng::seed_from_u64(2);
        let board = board_with_end(4);
        let moves = vec![
            (Tile::new(4, 6).unwrap(), Side::Right),
            (Tile::new(0, 4).unwrap(), Side::Right),
        ];
        let (t, _) = choose_move(BotLevel::Medium, &moves, &board, 10, &mut rng);
        assert_eq!(t, Tile::new(0, 4).unwrap());
    }

    #[test]
    fn hard_évite_d_ouvrir_un_chiffre_élevé() {
        let mut rng = StdRng::seed_from_u64(3);
        let board = board_with_end(0);
        // 0-6 ouvre un 6 (cher) alors que 0-1 ouvre un 1 (moins cher).
        let moves = vec![
            (Tile::new(0, 6).unwrap(), Side::Right),
            (Tile::new(0, 1).unwrap(), Side::Right),
        ];
        let (t, _) = choose_move(BotLevel::Hard, &moves, &board, 10, &mut rng);
        assert_eq!(t, Tile::new(0, 1).unwrap());
    }
}
