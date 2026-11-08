const url = "ws://127.0.0.1:8092";
const ws = new WebSocket(url);
let last = null, stable = 0, prevKey = "";
ws.onopen = () => ws.send(JSON.stringify({ t: "join", name: "Solo" }));
ws.onmessage = (e) => {
  const d = JSON.parse(e.data);
  if (d.t === "error") { console.log("ERR:", d.msg); return; }
  const key = JSON.stringify([d.phase, d.turn, d.boneyard_len, d.round, d.players.map(p => p.hand_len).join(","), d.board]);
  stable = key === prevKey ? stable + 1 : 0;
  prevKey = key;
  if (d.phase === "waiting") ws.send(JSON.stringify({ t: "start" }));
  else if (d.phase === "playing" && d.turn === d.your_id) {
    const mv = d.legal_moves[0];
    if (mv) ws.send(JSON.stringify({ t: "play", tile: `${mv[0].a}-${mv[0].b}`, side: mv[1] === "Left" ? "left" : "right" }));
    else if (d.boneyard_len > 0) ws.send(JSON.stringify({ t: "draw" }));
    else ws.send(JSON.stringify({ t: "pass" }));
  } else if (d.phase === "round_over") ws.send(JSON.stringify({ t: "next" }));
  else if (d.phase === "match_over") { console.log("FIN:", d.players.find(p => p.id === d.match_winner)?.name); process.exit(0); }
  if (stable >= 4) {
    console.log("GEL — état complet :");
    console.log(JSON.stringify({
      phase: d.phase, turn: d.turn, your_id: d.your_id, round: d.round,
      boneyard_len: d.boneyard_len,
      board: d.board,
      legal_moves: d.legal_moves,
      hands: d.players.map(p => `${p.id}:${p.hand_len}`),
      log: d.log.slice(-5)
    }, null, 1));
    process.exit(1);
  }
};
setTimeout(() => { console.log("TIMEOUT global"); process.exit(1); }, 60000);
