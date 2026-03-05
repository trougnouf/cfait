use cfait::model::parser::parse_session_input;
use chrono::Local;

#[test]
fn test_parse_session_duration_only() {
    let now = Local::now();
    let session = parse_session_input("30m").expect("Failed to parse 30m");

    // The logic creates a block ending at 'now'
    let dur = session.end - session.start;
    assert_eq!(dur, 30 * 60, "Duration should be 30 minutes");

    // Because 'now' changes during execution, allow a small delta for `end`
    let end_diff = now.timestamp() - session.end;
    assert!(end_diff.abs() < 5, "End time should be approximately now");
}

#[test]
fn test_parse_session_date_and_duration() {
    let now = Local::now();
    let yesterday = now.date_naive() - chrono::Duration::days(1);

    let session = parse_session_input("yesterday 1h").expect("Failed to parse yesterday 1h");

    let dur = session.end - session.start;
    assert_eq!(dur, 60 * 60, "Duration should be 1 hour");

    // When date is not today, it defaults to 12:00
    let start_dt = chrono::DateTime::from_timestamp(session.start, 0)
        .unwrap()
        .with_timezone(&Local);

    assert_eq!(start_dt.date_naive(), yesterday);
    use chrono::Timelike;
    assert_eq!(start_dt.hour(), 12);
    assert_eq!(start_dt.minute(), 0);
}

#[test]
fn test_parse_session_time_range() {
    let now = Local::now();

    let session = parse_session_input("14:00-15:30").expect("Failed to parse range");

    let dur = session.end - session.start;
    assert_eq!(dur, 90 * 60, "Duration should be 90 minutes");

    let start_dt = chrono::DateTime::from_timestamp(session.start, 0)
        .unwrap()
        .with_timezone(&Local);

    assert_eq!(start_dt.date_naive(), now.date_naive());
    use chrono::Timelike;
    assert_eq!(start_dt.hour(), 14);
    assert_eq!(start_dt.minute(), 0);
}

#[test]
fn test_parse_session_cross_midnight_range() {
    // 23:00 to 01:00 should auto-advance the end date by 1 day
    let session = parse_session_input("23:00-01:00").expect("Failed to parse midnight range");
    let dur = session.end - session.start;
    assert_eq!(dur, 120 * 60, "Duration should be 2 hours");
}

#[test]
fn test_parse_session_date_and_range() {
    let session = parse_session_input("2024-01-01 10:00-11:00").expect("Failed");

    let start_dt = chrono::DateTime::from_timestamp(session.start, 0)
        .unwrap()
        .with_timezone(&Local);

    assert_eq!(
        start_dt.date_naive(),
        chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
    );
    use chrono::Timelike;
    assert_eq!(start_dt.hour(), 10);
    assert_eq!(start_dt.minute(), 0);
}
