use base64::{engine::general_purpose::STANDARD, Engine};
use regex::Regex;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Default, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct AppKeys {
  pub app_id: String,
  pub app_secret: String,
  pub private_key: String,
}

impl AppKeys {
  pub fn check(&self) -> Result<(), String> {
    let rules: [(&str, bool); 3] = [
      (
        "Enter a valid App ID",
        !self.app_id.is_empty()
          && self.app_id.len() <= 32
          && self.app_id.bytes().all(|byte| byte.is_ascii_digit()),
      ),
      (
        "Enter a valid App Secret",
        self.app_secret.len() == 32 && self.app_secret.bytes().all(|byte| byte.is_ascii_hexdigit()),
      ),
      (
        "Enter a valid Private Key",
        !self.private_key.is_empty()
          && self.private_key.len() <= 256
          && self.private_key.bytes().all(|byte| byte.is_ascii_alphanumeric()),
      ),
    ];
    rules
      .into_iter()
      .find_map(|(message, valid)| (!valid).then_some(message))
      .map_or(Ok(()), |message| Err(message.to_owned()))
  }

  pub fn edit(&mut self, submitted: &serde_json::Value) -> Result<(), String> {
    for (name, destination) in [
      ("app_id", &mut self.app_id),
      ("app_secret", &mut self.app_secret),
      ("private_key", &mut self.private_key),
    ] {
      if let Some(value) = submitted.get(name).and_then(serde_json::Value::as_str) {
        if value.len() > 512 {
          return Err("Credential is too long".into());
        }
        destination.clear();
        destination.push_str(value.trim());
      }
    }
    Ok(())
  }
}

fn expression(pattern: &str) -> Regex {
  Regex::new(pattern).expect("static credential pattern")
}

fn first_capture<'a>(pattern: &str, text: &'a str) -> Result<&'a str, String> {
  expression(pattern)
    .captures(text)
    .and_then(|capture| capture.get(1))
    .map(|capture| capture.as_str())
    .ok_or_else(|| "The web player credential format has changed".into())
}

pub(super) fn resource_address(html: &str) -> Result<url::Url, String> {
  let relative =
    first_capture(r#"<script\b[^>]*\bsrc=["'](/resources/[\d.]+-[a-z]\d+/bundle\.js)["']"#, html)?;
  url::Url::parse("https://play.qobuz.com")
    .expect("static origin")
    .join(relative)
    .map_err(|_| "Invalid web player address".into())
}

pub(super) fn decode_bundle(bundle: &str) -> Result<AppKeys, String> {
  let app = first_capture(r#"production:\{api:\{appId:"(\d+)""#, bundle)?;
  let private = first_capture(r#"privateKey:"([A-Za-z0-9]+)""#, bundle)?;
  let seeds = expression(r#"[a-z]\.initialSeed\("([\w=]+)",window\.utimezone\.([a-z]+)\)"#);
  let fragments = expression(r#"name:"\w+/(\w+)",info:"([\w=]+)",extras:"([\w=]+)""#);
  let mut zones: Vec<_> = seeds.captures_iter(bundle).collect();
  zones.sort_by_key(|capture| {
    ["berlin", "london", "abidjan"].iter().position(|zone| *zone == &capture[2]).unwrap_or(3)
  });
  for zone in zones {
    for fragment in
      fragments.captures_iter(bundle).filter(|fragment| fragment[1].eq_ignore_ascii_case(&zone[2]))
    {
      let joined = Zeroizing::new([&zone[1], &fragment[2], &fragment[3]].concat());
      let Some(encoded) = joined.len().checked_sub(44).and_then(|length| joined.get(..length))
      else {
        continue;
      };
      let Ok(raw) = STANDARD.decode(encoded) else {
        continue;
      };
      let Ok(secret) = String::from_utf8(raw) else {
        continue;
      };
      let candidate =
        AppKeys { app_id: app.to_owned(), app_secret: secret, private_key: private.to_owned() };
      if candidate.check().is_ok() {
        return Ok(candidate);
      }
    }
  }
  Err("No usable app credentials were found in the web player".into())
}

async fn bounded_text(
  http: &reqwest::Client,
  address: &str,
  capacity: usize,
) -> Result<String, String> {
  let mut reply = http.get(address).send().await.map_err(|_| "Cannot reach the web player")?;
  if !reply.status().is_success() {
    return Err(format!("Web player returned HTTP {}", reply.status().as_u16()));
  }
  let mut payload = Vec::new();
  while let Some(chunk) = reply.chunk().await.map_err(|_| "Cannot read the web player")? {
    if chunk.len() > capacity.saturating_sub(payload.len()) {
      return Err("Web player response is too large".into());
    }
    payload.extend_from_slice(&chunk);
  }
  String::from_utf8(payload).map_err(|_| "Invalid web player response".into())
}

pub(super) async fn discover(http: &reqwest::Client) -> Result<AppKeys, String> {
  let login = bounded_text(http, "https://play.qobuz.com/login", 2 * 1024 * 1024).await?;
  let address = resource_address(&login)?;
  let javascript = Zeroizing::new(bounded_text(http, address.as_str(), 24 * 1024 * 1024).await?);
  decode_bundle(&javascript)
}
