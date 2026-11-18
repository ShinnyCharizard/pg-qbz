fn form_case(
  language: &str,
  revision: u64,
  keys: &AppKeys,
  visible: [bool; 3],
  account: Option<&str>,
  browser: Option<&str>,
  notice: Option<&str>,
  editing: bool,
) -> Value {
  render(FormSnapshot { language, revision, keys, visible, account, browser, notice, editing })
}
use super::*;
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn test_field_mask() {
    let credentials = AppKeys::default();
    let masked = form_case("es", 1, &credentials, [false; 3], None, None, None, false);
    assert!(masked["form"]["fields"]
      .as_array()
      .unwrap()
      .iter()
      .all(|f| f["control"]["type"] == "secret"));
    let shown = form_case("es", 2, &credentials, [false, true, false], None, None, None, false);
    assert_eq!(shown["form"]["fields"][1]["control"]["type"], "text");
    assert_eq!(shown["form"]["fields"][0]["control"]["type"], "secret");
    let pending = form_case(
      "es",
      3,
      &credentials,
      [true; 3],
      None,
      Some("https://www.qobuz.com/signin/oauth"),
      None,
      false,
    );
    assert_eq!(pending["values"], json!({}));
    assert!(pending["form"]["fields"].as_array().unwrap().is_empty());
  }
  #[test]
  fn test_wizard_steps() {
    let credentials = AppKeys {
      app_id: "123456789".into(),
      app_secret: "0".repeat(32),
      private_key: "fixturekey".into(),
    };
    let prepared = form_case("en", 1, &credentials, [false; 3], None, None, None, false);
    assert_eq!(prepared["wizard"]["step"], 2);
    assert!(prepared["form"]["fields"].as_array().unwrap().is_empty());
    assert_eq!(prepared["values"], json!({}));
    assert_eq!(prepared["status"]["phase"], "ready");
    let browser = form_case(
      "en",
      2,
      &credentials,
      [false; 3],
      None,
      Some("https://example.org/authorize"),
      None,
      false,
    );
    assert_eq!(browser["wizard"]["waiting"], true);
    assert_eq!(browser["status"]["phase"], "pending");
    let connected =
      form_case("en", 3, &credentials, [false; 3], Some("Fixture"), None, None, false);
    assert_eq!(connected["wizard"]["id"], "done");
    assert_eq!(connected["progress"]["done"], 3);
    assert_eq!(connected["values"], json!({}));
    let edit = form_case("en", 4, &credentials, [false; 3], None, None, None, true);
    assert_eq!(edit["wizard"]["step"], 1);
    assert!(edit["form"]["fields"]
      .as_array()
      .unwrap()
      .iter()
      .all(|f| f["control"]["type"] == "secret"));
  }
}
