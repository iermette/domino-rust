use domino_core::bot::choose_move;
use domino_core::{
    all_tiles, BotLevel, Game, GameError, Phase, Pip, Player, PlayerKind, RoundKind, Side,
    Tile,
};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn mk(id: u8, name: &str) -> Player {
    Player {
        id,
        name: name.into(),
        kind: PlayerKind::Human,
        hand: Vec::new(),
        score: 0,
        connected: true,
    }
}

#[test]
fn distribution_couvre_les_28_dominos() {
    for n in [2u8, 3, 4] {
        let players: Vec<Player> = (0..n).map(|i| mk(i, &format!("P{i}"))).collect();
        let mut g = Game::with_players(players, 100, 42);
        g.start().unwrap();
        let total: usize =
            g.board.len() + g.boneyard.len() + g.players.iter().map(|p| p.hand.len()).sum::<usize>();
        assert_eq!(total, 28);
        assert_eq!(
            g.players.iter().map(|p| p.hand.len()).sum::<usize>(),
            n as usize * 7
        );
        assert_eq!(g.boneyard.len(), 28 - 7 * n as usize);
        assert!(g.opening_tile.is_some());
        assert_eq!(g.turn, g.opener);
    }
}

#[test]
fn distribution_deterministe_avec_seed() {
    let mk = |seed: u64| {
        let players: Vec<Player> = (0..4u8).map(|i| mk(i, &format!("P{i}"))).collect();
        let mut g = Game::with_players(players, 100, seed);
        g.start().unwrap();
        g.players[0].hand.clone()
    };
    assert_eq!(mk(7), mk(7));
}

#[test]
fn premiere_main_obligation_d_ouverture() {
    let players: Vec<Player> = (0..4u8).map(|i| mk(i, &format!("P{i}"))).collect();
    let mut g = Game::with_players(players, 100, 7);
    g.start().unwrap();
    let opener = g.opener;
    let hand = g.players[opener as usize].hand.clone();
    let opening = g.opening_tile.unwrap();

    // Un autre domino de la main doit être refusé.
    if let Some(other) = hand.iter().copied().find(|t| *t != opening) {
        assert_eq!(
            g.play(opener, other, Side::Left),
            Err(GameError::OpeningTileRequired)
        );
    }
    // Un joueur non-ouvreur ne peut pas jouer non plus.
    let other_id = g.next_id(opener);
    assert_eq!(
        g.play(other_id, opening, Side::Left),
        Err(GameError::NotYourTurn)
    );
    // Le domino d'ouverture passe.
    g.play(opener, opening, Side::Left).unwrap();
    assert_eq!(g.board.len(), 1);
    assert_eq!(g.turn, g.next_id(opener));
}

#[test]
fn pose_invalides_rejetees() {
    let players: Vec<Player> = (0..2u8).map(|i| mk(i, &format!("P{i}"))).collect();
    let mut g = Game::with_players(players, 100, 11);
    g.start().unwrap();
    let opening = g.opening_tile.unwrap();
    g.play(g.opener, opening, Side::Left).unwrap();
    let p2 = g.next_id(g.opener);
    // P2 au tour : un domino non dans la main est refusé.
    let t = Tile::new(0, 1).unwrap();
    if !g.players[p2 as usize].hand.iter().any(|h| *h == t) {
        assert_eq!(g.play(p2, t, Side::Left), Err(GameError::TileNotInHand));
    }
    // Un domino de la main ne correspondant à aucune extrémité est refusé.
    let bad = g.players[p2 as usize]
        .hand
        .iter()
        .copied()
        .find(|t| !g.board.can_place(*t, Side::Left) && !g.board.can_place(*t, Side::Right));
    if let Some(t) = bad {
        assert_eq!(g.play(p2, t, Side::Left), Err(GameError::CannotPlay));
    }
}

#[test]
fn pioche_uniquement_si_bloque() {
    let players: Vec<Player> = (0..2u8).map(|i| mk(i, &format!("P{i}"))).collect();
    let mut g = Game::with_players(players, 100, 13);
    g.start().unwrap();
    let opening = g.opening_tile.unwrap();
    g.play(g.opener, opening, Side::Left).unwrap();
    let p2 = g.next_id(g.opener);
    // P2 : s'il a un coup, la pioche est refusée ; sinon elle est acceptée (pioche non vide).
    if g.can_play_any(p2) {
        assert_eq!(g.draw(p2), Err(GameError::CanStillPlay));
    } else {
        assert!(g.boneyard.is_empty() || g.draw(p2).is_ok());
    }
}

#[test]
fn passe_uniquement_pioche_vide_et_bloque() {
    let players: Vec<Player> = (0..2u8).map(|i| mk(i, &format!("P{i}"))).collect();
    let mut g = Game::with_players(players, 100, 17);
    g.start().unwrap();
    let opening = g.opening_tile.unwrap();
    g.play(g.opener, opening, Side::Left).unwrap();
    let p2 = g.next_id(g.opener);
    if !g.can_play_any(p2) {
        if !g.boneyard.is_empty() {
            assert_eq!(g.pass(p2), Err(GameError::BoneyardEmpty));
        } else {
            assert!(g.pass(p2).is_ok());
        }
    }
}

#[test]
fn manche_fermee_le_plus_bas_gagne() {
    let players = vec![mk(0, "A"), mk(1, "B")];
    let mut g = Game::with_players(players, 100, 9);
    g.phase = Phase::Playing;
    g.round = 1;
    g.first_hand = false;
    g.opener = 1;
    g.turn = 0;
    // Plateau bloqué : extrémités 6 / 6.
    g.board.tiles = vec![(Pip(2), Pip(6)), (Pip(6), Pip(6)), (Pip(6), Pip(6))];
    g.boneyard.clear();
    g.players[0].hand = vec![Tile::new(0, 1).unwrap()]; // 1 pt
    g.players[1].hand = vec![Tile::new(0, 3).unwrap()]; // 3 pts
    g.check_round_end();
    assert_eq!(g.phase, Phase::RoundOver);
    let rr = g.round_result.as_ref().unwrap();
    assert_eq!(rr.kind, RoundKind::Closed);
    assert_eq!(rr.winner, 0);
    assert_eq!(rr.awarded, 3);
    assert!(!rr.tie_break);
    assert_eq!(g.players[0].score, 3);
}

#[test]
fn egalite_departagee_par_la_main() {
    let players = vec![mk(0, "A"), mk(1, "B")];
    let mut g = Game::with_players(players, 100, 9);
    g.phase = Phase::Playing;
    g.round = 1;
    g.first_hand = false;
    g.opener = 0; // A a la main
    g.turn = 1;
    g.board.tiles = vec![(Pip(2), Pip(6)), (Pip(6), Pip(6)), (Pip(6), Pip(6))];
    g.boneyard.clear();
    g.players[0].hand = vec![Tile::new(0, 1).unwrap()]; // 1 pt
    g.players[1].hand = vec![Tile::new(0, 1).unwrap()]; // 1 pt → égalité
    g.check_round_end();
    assert_eq!(g.phase, Phase::RoundOver);
    let rr = g.round_result.as_ref().unwrap();
    assert_eq!(rr.winner, 0, "la main doit départager");
    assert!(rr.tie_break);
}

#[test]
fn manche_ouverte_score_des_adversaires() {
    let players = vec![mk(0, "A"), mk(1, "B")];
    let mut g = Game::with_players(players, 100, 3);
    g.phase = Phase::Playing;
    g.round = 1;
    g.first_hand = false;
    g.opener = 0;
    g.turn = 0;
    g.board.tiles = vec![(Pip(1), Pip(2))];
    g.players[0].hand = vec![Tile::new(1, 2).unwrap()];
    g.players[1].hand = vec![Tile::new(3, 4).unwrap()]; // 7 pts
    g.play(0, Tile::new(1, 2).unwrap(), Side::Right).unwrap();
    assert_eq!(g.phase, Phase::RoundOver);
    let rr = g.round_result.as_ref().unwrap();
    assert_eq!(rr.kind, RoundKind::Open);
    assert_eq!(rr.winner, 0);
    assert_eq!(rr.awarded, 7);
    assert_eq!(g.players[0].score, 7);
}

#[test]
fn manche_ouverte_atteint_cible_fin_de_partie() {
    let players = vec![mk(0, "A"), mk(1, "B")];
    let mut g = Game::with_players(players, 5, 3);
    g.phase = Phase::Playing;
    g.round = 1;
    g.first_hand = false;
    g.opener = 0;
    g.turn = 0;
    g.board.tiles = vec![(Pip(1), Pip(2))];
    g.players[0].hand = vec![Tile::new(1, 2).unwrap()];
    g.players[1].hand = vec![Tile::new(3, 4).unwrap()];
    g.play(0, Tile::new(1, 2).unwrap(), Side::Right).unwrap();
    assert_eq!(g.phase, Phase::MatchOver);
    assert_eq!(g.match_winner, Some(0));
}

#[test]
fn manche_suivante_l_ouvreur_tourne() {
    let players: Vec<Player> = (0..4u8).map(|i| mk(i, &format!("P{i}"))).collect();
    let mut g = Game::with_players(players, 100, 1);
    g.start().unwrap();
    let first_opener = g.opener;
    g.phase = Phase::RoundOver;
    g.advance_round().unwrap();
    assert_eq!(g.opener, g.next_id(first_opener));
    assert!(!g.first_hand);
    assert!(g.opening_tile.is_none());
    assert!(g.board.is_empty());
    assert_eq!(g.round, 2);
}

#[test]
fn partie_complete_4_bots_terminaison_et_invariants() {
    for n in [2u8, 3, 4] {
        for seed in 0..40u64 {
            let players: Vec<Player> = (0..n)
                .map(|i| Player {
                    id: i,
                    name: format!("B{i}"),
                    kind: PlayerKind::Bot(BotLevel::Hard),
                    hand: vec![],
                    score: 0,
                    connected: true,
                })
                .collect();
            let mut g = Game::with_players(players, 25, seed);
            g.start().unwrap();
            let mut rng = StdRng::seed_from_u64(seed.wrapping_add(999));
            let mut steps = 0u32;
            loop {
                while g.phase == Phase::Playing {
                    let id = g.turn;
                    let moves = g.legal_moves(id);
                    if !moves.is_empty() {
                        let (t, s) = choose_move(BotLevel::Hard, &moves, &g.board, g.boneyard.len(), &mut rng);
                        g.play(id, t, s).unwrap();
                    } else if !g.boneyard.is_empty() {
                        g.draw(id).unwrap();
                    } else {
                        g.pass(id).unwrap();
                    }
                    steps += 1;
                    assert!(steps < 200_000, "boucle infinie détectée");
                    let total: usize =
                        g.board.len() + g.boneyard.len() + g.players.iter().map(|p| p.hand.len()).sum::<usize>();
                    assert_eq!(total, 28, "invariant de conservation des dominos");
                }
                match g.phase {
                    Phase::MatchOver => break,
                    Phase::RoundOver => {
                        g.advance_round().unwrap();
                    }
                    _ => panic!("phase inattendue"),
                }
            }
            assert_eq!(g.phase, Phase::MatchOver);
            let w = g.match_winner.expect("un gagnant existe");
            assert!(g.players[w as usize].score >= g.target);
            let _ = all_tiles().len(); // garde l'import
        }
    }
}
