mod attempt;
mod callback;
mod exchange;
mod keys;
mod view;

use crate::files;
use attempt::BrowserAttempt;
pub use keys::AppKeys;
use qobuz_api::QobuzClient;
use qobuz_models::UserSession;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

#[derive(Default, Serialize, Deserialize)]
struct AccountFile {
  credentials: AppKeys,
  session: Option<UserSession>,
}

enum Identity {
  Empty,
  Browser(BrowserAttempt),
  Linked(UserSession),
}

struct AccessState {
  keys: AppKeys,
  identity: Identity,
  revealed: [bool; 3],
  editing: bool,
  revision: u64,
  notice: Option<String>,
}

impl Default for AccessState {
  fn default() -> Self {
    Self {
      keys: AppKeys::default(),
      identity: Identity::Empty,
      revealed: [false; 3],
      editing: false,
      revision: 0,
      notice: None,
    }
  }
}

impl AccessState {
  fn session(&self) -> Option<&UserSession> {
    match &self.identity {
      Identity::Linked(session) => Some(session),
      _ => None,
    }
  }

  fn checkpoint(&self, file: &Path) -> Result<(), String> {
    files::store_json(
      file,
      &AccountFile { credentials: self.keys.clone(), session: self.session().cloned() },
    )
  }

  fn page(&mut self, language: &str) -> Value {
    self.revision += 1;
    let pending = match &self.identity {
      Identity::Browser(attempt) => Some(attempt.address.as_str()),
      _ => None,
    };
    view::render(view::FormSnapshot {
      language,
      revision: self.revision,
      keys: &self.keys,
      visible: self.revealed,
      account: self.session().map(|session| session.display_name.as_str()),
      browser: pending,
      notice: self.notice.as_deref(),
      editing: self.editing,
    })
  }
}

enum Command {
  Disconnect,
  Poll,
  Cancel,
  Configure,
  Reveal(usize),
  Discover,
  Authorize,
  Unknown,
}

impl From<&str> for Command {
  fn from(name: &str) -> Self {
    match name {
      "sign_out" => Self::Disconnect,
      "check_sign_in" => Self::Poll,
      "cancel_sign_in" => Self::Cancel,
      "configure_credentials" => Self::Configure,
      "fetch_credentials" => Self::Discover,
      "sign_in" => Self::Authorize,
      other => other
        .strip_prefix("visibility_")
        .and_then(|suffix| suffix.parse().ok())
        .filter(|index| *index < 3)
        .map_or(Self::Unknown, Self::Reveal),
    }
  }
}

pub struct AccessController {
  file: PathBuf,
  state: Mutex<AccessState>,
  network: reqwest::Client,
}

impl AccessController {
  pub fn new(directory: &Path) -> Self {
    let network = reqwest::Client::builder()
      .user_agent("Qobuz-Source/1.0")
      .redirect(reqwest::redirect::Policy::none())
      .connect_timeout(std::time::Duration::from_secs(10))
      .timeout(std::time::Duration::from_secs(25))
      .build()
      .expect("account transport");
    Self {
      file: directory.join("account.json"),
      state: Mutex::new(AccessState::default()),
      network,
    }
  }

  pub async fn load(&self, api: &QobuzClient) -> Result<(), String> {
    let mut state = self.state.lock().await;
    let record = match files::load_json::<AccountFile>(&self.file) {
      Ok(Some(record)) => record,
      Ok(None) => return Ok(()),
      Err(error) => {
        state.notice = Some(error);
        return Ok(());
      }
    };
    state.keys = record.credentials;
    if state.keys.app_id.is_empty() && record.session.is_none() {
      return Ok(());
    }
    if let Err(error) = state.keys.check() {
      state.identity = Identity::Empty;
      state.notice = Some(error);
      return Ok(());
    }
    api
      .configure(state.keys.app_id.clone(), state.keys.app_secret.clone())
      .await
      .map_err(|_| "Invalid saved credentials")?;
    if let Some(session) = record.session {
      if session.user_id == 0
        || session.user_auth_token.is_empty()
        || reqwest::header::HeaderValue::from_str(&session.user_auth_token).is_err()
      {
        state.notice = Some("The saved session is invalid. Sign in again.".into());
      } else {
        api.set_session(session.clone()).await;
        state.identity = Identity::Linked(session);
      }
    }
    Ok(())
  }

  async fn settle(&self, state: &mut AccessState, api: &QobuzClient) {
    let outcome = match &mut state.identity {
      Identity::Browser(attempt) => attempt.poll(),
      _ => None,
    };
    let Some(outcome) = outcome else {
      return;
    };
    state.identity = Identity::Empty;
    state.revealed.fill(false);
    match outcome {
      Err(error) => state.notice = Some(error),
      Ok(session) => {
        state.identity = Identity::Linked(session.clone());
        match state.checkpoint(&self.file) {
          Ok(()) => {
            api.set_session(session).await;
            state.notice = None;
          }
          Err(error) => {
            state.identity = Identity::Empty;
            state.notice = Some(error);
          }
        }
      }
    }
  }

  async fn disconnect(&self, state: &mut AccessState, api: &QobuzClient) -> Result<(), String> {
    state.identity = Identity::Empty;
    state.revealed.fill(false);
    api.logout().await;
    state.checkpoint(&self.file)
  }

  pub async fn inspect(&self, api: &QobuzClient) -> Value {
    let mut state = self.state.lock().await;
    self.settle(&mut state, api).await;
    json!({"signed_in": state.session().is_some(), "user": state.session().map(|session| &session.display_name),
      "pending": matches!(state.identity, Identity::Browser(_)), "notice": state.notice})
  }

  pub async fn sign_out(&self, api: &QobuzClient) -> Result<(), String> {
    self.disconnect(&mut *self.state.lock().await, api).await
  }

  pub async fn form(&self, api: &QobuzClient, language: &str) -> Value {
    let mut state = self.state.lock().await;
    self.settle(&mut state, api).await;
    state.revealed.fill(false);
    state.page(language)
  }

  pub async fn apply(&self, api: &QobuzClient, language: &str, payload: Value) -> Value {
    let mut state = self.state.lock().await;
    if payload.get("revision").and_then(Value::as_u64) != Some(state.revision) {
      state.revealed.fill(false);
      state.notice = Some("This settings page changed. Try again.".into());
      return state.page(language);
    }
    state.notice = None;
    self.settle(&mut state, api).await;
    let command = Command::from(payload["action"].as_str().unwrap_or_default());
    if let Err(error) = self.execute(&mut state, api, command, &payload["values"]).await {
      state.notice = Some(error);
      state.revealed.fill(false);
    }
    state.page(language)
  }

  async fn execute(
    &self,
    state: &mut AccessState,
    api: &QobuzClient,
    command: Command,
    values: &Value,
  ) -> Result<(), String> {
    match command {
      Command::Disconnect => return self.disconnect(state, api).await,
      Command::Poll => return Ok(()),
      Command::Cancel => {
        if matches!(state.identity, Identity::Browser(_)) {
          state.identity = Identity::Empty;
        }
        state.revealed.fill(false);
        return Ok(());
      }
      _ => (),
    }
    if !matches!(state.identity, Identity::Empty) {
      return Err("Cancel or sign out before changing credentials".into());
    }
    if matches!(command, Command::Configure) {
      state.editing = true;
      state.revealed.fill(false);
      return Ok(());
    }
    state.keys.edit(values)?;
    if let Command::Reveal(index) = command {
      state.editing = true;
      state.revealed[index] = !state.revealed[index];
      return Ok(());
    }
    state.revealed.fill(false);
    match command {
      Command::Discover => {
        state.keys = keys::discover(&self.network).await?;
        state.editing = false;
        state.checkpoint(&self.file)
      }
      Command::Authorize => {
        state.keys.check()?;
        state.editing = false;
        state.checkpoint(&self.file)?;
        api
          .configure(state.keys.app_id.clone(), state.keys.app_secret.clone())
          .await
          .map_err(|_| "Invalid app credentials")?;
        state.identity = Identity::Browser(
          BrowserAttempt::launch(self.network.clone(), state.keys.clone()).await?,
        );
        Ok(())
      }
      _ => Err("Unknown account action".into()),
    }
  }
}

#[cfg(test)]
#[path = "../checks/access.rs"]
mod checks;
