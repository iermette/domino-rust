// Test de fumée : rejoint la table, ajoute 3 bots, joue une partie complète.
const url = process.env.WS_URL || "ws://127.0.0.1:8080";
const ws = new WebSocket(url);
let me = null, states = 0, started = false;

const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const ok = (m) => console.log("OK:", m);

ws.onopen = () => ws.send(JSON.stringify({ t: "join", name: "Smoke" }));

ws.onmessage = (e) => {
  const d = JSON.parse(e.data);
  if (d.t === "error") { fail(d.msg); return; }
  me = d.your_id;
  states++;
  const last = d.log[d.log.length - 1] || "";
  console.log(`[${d.phase}] r=${d.round} tour=${d.turn} plateau=${d.board.tiles.length} pioche=${d.boneyard_len} | ${last}`);

  if (states === 1 && d.phase === "waiting") {
    setTimeout(() => ws.send(JSON.stringify({ t: "add_bot", level: "hard" })), 50);
    setTimeout(() => ws.send(JSON.stringify({ t: "add_bot", level: "medium" })), 120);
    setTimeout(() => ws.send(JSON.stringify({ t: "add_bot", level: "easy" })), 190);
    setTimeout(() => ws.send(JSON.stringify({ t: "start" })), 350);
  }

  // Joue mes coups si c'est mon tour.
  if (d.phase === "playing" && d.turn === me) {
    const mv = d.legal_moves[0];
    if (mv) {
      ws.send(JSON.stringify({ t: "play", tile: `${mv[0].a}-${mv[0].b}`, side: mv[1] }));
    } else if (d.boneyard_len > 0) {
      ws.send(JSON.stringify({ t: "draw" }));
    } else {
      ws.send(JSON.stringify({ t: "pass" }));
    }
  }

  if (d.phase === "round_over") ws.send(JSON.stringify({ t: "next" }));

  if (d.phase === "match_over") {
    const w = d.players.find((p) => p.id === d.match_winner);
    ok(`partie complète terminée en ${states} états — vainqueur : ${w ? w.name : "?"} (${w ? w.score : "?"} pts)`);
    ws.close();
    process.exit(0);
  }
};

ws.onerror = (e) => fail("ws error: " + (e.message || "inconnu"));
ws.onclose = () => { if (states === 0) fail("fermeture sans aucun état reçu"); };
setTimeout(() => fail("timeout 180 s — la partie n'a pas convergé"), 180000);
