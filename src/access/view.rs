use super::AppKeys;
use serde_json::{json, Value};

pub(super) struct FormSnapshot<'a> {
  pub language: &'a str,
  pub revision: u64,
  pub keys: &'a AppKeys,
  pub visible: [bool; 3],
  pub account: Option<&'a str>,
  pub browser: Option<&'a str>,
  pub notice: Option<&'a str>,
  pub editing: bool,
}

#[derive(Clone, Copy)]
enum Screen {
  Credentials,
  Account,
  Browser,
  Connected,
}
impl Screen {
  fn select(snapshot: &FormSnapshot<'_>) -> Self {
    if snapshot.account.is_some() {
      Self::Connected
    } else if snapshot.browser.is_some() {
      Self::Browser
    } else if !snapshot.editing && snapshot.keys.check().is_ok() {
      Self::Account
    } else {
      Self::Credentials
    }
  }
  fn identifier(self) -> &'static str {
    match self {
      Self::Credentials => "credentials",
      Self::Account => "account",
      Self::Browser => "browser",
      Self::Connected => "done",
    }
  }
  fn step(self) -> u8 {
    match self {
      Self::Credentials => 1,
      Self::Account | Self::Browser => 2,
      Self::Connected => 3,
    }
  }
}

struct FormPage<'a> {
  snapshot: FormSnapshot<'a>,
  screen: Screen,
  fields: Vec<Value>,
  actions: Vec<Value>,
  links: Vec<Value>,
  values: Value,
}

impl<'a> FormPage<'a> {
  fn text(&self, english: &str, spanish: &str) -> String {
    if self.snapshot.language.starts_with("es") { spanish } else { english }.to_owned()
  }
  fn action(&mut self, key: &str, english: &str, spanish: &str, fields: &[&str]) {
    let mut action = json!({"key":key,"label":self.text(english,spanish),"fields":fields});
    if key == "sign_out" {
      action["destructive"] = json!("account");
    }
    self.actions.push(action);
  }
  fn credential_fields(&mut self) {
    let keys = ["app_id", "app_secret", "private_key"];
    let entries = [
      (&self.snapshot.keys.app_id, "App ID"),
      (&self.snapshot.keys.app_secret, "App Secret"),
      (&self.snapshot.keys.private_key, "Private Key"),
    ];
    for (position, (value, label)) in entries.into_iter().enumerate() {
      let visible = self.snapshot.visible[position];
      self.fields.push(json!({"key":keys[position],"label":label,"description":"","required":true,
        "control":{"type":if visible {"text"} else {"secret"},"max_length":512}}));
      self.values[keys[position]] = json!(value);
      let verb = if visible { self.text("Hide", "Ocultar") } else { self.text("Show", "Mostrar") };
      self.actions.push(json!({"key":format!("visibility_{position}"),"label":format!("{verb} {label}"),"fields":keys}));
    }
    self.action("fetch_credentials", "Fetch credentials", "Obtener credenciales", &[]);
    self.action("sign_in", "Continue: authorize account", "Continuar: autorizar cuenta", &keys);
  }
  fn compose(mut self) -> Value {
    let (label, content) = match self.screen {
      Screen::Connected => {
        self.action("sign_out", "Sign out", "Salir", &[]);
        (
          self.text("Ready: Qobuz is connected", "Listo: Qobuz está conectado"),
          format!(
            "{} {}",
            self.text("Signed in as", "Conectado como"),
            self.snapshot.account.unwrap_or_default()
          ),
        )
      }
      Screen::Browser => {
        self.links.push(
          json!({"label":self.text("Open Qobuz", "Abrir Qobuz"),"url":self.snapshot.browser}),
        );
        self.action("check_sign_in", "Check sign-in", "Comprobar acceso", &[]);
        self.action("cancel_sign_in", "Cancel", "Cancelar", &[]);
        (self.text("Step 2 of 3: authorize your account", "Paso 2 de 3: autoriza tu cuenta"),
          self.text("Open Qobuz in your browser, then check sign-in here. This attempt expires in five minutes.", "Abre Qobuz en el navegador y después comprueba el acceso aquí. Este intento caduca en cinco minutos."))
      }
      Screen::Account => {
        self.action("sign_in", "Continue: authorize account", "Continuar: autorizar cuenta", &[]);
        self.action(
          "configure_credentials",
          "Back to app credentials",
          "Volver a las credenciales",
          &[],
        );
        (self.text("Step 2 of 3: connect your account", "Paso 2 de 3: conecta tu cuenta"),
          self.text("App credentials are ready. Continue to open Qobuz authorization, then return here. Your password stays in the browser and this wizard advances automatically.", "Las credenciales están listas. Continúa para abrir la autorización de Qobuz y vuelve aquí. Tu contraseña se queda en el navegador y el asistente avanza automáticamente."))
      }
      Screen::Credentials => {
        self.credential_fields();
        (self.text("Step 1 of 3: prepare app access", "Paso 1 de 3: prepara el acceso"),
          self.text("Fetch app credentials or enter them, then sign in using your browser. Your account password stays in the browser.", "Obtén las credenciales de la aplicación o introdúcelas y entra desde el navegador. La contraseña de tu cuenta se queda en el navegador."))
      }
    };
    let waiting = matches!(self.screen, Screen::Browser);
    let step = self.screen.step();
    json!({"revision":self.snapshot.revision,"wizard":{"step":step,"total":3,"id":self.screen.identifier(),"waiting":waiting},
      "progress":{"done":if matches!(self.screen, Screen::Connected) {3} else {step-1},"total":3},
      "status":{"phase":if waiting {"pending"} else {"ready"},"label":label},
      "form":{"fields":self.fields,"actions":self.actions,"unsupported":[]},"values":self.values,
      "content":[{"text":content}],"links":self.links,"notices":self.snapshot.notice.into_iter().collect::<Vec<_>>()})
  }
}

pub(super) fn render(snapshot: FormSnapshot<'_>) -> Value {
  FormPage {
    screen: Screen::select(&snapshot),
    snapshot,
    fields: vec![],
    actions: vec![],
    links: vec![],
    values: json!({}),
  }
  .compose()
}

#[cfg(test)]
#[path = "../checks/account_view.rs"]
mod checks;
