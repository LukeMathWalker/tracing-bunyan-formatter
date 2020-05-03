use crate::mock_writer::MockWriter;
use lazy_static::lazy_static;
use serde_json::Value;
use std::sync::Mutex;
use tracing::{info, span, Level};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

mod mock_writer;

/// Tests have to be run on a single thread because we are re-using the same buffer for
/// all of them.
type InMemoryBuffer = Mutex<Vec<u8>>;
lazy_static! {
    static ref BUFFER: InMemoryBuffer = Mutex::new(vec![]);
}

// Run a closure and collect the output emitted by the tracing instrumentation using an in-memory buffer.
fn run_and_get_raw_output<F: Fn()>(action: F) -> String {
    let formatting_layer = BunyanFormattingLayer::new("test".into(), || MockWriter::new(&BUFFER));
    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::with_default(subscriber, action);

    // Return the formatted output as a string to make assertions against
    let mut buffer = BUFFER.try_lock().unwrap();
    let output = buffer.to_vec();
    // Clean the buffer to avoid cross-tests interactions
    buffer.clear();
    String::from_utf8(output).unwrap()
}

// Run a closure and collect the output emitted by the tracing instrumentation using
// an in-memory buffer as structured new-line-delimited JSON.
fn run_and_get_output<F: Fn()>(action: F) -> Vec<Value> {
    run_and_get_raw_output(action)
        .lines()
        .filter(|&l| !l.is_empty())
        .inspect(|l| println!("{}", l))
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

// Instrumented code to be run to test the behaviour of the tracing instrumentation.
fn test_action() {
    let a = 2;
    let span = span!(Level::DEBUG, "shaving_yaks", a);
    let _enter = span.enter();

    info!("pre-shaving yaks");
    let b = 3;
    let new_span = span!(Level::DEBUG, "inner shaving", b);
    let _enter2 = new_span.enter();

    info!("shaving yaks");
}

#[test]
fn each_line_is_valid_json() {
    let tracing_output = run_and_get_raw_output(test_action);

    // Each line is valid JSON
    for line in tracing_output.lines().filter(|&l| !l.is_empty()) {
        assert!(serde_json::from_str::<Value>(line).is_ok());
    }
}

#[test]
fn each_line_has_the_mandatory_bunyan_fields() {
    let tracing_output = run_and_get_output(test_action);

    for record in tracing_output {
        assert!(record.get("name").is_some());
        assert!(record.get("level").is_some());
        assert!(record.get("time").is_some());
        assert!(record.get("msg").is_some());
        assert!(record.get("v").is_some());
        assert!(record.get("pid").is_some());
        assert!(record.get("hostname").is_some());
    }
}

#[test]
fn time_is_formatted_according_to_rfc_3339() {
    let tracing_output = run_and_get_output(test_action);

    for record in tracing_output {
        let time = record.get("time").unwrap().as_str().unwrap();
        let parsed = chrono::DateTime::parse_from_rfc3339(time);
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap();
        assert_eq!(parsed.offset().local_minus_utc(), 0);
    }
}

#[test]
fn parent_properties_are_propagated() {
    let action = || {
        let span = span!(Level::DEBUG, "parent_span", parent_property = 2);
        let _enter = span.enter();

        let child_span = span!(Level::DEBUG, "child_span");
        let _enter_child = child_span.enter();

        info!("shaving yaks");
    };
    let tracing_output = run_and_get_output(action);

    for record in tracing_output {
        assert!(record.get("parent_property").is_some());
    }
}
