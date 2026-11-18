use super::AppKeys;
use qobuz_models::UserSession;
use serde_json::Value;

fn identity(body: &Value) -> Option<u64> {
  ["/userId", "/user_id", "/id", "/user/id"]
    .into_iter()
    .filter_map(|path| body.pointer(path))
    .find_map(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()).filter(|id| *id > 0))
}

pub(super) async fn resolve_identity(
  http: &reqwest::Client,
  app: &str,
  body: Value,
  profile: &str,
) -> Result<UserSession, String> {
  let token = body
    .get("token")
    .or_else(|| body.get("user_auth_token"))
    .and_then(Value::as_str)
    .filter(|token| !token.is_empty())
    .ok_or("Authentication response has no token")?;
  reqwest::header::HeaderValue::from_str(token)
    .map_err(|_| "Authentication response has an invalid token")?;
  let id = match identity(&body) {
    Some(id) => id,
    None => {
      let response = http
        .get(profile)
        .header("X-App-Id", app)
        .header("X-User-Auth-Token", token)
        .send()
        .await
        .map_err(|_| "Cannot retrieve the signed-in account. Sign in again.")?;
      if !response.status().is_success() {
        return Err(format!(
          "Account verification returned HTTP {}. Sign in again.",
          response.status().as_u16()
        ));
      }
      let record: Value = response.json().await.map_err(|_| "Invalid account profile response")?;
      identity(&record).ok_or("The account profile has no valid user ID")?
    }
  };
  Ok(UserSession {
    user_auth_token: token.to_owned(),
    user_id: id,
    display_name: format!("Account {id}"),
    ..Default::default()
  })
}

pub(super) async fn redeem(
  http: &reqwest::Client,
  keys: &AppKeys,
  code: String,
) -> Result<UserSession, String> {
  let response = http
    .get("https://www.qobuz.com/api.json/0.2/oauth/callback")
    .header("X-App-Id", &keys.app_id)
    .query(&[("code", code.as_str()), ("private_key", keys.private_key.as_str())])
    .send()
    .await
    .map_err(|_| "Cannot exchange the authorization code")?;
  if !response.status().is_success() {
    return Err(format!("Authentication returned HTTP {}", response.status().as_u16()));
  }
  let payload = response.json().await.map_err(|_| "Invalid authentication response")?;
  resolve_identity(http, &keys.app_id, payload, "https://www.qobuz.com/api.json/0.2/user/get").await
}
