use super::*;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
};

fn fixture_keys() -> AppKeys {
  AppKeys {
    app_id: "123456789".into(),
    app_secret: "0123456789abcdef0123456789abcdef".into(),
    private_key: "fixturekey".into(),
  }
}
fn client() -> QobuzClient {
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
  QobuzClient::new().unwrap()
}

#[test]
fn test_callback_table() {
  for (request, accepted) in [
    ("GET /nonce?code_autorisation=a%2Bb HTTP/1.1", true),
    ("GET /nonce?code=fixture HTTP/1.1", true),
    ("GET /other?code=fixture HTTP/1.1", false),
    ("GET /nonce?code=a&code_autorisation=b HTTP/1.1", false),
    ("POST /nonce?code=a HTTP/1.1", false),
    ("GET //host/nonce?code=a HTTP/1.1", false),
    ("GET /nonce?code= HTTP/1.1", false),
  ] {
    assert_eq!(callback::authorization_code(request, "/nonce").is_ok(), accepted, "{request}");
  }
}

#[test]
fn test_resource_and_zone_table() {
  use base64::Engine;
  assert!(keys::resource_address(r#"<script src="https://elsewhere.test/bundle.js">"#).is_err());
  assert_eq!(
    keys::resource_address(r#"<script src="/resources/8.2.0-b034/bundle.js">"#).unwrap().host_str(),
    Some("play.qobuz.com")
  );
  let encoded = base64::engine::general_purpose::STANDARD.encode(&fixture_keys().app_secret);
  for (zone, accepted) in [("Berlin", true), ("berlin", true), ("Paris", false)] {
    let bundle = format!(
      r#"production:{{api:{{appId:"123456789"}}}} privateKey:"fixturekey" a.initialSeed("{}",window.utimezone.berlin) name:"Europe/{zone}",info:"{}",extras:"{}""#,
      &encoded[..20],
      &encoded[20..],
      "x".repeat(44)
    );
    assert_eq!(keys::decode_bundle(&bundle).is_ok(), accepted);
  }
}

#[tokio::test]
async fn test_callback_keeps_waiting_after_unrelated_request() {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  let receiving = tokio::spawn(callback::listen(listener, "/nonce".into()));
  for (target, status) in [("/favicon.ico", "400"), ("/nonce?code=fixture", "200")] {
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket
      .write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
      .await
      .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with(&format!("HTTP/1.1 {status}")));
  }
  assert_eq!(receiving.await.unwrap().unwrap(), "fixture");
}

#[tokio::test]
async fn test_cancel_releases_attempt_listener() {
  let _ = client();
  let attempt = BrowserAttempt::launch(reqwest::Client::new(), fixture_keys()).await.unwrap();
  let address = url::Url::parse(&attempt.address)
    .unwrap()
    .query_pairs()
    .find(|(name, _)| name == "redirect_url")
    .unwrap()
    .1
    .into_owned();
  let address = url::Url::parse(&address).unwrap();
  let port = address.port().unwrap();
  drop(attempt);
  tokio::time::timeout(std::time::Duration::from_secs(1), async {
    loop {
      if TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await.is_err() {
        break;
      }
      tokio::task::yield_now().await;
    }
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn test_account_migration_and_private_persistence() {
  let api = client();
  let folder = tempfile::tempdir().unwrap();
  let session = UserSession {
    user_auth_token: "fixture-token".into(),
    user_id: 42,
    display_name: "Fixture".into(),
    ..Default::default()
  };
  files::store_json(
    &folder.path().join("account.json"),
    &AccountFile { credentials: fixture_keys(), session: Some(session) },
  )
  .unwrap();
  let controller = AccessController::new(folder.path());
  controller.load(&api).await.unwrap();
  assert_eq!(controller.inspect(&api).await["signed_in"], true);
  assert!(!controller.form(&api, "en").await.to_string().contains("fixture-token"));
  controller.sign_out(&api).await.unwrap();
  assert!(!std::fs::read_to_string(folder.path().join("account.json"))
    .unwrap()
    .contains("fixture-token"));
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
      std::fs::metadata(folder.path().join("account.json")).unwrap().permissions().mode() & 0o777,
      0o600
    );
  }
}

#[tokio::test]
async fn test_token_only_exchange_profile_table() {
  let _ = client();
  for (status, profile, identity) in [
    (200, json!({"id":42}), Some(42)),
    (200, json!({"user":{"id":"43"}}), Some(43)),
    (200, json!({"id":0}), None),
    (401, json!({"private":"fixture-private-body"}), None),
  ] {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}/profile", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
      let (mut socket, _) = listener.accept().await.unwrap();
      let mut bytes = [0; 4096];
      let count = socket.read(&mut bytes).await.unwrap();
      let request = String::from_utf8_lossy(&bytes[..count]).to_ascii_lowercase();
      assert!(request.contains("x-user-auth-token: fixture-token"));
      let body = profile.to_string();
      socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    });
    let result = exchange::resolve_identity(
      &reqwest::Client::new(),
      "123",
      json!({"token":"fixture-token"}),
      &address,
    )
    .await;
    server.await.unwrap();
    assert_eq!(result.as_ref().ok().map(|session| session.user_id), identity);
    if let Err(error) = result {
      assert!(!error.contains("fixture-private-body"));
    }
  }
}
