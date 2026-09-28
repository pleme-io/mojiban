use mojiban::{InlineKind, Role, TextStyle, render_markdown};

#[test]
fn roles_come_from_the_inline_kind_not_the_colour() {
    let doc = render_markdown("`c` [l](u) $x$ plain", 80, &mojiban::Theme::default());
    let roles: Vec<Role> = doc.styles.iter().map(|s| s.role).collect();
    for r in [Role::InlineCode, Role::Link, Role::Math, Role::Text] {
        assert!(roles.contains(&r), "missing {r:?} in {roles:?}");
    }
}

#[test]
fn plain_kind_is_omitted_on_the_wire() {
    let json = serde_json::to_string(&TextStyle::default()).unwrap();
    assert!(!json.contains("kind"), "{json}");
    let code = TextStyle { kind: InlineKind::Code, ..TextStyle::default() };
    let back: TextStyle = serde_json::from_str(&serde_json::to_string(&code).unwrap()).unwrap();
    assert_eq!(back.kind, InlineKind::Code);
    let legacy: TextStyle = serde_json::from_str(r#"{"italic":true}"#).unwrap();
    assert!(legacy.kind.is_plain());
}
