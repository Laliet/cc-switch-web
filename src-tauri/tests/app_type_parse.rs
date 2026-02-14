use std::str::FromStr;

use cc_switch_lib::AppType;

#[test]
fn parse_known_apps_case_insensitive_and_trim() {
    assert!(matches!(AppType::from_str("claude"), Ok(AppType::Claude)));
    assert!(matches!(AppType::from_str("codex"), Ok(AppType::Codex)));
    assert!(matches!(AppType::from_str("gemini"), Ok(AppType::Gemini)));
    assert!(matches!(
        AppType::from_str("opencode"),
        Ok(AppType::Opencode)
    ));
    assert!(matches!(AppType::from_str("omo"), Ok(AppType::Omo)));
    assert!(matches!(
        AppType::from_str(" ClAuDe \n"),
        Ok(AppType::Claude)
    ));
    assert!(matches!(AppType::from_str("\tcoDeX\t"), Ok(AppType::Codex)));
}

#[test]
fn parse_supported_rejects_upcoming_apps() {
    let err = AppType::parse_supported("opencode").unwrap_err();
    assert!(err.to_string().contains("暂未支持") || err.to_string().contains("not supported yet"));
}

#[test]
fn parse_unknown_app_returns_localized_error_message() {
    let err = AppType::from_str("unknown").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("可选值") || msg.contains("Allowed"));
    assert!(msg.contains("unknown"));
}
