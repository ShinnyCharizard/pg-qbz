use std::time::Duration;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
};

pub(super) fn authorization_code(request: &str, route: &str) -> Result<String, String> {
  let first = request.lines().next().unwrap_or_default();
  let words: Vec<_> = first.split_whitespace().collect();
  if words.first().copied() != Some("GET") {
    return Err("Invalid callback method".into());
  }
  let target = *words.get(1).ok_or("Missing callback address")?;
  if !target.starts_with('/') || target.starts_with("//") {
    return Err("Invalid callback address".into());
  }
  let parsed = url::Url::parse(&format!("http://127.0.0.1{target}"))
    .map_err(|_| "Invalid callback address")?;
  if parsed.path() != route {
    return Err("Unexpected callback path".into());
  }
  let mut found = None;
  for (key, value) in parsed.query_pairs() {
    if key != "code" && key != "code_autorisation" {
      continue;
    }
    if found.is_some() || value.is_empty() || value.len() > 4096 {
      return Err("Missing or ambiguous authorization code".into());
    }
    found = Some(value.into_owned());
  }
  found.ok_or_else(|| "Missing or ambiguous authorization code".into())
}

async fn read_request(socket: &mut TcpStream, route: &str) -> Result<String, String> {
  let mut request = Vec::with_capacity(1024);
  let mut block = [0; 1024];
  loop {
    if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
      break;
    }
    let count = socket.read(&mut block).await.map_err(|_| "Cannot read callback")?;
    if count == 0 {
      break;
    }
    request.extend_from_slice(&block[..count]);
    if request.len() >= 8192 {
      return Err("Callback is too large".into());
    }
  }
  authorization_code(std::str::from_utf8(&request).map_err(|_| "Invalid callback encoding")?, route)
}

pub(super) async fn listen(listener: TcpListener, route: String) -> Result<String, String> {
  loop {
    let (mut socket, peer) =
      listener.accept().await.map_err(|_| "Cannot receive authentication callback")?;
    if !peer.ip().is_loopback() {
      continue;
    }
    let result =
      tokio::time::timeout(Duration::from_secs(2), read_request(&mut socket, &route)).await;
    let Ok(Ok(code)) = result else {
      let _ = socket
        .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .await;
      continue;
    };
    let body = "Authorization received. Return to your player and press Check sign-in.";
    let answer = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    let _ = tokio::time::timeout(Duration::from_secs(2), socket.write_all(answer.as_bytes())).await;
    return Ok(code);
  }
}
