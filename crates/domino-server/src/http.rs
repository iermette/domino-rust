//! Mini serveur HTTP/1.1 async — sert le client web embarqué (GET uniquement).
//! `read_head` consomme l'en-tête de la connexion ; les octets sont restitués
//! via `Prepend` (flux composé) pour la poignée de main WebSocket.
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;

/// Lit l'en-tête de requête (jusqu'à \r\n\r\n).
/// Renvoie : (octets lus à restituer, path, est-ce un upgrade WebSocket ?).
pub async fn read_head(stream: &mut TcpStream) -> Result<(Vec<u8>, String, bool), ()> {
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp).await.map_err(|_| ())?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 65536 {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    let first = head.lines().next().unwrap_or("");
    let path = first
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let upgrade = head
        .lines()
        .any(|l| l.to_ascii_lowercase().trim() == "upgrade: websocket");
    Ok((buf, path, upgrade))
}

/// Sert le client web embarqué (GET), les avatars (`/avatars/*.png`), ou une 404.
pub async fn serve<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    html: &str,
    path: &str,
    avatars: &[(&str, &[u8])],
) {
    let clean: String = path
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '/' || *c == '.' || *c == '-' || *c == '_')
        .collect();
    let rel = clean.split('?').next().unwrap_or("").trim_start_matches('/');
    if let Some(fname) = rel.strip_prefix("avatars/") {
        // Avatars embarqués au compile (build.rs). no-cache : un rebuild peut les remplacer.
        if let Some((_, data)) = avatars.iter().find(|(n, _)| *n == fname) {
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ncontent-length: {}\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n",
                data.len()
            );
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(data).await;
            return;
        }
    }
    if rel.is_empty() || rel == "index.html" || rel == "/" {
        let data = html.as_bytes();
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n",
            data.len()
        );
        let _ = stream.write_all(head.as_bytes()).await;
        let _ = stream.write_all(data).await;
        return;
    }
    let resp = format!(
        "HTTP/1.1 404 Not Found\r\ncontent-type: text/plain; charset=utf-8\r\nconnection: close\r\n\r\n404 — {path} introuvable\r\n"
    );
    let _ = stream.write_all(resp.as_bytes()).await;
}

/// Flux composé : d'abord les octets déjà lus (restitués), puis le TcpStream.
/// Nécessaire car `read_head` a déjà consommé la requête WS.
pub struct Prepend {
    pre: Vec<u8>,
    pos: usize,
    stream: TcpStream,
}

impl Prepend {
    pub fn new(pre: Vec<u8>, stream: TcpStream) -> Self {
        Self { pre, pos: 0, stream }
    }
}

impl AsyncRead for Prepend {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if self.pos < self.pre.len() {
            let n = std::cmp::min(buf.remaining(), self.pre.len() - self.pos);
            let dst = buf.initialize_unfilled();
            let n = n.min(dst.len());
            dst[..n].copy_from_slice(&self.pre[self.pos..self.pos + n]);
            buf.advance(n);
            self.pos += n;
            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.stream).poll_read(cx, buf)
        }
    }
}

impl AsyncWrite for Prepend {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
