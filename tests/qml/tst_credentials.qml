import QtQuick
import QtTest
import "@HOST_PATTERNS@"

TestCase {
  name: "PluginCredentialVisibility"
  when: windowShown
  width: 640
  height: 640
  visible: true
  PluginSettingsForm {
    id: form
    width: 600
    document: ({})
  }
  SignalSpy { id: submissions; target: form; signalName: "submitted" }
  function inputs(item, found) {
    for (let child of item.children) {
      if (child.echoMode !== undefined) found.push(child)
      inputs(child, found)
    }
  }
  function buttons(item, found) {
    for (let child of item.children) {
      if (child.text !== undefined && child.clicked !== undefined) found.push(child)
      buttons(child, found)
    }
  }
  function test_masked_fields_and_eye_roundtrip() {
    const keys = ["app_id", "app_secret", "private_key"]
    form.document = {
      revision: 1,
      values: {app_id:"123456789",app_secret:"fixture-secret",private_key:"fixture-key"},
      form: {
        fields: keys.map(key => ({key:key,label:key,control:{type:"secret"}})),
        actions: [{key:"visibility_1",label:"Show App Secret",fields:keys}]
      }
    }
    wait(20)
    let fields = []; inputs(form, fields)
    compare(fields.length, 3)
    for (let field of fields) compare(field.echoMode, TextInput.Password)
    let actions = []; buttons(form, actions)
    compare(actions.length, 1)
    actions[0].clicked()
    compare(submissions.count, 1)
    compare(submissions.signalArguments[0][0], "visibility_1")
    compare(submissions.signalArguments[0][1].app_secret, "fixture-secret")
    const next = JSON.parse(JSON.stringify(form.document))
    next.revision = 2
    next.form.fields[1].control.type = "text"
    form.document = next
    wait(20)
    fields = []; inputs(form, fields)
    compare(fields[0].echoMode, TextInput.Password)
    compare(fields[1].echoMode, TextInput.Normal)
    compare(fields[2].echoMode, TextInput.Password)
    next.form.fields[1].control.type = "secret"
    next.revision = 3
    form.document = JSON.parse(JSON.stringify(next))
    wait(20)
    fields = []; inputs(form, fields)
    for (let field of fields) compare(field.echoMode, TextInput.Password)
  }
}
