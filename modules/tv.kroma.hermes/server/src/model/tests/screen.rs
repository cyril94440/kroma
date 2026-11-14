use crate::model::screen::{parse_reading, prompt_for, ScreenReading};

#[test]
fn an_unreadable_answer_is_unknown() {
    assert_eq!(parse_reading("not json"), ScreenReading::unknown());
    assert_eq!(parse_reading(""), ScreenReading::unknown());
}

#[test]
fn a_fenced_answer_is_read_field_by_field_within_bounds() {
    let text = "```json\n{\"type\":\"DIRECT\",\"team1\":\"Lens\",\"team2\":\"PSG\",\"score\":\"1-0\",\"clock\":\"34:12\",\"confidence\":0.9,\"description\":\"x\"}\n```";
    let reading = parse_reading(text);
    assert_eq!(reading.kind, "direct");
    assert_eq!((reading.team1.as_str(), reading.team2.as_str(), reading.score.as_str()), ("Lens", "PSG", "1-0"));
    assert!(reading.is_trusted());
}

#[test]
fn an_unknown_type_becomes_other_and_confidence_is_clamped() {
    let reading = parse_reading(r#"{"type":"documentaire","confidence":7}"#);
    assert_eq!(reading.kind, "autre");
    assert_eq!(reading.confidence, 1.0);
    let low = parse_reading(r#"{"type":"direct","confidence":0.2}"#);
    assert!(!low.is_trusted());
}

#[test]
fn a_list_answer_takes_its_first_item_and_long_fields_are_cut() {
    let long = "x".repeat(300);
    let reading = parse_reading(&format!(r#"{{"items":[{{"type":"mire","answer":"{long}","confidence":0.8}}]}}"#));
    assert_eq!(reading.kind, "mire");
    assert_eq!(reading.answer.chars().count(), 200);
}

#[test]
fn the_question_is_quoted_inside_the_prompt_without_control_characters() {
    let prompt = prompt_for("quel score ?\n\u{7}");
    assert!(prompt.contains("« quel score ? »"));
    assert!(!prompt.contains('\u{7}'));
    assert_eq!(prompt_for("  "), crate::model::screen::PROMPT);
}
