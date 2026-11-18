use super::{callback, exchange, AppKeys};
use qobuz_models::UserSession;
use tokio::{net::TcpListener, sync::oneshot, task::AbortHandle};

pub(super) struct BrowserAttempt {
  pub address: String,
  completion: oneshot::Receiver<Result<UserSession, String>>,
  cancellation: AbortHandle,
}

impl Drop for BrowserAttempt {
  fn drop(&mut self) {
    self.cancellation.abort();
  }
}

impl BrowserAttempt {
  pub async fn launch(http: reqwest::Client, keys: AppKeys) -> Result<Self, String> {
    keys.check()?;
    let socket = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
      .await
      .map_err(|_| "Cannot start the authentication callback")?;
    let port = socket.local_addr().map_err(|_| "Cannot read callback address")?.port();
    let route = format!("/{}", uuid::Uuid::new_v4());
    let mut destination =
      url::Url::parse("https://www.qobuz.com/signin/oauth").expect("static authorization URL");
    destination
      .query_pairs_mut()
      .append_pair("ext_app_id", &keys.app_id)
      .append_pair("redirect_url", &format!("http://127.0.0.1:{port}{route}"));
    let (sender, completion) = oneshot::channel();
    let cancellation = tokio::spawn(async move {
      let result = async {
        let code = tokio::time::timeout(
          std::time::Duration::from_secs(300),
          callback::listen(socket, route),
        )
        .await
        .map_err(|_| "Authentication timed out. Start again.")??;
        exchange::redeem(&http, &keys, code).await
      }
      .await;
      let _ = sender.send(result);
    })
    .abort_handle();
    Ok(Self { address: destination.into(), completion, cancellation })
  }

  pub fn poll(&mut self) -> Option<Result<UserSession, String>> {
    match self.completion.try_recv() {
      Ok(result) => Some(result),
      Err(oneshot::error::TryRecvError::Empty) => None,
      Err(oneshot::error::TryRecvError::Closed) => {
        Some(Err("Authentication was interrupted".into()))
      }
    }
  }
}
