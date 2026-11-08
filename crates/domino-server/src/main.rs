//! Domino — serveur local & LAN.
//! Un port unique : HTTP (client web) + WebSocket (jeu). Le serveur est l'autorité.
mod http;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};

use domino_core::bot::choose_move;
use domino_core::{
    Board, BotLevel, Game, GameError, Phase, PlayerKind, RoundResult, Side, Tile,
};

/// Client web embarqué au moment de la compilation (binaire autonome).
pub const WEB_HTML: &str = include_str!("../web/index.html");

// Avatars embarqués au compile (dossier web/avatars/, voir build.rs).
include!(concat!(env!("OUT_DIR"), "/avatars_gen.rs"));

#[derive(Parser)]
#[command(name = "domino", about = "Serveur de domino — local & LAN")]
struct Args {
    /// Port TCP à écouter.
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// Points à atteindre pour gagner la partie (50-300).
    #[arg(long, default_value_t = 100)]
    target: u16,
    /// Nombre de places à la table (2-4).
    #[arg(long, default_value_t = 4)]
    players: u8,
    /// Remplit automatiquement les places vides de bots au lancement de la partie.
    #[arg(long, default_value_t = false)]
    bot_fill: bool,
    /// Graine du générateur aléatoire (déterminisme).
    #[arg(long, default_value_t = 0x5eed)]
    seed: u64,
}

#[derive(Clone)]
struct Sock {
    tx: mpsc::Sender<Message>,
    seat: u8,
}

#[derive(Clone)]
struct Shared {
    game: Arc<Mutex<Game>>,
    socks: Arc<Mutex<HashMap<usize, Sock>>>,
    conn: Arc<AtomicUsize>,
    rng: Arc<Mutex<StdRng>>,
    /// Complète la table avec des bots au lancement (modifiable via le menu).
    bot_fill: Arc<AtomicBool>,
    /// Dernier état JSON envoyé par siège — évite de rebroadcast un état inchangé.
    last_sent: Arc<Mutex<HashMap<u8, String>>>,
}

impl Shared {
    fn new(target: u16, seed: u64, bot_fill: bool) -> Self {
        Self {
            game: Arc::new(Mutex::new(Game::new(target, seed))),
            socks: Arc::new(Mutex::new(HashMap::new())),
            conn: Arc::new(AtomicUsize::new(0)),
            // RNG des prénoms de bots : entropie horloge (indépendante du --seed,
            // qui reste déterministe pour les distributions) → noms différents à chaque session.
            rng: Arc::new(Mutex::new(StdRng::seed_from_u64(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(seed.wrapping_add(0x9e3779b97f4a7c15)),
            ))),
            bot_fill: Arc::new(AtomicBool::new(bot_fill)),
            last_sent: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// ---------- Vue envoyée aux clients (la main des autres est masquée) ----------

#[derive(Serialize)]
struct PlayerView {
    id: u8,
    name: String,
    kind: PlayerKind,
    score: u16,
    connected: bool,
    hand_len: usize,
    hand: Option<Vec<Tile>>,
}

#[derive(Serialize)]
struct State {
    players: Vec<PlayerView>,
    boneyard_len: usize,
    board: Board,
    phase: Phase,
    turn: u8,
    round: u32,
    opener: u8,
    target: u16,
    /// Nombre maximum de sièges à la table (2–4).
    max_players: usize,
    /// Compléter automatiquement les sièges vides avec des bots au lancement.
    fill_bots: bool,
    first_hand: bool,
    opening_tile: Option<Tile>,
    round_result: Option<RoundResult>,
    match_winner: Option<u8>,
    log: Vec<String>,
    your_id: u8,
    legal_moves: Vec<(Tile, Side)>,
}

fn state_for(g: &Game, me: u8, fill_bots: bool) -> State {
    let from = g.log.len().saturating_sub(30);
    State {
        players: g
            .players
            .iter()
            .map(|p| PlayerView {
                id: p.id,
                name: p.name.clone(),
                kind: p.kind,
                score: p.score,
                connected: p.connected,
                hand_len: p.hand.len(),
                hand: (p.id == me).then(|| p.hand.clone()),
            })
            .collect(),
        boneyard_len: g.boneyard.len(),
        board: g.board.clone(),
        phase: g.phase,
        turn: g.turn,
        round: g.round,
        opener: g.opener,
        target: g.target,
        max_players: g.max_players,
        fill_bots,
        first_hand: g.first_hand,
        opening_tile: g.opening_tile,
        round_result: g.round_result.clone(),
        match_winner: g.match_winner,
        log: g.log.iter().skip(from).cloned().collect(),
        your_id: me,
        legal_moves: (g.turn == me && g.phase == Phase::Playing).then(|| g.legal_moves(me))
            .unwrap_or_default(),
    }
}

async fn broadcast(shared: &Shared) {
    let fill_bots = shared.bot_fill.load(Ordering::SeqCst);
    // On ne renvoie l'état que s'il a réellement changé : sinon le client
    // reconstruirait son DOM (et relancerait ses animations) toutes les 120 ms.
    let msgs: Vec<(u8, String)> = {
        let g = shared.game.lock().await;
        g.players
            .iter()
            .filter(|p| p.connected)
            .filter_map(|p| {
                serde_json::to_string(&state_for(&g, p.id, fill_bots))
                    .ok()
                    .map(|json| (p.id, json))
            })
            .collect()
    };
    let mut last = shared.last_sent.lock().await;
    let socks = shared.socks.lock().await;
    for (seat, json) in msgs {
        if last.get(&seat) == Some(&json) {
            continue; // état identique : rien à envoyer.
        }
        if let Some(s) = socks.values().find(|s| s.seat == seat) {
            let _ = s.tx.send(Message::Text(json.clone().into())).await;
        }
        last.insert(seat, json);
    }
}

// ---------- Noms aléatoires (prénoms français) ----------

/// Top 400 des prénoms les plus portés en France depuis 1900 (données INSEE).
const NAMES_POOL: &str = include_str!("../names.txt");

/// Attribue un prénom français aléatoire, non déjà utilisé à la table.
async fn assign_name(shared: &Shared, g: &Game) -> String {
    let pool: Vec<&str> = NAMES_POOL
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|n| !g.players.iter().any(|p| p.name == *n))
        .collect();
    if pool.is_empty() {
        return "Joueur".into(); // quasi impossible avec 400 prénoms.
    }
    let idx = shared.rng.lock().await.random_range(0..pool.len());
    pool[idx].to_string()
}

// ---------- Bot ----------

async fn bot_turn(shared: &Shared, id: u8) {
    let level = {
        let g = shared.game.lock().await;
        if g.phase != Phase::Playing || g.turn != id {
            return;
        }
        match g.player(id) {
            Some(p) => match p.kind {
                PlayerKind::Bot(l) => l,
                _ => return,
            },
            None => return,
        }
    };
    // Petit délai « humain ».
    {
        let mut rng = shared.rng.lock().await;
        let d = rng.random_range(400..1200u64);
        tokio::time::sleep(Duration::from_millis(d)).await;
    }
    let mut g = shared.game.lock().await;
    if g.phase != Phase::Playing || g.turn != id {
        return;
    }
    let moves = g.legal_moves(id);
    if moves.is_empty() {
        if g.boneyard.is_empty() {
            if let Err(e) = g.pass(id) {
                g.push_log(format!("⚠️ {e}"));
            }
        } else if let Err(e) = g.draw(id) {
            g.push_log(format!("⚠️ {e}"));
        }
        return;
    }
    let mut rng = shared.rng.lock().await;
    let (t, s) = choose_move(level, &moves, &g.board, g.boneyard.len(), &mut rng);
    if let Err(e) = g.play(id, t, s) {
        g.push_log(format!("⚠️ {e}"));
    }
}

// ---------- Intentions des joueurs ----------

#[derive(Deserialize)]
struct Intent {
    t: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    tile: Option<String>,
    #[serde(default)]
    side: Option<String>,
    #[serde(default)]
    id: Option<u8>,
    /// Réglages (menu, phase waiting) : cible de points (50–300).
    #[serde(default)]
    target: Option<u16>,
    /// Réglages : nombre maximum de joueurs à la table (2–4).
    #[serde(default)]
    max_players: Option<u8>,
    /// Réglages : compléter la table avec des bots au lancement.
    #[serde(default)]
    fill_bots: Option<bool>,
}

async fn handle_intent(shared: &Shared, seat: u8, raw: &str) {
    let it: Intent = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return,
    };
    let mut g = shared.game.lock().await;
    if g.player(seat).is_none() {
        return;
    }
    let res: Result<(), GameError> = match it.t.as_str() {
        "join" => Ok(()),
        "play" => {
            let (tile, side) = match (
                it.tile.as_deref().and_then(|s| Tile::from_str(s).ok()),
                it.side.as_deref().and_then(Side::parse),
            ) {
                (Some(tile), Some(side)) => (tile, side),
                _ => return,
            };
            g.play(seat, tile, side)
        }
        "draw" => g.draw(seat).map(|_| ()),
        "pass" => g.pass(seat),
        "start" => {
            if shared.bot_fill.load(Ordering::SeqCst) && g.phase == Phase::Waiting {
                let levels = [BotLevel::Easy, BotLevel::Medium, BotLevel::Hard];
                while g.players.len() < g.max_players {
                    let lvl = levels[g.players.len() % levels.len()];
                    // Prénom français aléatoire (le niveau reste visible via la pastille BOT).
                    let name = assign_name(shared, &g).await;
                    if g.add_player(name, PlayerKind::Bot(lvl)).is_err() {
                        break;
                    }
                }
            }
            g.start()
        }
        "next" => g.advance_round(),
        "add_bot" => {
            let level = it
                .level
                .as_deref()
                .and_then(BotLevel::parse)
                .unwrap_or(BotLevel::Medium);
            let name = assign_name(shared, &g).await;
            g.add_player(name, PlayerKind::Bot(level)).map(|_| ())
        }
        "remove_bot" => {
            let bid = it.id;
            match bid {
                Some(b) if g.player(b).map(|p| matches!(p.kind, PlayerKind::Bot(_))) == Some(true) => {
                    match g.remove_player(b as usize) {
                        Some(_) => Ok(()),
                        None => Err(GameError::InvalidPlayer),
                    }
                }
                _ => Err(GameError::InvalidPlayer),
            }
        }
        "rename" => {
            let name = it.name.as_deref().map(str::trim).unwrap_or("");
            if g.phase != Phase::Waiting || name.is_empty() {
                Err(GameError::PhaseMismatch)
            } else {
                if let Some(p) = g.players.get_mut(seat as usize) {
                    p.name = name.chars().take(20).collect();
                }
                Ok(())
            }
        }
        "config" => {
            // Réglages de la partie, uniquement avant le lancement.
            if g.phase != Phase::Waiting {
                Err(GameError::PhaseMismatch)
            } else {
                let mut notes: Vec<String> = Vec::new();
                if let Some(t) = it.target {
                    let t = t.clamp(50, 300);
                    if t != g.target {
                        g.target = t;
                        notes.push(format!("cible {t} pts"));
                    }
                }
                if let Some(mp) = it.max_players {
                    let mp = (mp as usize).clamp(2, 4);
                    // En réduisant la table, on retire d'abord les bots en bout de file.
                    loop {
                        if g.players.len() <= mp {
                            break;
                        }
                        let last_is_bot =
                            matches!(g.players.last().map(|p| &p.kind), Some(PlayerKind::Bot(_)));
                        if !last_is_bot {
                            break;
                        }
                        let pos = g.players.len() - 1;
                        let _ = g.remove_player(pos);
                    }
                    match g.set_max_players(mp) {
                        Ok(()) => notes.push(format!("table de {mp}")),
                        Err(_) => {
                            g.push_log("⚠️ Impossible de réduire la table : trop de joueurs humains assis.".into());
                        }
                    }
                }
                if !notes.is_empty() {
                    g.push_log(format!("⚙️ Réglages — {}", notes.join(" · ")));
                }
                Ok(())
            }
        }
        _ => return,
    };
    if let Err(e) = res {
        g.push_log(format!("⚠️ {e}"));
    }
    drop(g);
    if let Some(f) = it.fill_bots {
        shared.bot_fill.store(f, Ordering::SeqCst);
    }
    broadcast(shared).await;
}

// ---------- Connexion WebSocket ----------

async fn handle_ws<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    mut ws: WebSocketStream<S>,
    shared: Shared,
) {
    // 1) Première trame : « join » {name} (vide ou absent → prénom français aléatoire).
    let raw_name = loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => {
                let it: Intent = serde_json::from_str(&t).unwrap_or(Intent {
                    t: "join".into(),
                    name: None,
                    level: None,
                    tile: None,
                    side: None,
                    id: None,
                    target: None,
                    max_players: None,
                    fill_bots: None,
                });
                break it.name;
            }
            Some(Ok(Message::Binary(b))) => {
                let s = String::from_utf8_lossy(&b).into_owned();
                let it: Intent = serde_json::from_str(&s).unwrap_or(Intent {
                    t: "join".into(),
                    name: None,
                    level: None,
                    tile: None,
                    side: None,
                    id: None,
                    target: None,
                    max_players: None,
                    fill_bots: None,
                });
                break it.name;
            }
            Some(Ok(Message::Ping(p))) => {
                let _ = ws.send(Message::Pong(p)).await;
                continue;
            }
            _ => return,
        }
    };

    // 2) On s'assoit (refusé en cours de manche, pour rester propre).
    //    Le nom humain est celui saisi au lancement ; vide → « Joueur ».
    //    (Les prénoms français aléatoires ne concernent que les bots.)
    let seat = {
        let mut g = shared.game.lock().await;
        let name = match &raw_name {
            Some(n) if !n.trim().is_empty() => n.clone(),
            _ => "Joueur".to_string(),
        };
        match g.add_player(name, PlayerKind::Human) {
            Ok(id) => {
                let st = state_for(&g, id, shared.bot_fill.load(Ordering::SeqCst));
                drop(g);
                if ws
                    .send(Message::Text(
                        serde_json::to_string(&st).unwrap_or_default().into(),
                    ))
                    .await
                    .is_err()
                {
                    return;
                }
                id
            }
            Err(e) => {
                drop(g);
                let msg = serde_json::json!({"t":"error","msg":e.to_string()}).to_string();
                let _ = ws.send(Message::Text(msg.into())).await;
                return;
            }
        }
    };

    // 3) Canal sortant (broadcast → ws).
    let (tx, mut sock_rx) = mpsc::channel::<Message>(128);
    let conn_id = shared.conn.fetch_add(1, Ordering::SeqCst);
    shared
        .socks
        .lock()
        .await
        .insert(conn_id, Sock { tx: tx.clone(), seat });

    // 4) Boucle : intentions entrantes + états sortants.
    loop {
        tokio::select! {
            inc = ws.next() => {
                match inc {
                    Some(Ok(Message::Text(t))) => handle_intent(&shared, seat, &t).await,
                    Some(Ok(Message::Binary(b))) => {
                        handle_intent(&shared, seat, &String::from_utf8_lossy(&b)).await
                    }
                    Some(Ok(Message::Ping(p))) => {
                        let _ = ws.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                    None => break,
                }
            }
            out = sock_rx.recv() => {
                match out {
                    Some(m) => {
                        if ws.send(m).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }
    // 5) Déconnexion : le bot prend le relais si la manche est en cours.
    {
        let mut g = shared.game.lock().await;
        if let Some(p) = g.player(seat) {
            let name = p.name.clone();
            match g.phase {
                Phase::Playing | Phase::RoundOver => {
                    if let Some(p) = g.players.get_mut(seat as usize) {
                        p.kind = PlayerKind::Bot(BotLevel::Medium);
                        p.connected = false;
                    }
                    g.push_log(format!("🤖 {name} s'est déconnecté — un bot reprend sa main."));
                }
                Phase::Waiting | Phase::MatchOver => {
                    g.remove_player(seat as usize);
                    g.push_log(format!("👋 {name} quitte la table."));
                }
            }
        }
    }
    shared.socks.lock().await.remove(&conn_id);
    // Oublie l'état en cache : un nouveau joueur pourra reprendre ce siège.
    shared.last_sent.lock().await.remove(&seat);
    broadcast(&shared).await;
}

// ---------- Boucle de jeu (tour des bots) ----------

async fn game_loop(shared: Shared) {
    loop {
        tokio::time::sleep(Duration::from_millis(120)).await;
        let (phase, turn) = {
            let g = shared.game.lock().await;
            (g.phase, g.turn)
        };
        if phase == Phase::Playing {
            let is_bot = {
                let g = shared.game.lock().await;
                matches!(g.player(turn).map(|p| p.kind), Some(PlayerKind::Bot(_)))
            };
            if is_bot {
                bot_turn(&shared, turn).await;
            }
        }
        broadcast(&shared).await;
    }
}

// ---------- Main ----------

fn guess_lan_ip() -> Option<String> {
    use std::net::UdpSocket;
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("1.1.1.1:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let target = args.target.clamp(50, 300);
    if !(2..=4).contains(&args.players) {
        eprintln!("--players doit être compris entre 2 et 4 (reçu : {})", args.players);
        std::process::exit(2);
    }
    let shared = Shared::new(target, args.seed, args.bot_fill);
    {
        let mut g = shared.game.lock().await;
        if let Err(e) = g.set_max_players(args.players as usize) {
            eprintln!("Plafond de joueurs invalide : {e}");
            std::process::exit(2);
        }
    }
    let listener = TcpListener::bind(("0.0.0.0", args.port))
        .await
        .unwrap_or_else(|e| panic!("Échec de l'écoute sur le port {} : {e}", args.port));
    let ip = guess_lan_ip().unwrap_or_else(|| "localhost".into());
    println!("🁫  Domino — serveur prêt");
    println!("    Vous  : http://localhost:{}", args.port);
    println!("    LAN   : http://{}:{}", ip, args.port);
    println!("    Cible : {target} pts — vos amis ouvrent ce lien dans leur navigateur.");
    tokio::spawn(game_loop(shared.clone()));
    loop {
        let (mut stream, _addr) = match listener.accept().await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            let (raw, path, upgrade) = match http::read_head(&mut stream).await {
                Ok(v) => v,
                Err(_) => return,
            };
            if upgrade {
                // Restitue les octets de la requête déjà lus, puis le flux.
                let combined = http::Prepend::new(raw, stream);
                match accept_async(combined).await {
                    Ok(ws) => handle_ws(ws, shared).await,
                    Err(e) => eprintln!("poignée de main WS refusée : {e}"),
                }
            } else {
                http::serve(stream, &WEB_HTML, &path, AVATARS).await;
            }
        });
    }
}
