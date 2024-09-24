use crate::mock_writer::MockWriter;
use claims::assert_some_eq;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use time::format_description::well_known::Rfc3339;
use tracing::{info, span, Level};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

mod mock_writer;

// Run a closure and collect the output emitted by the tracing instrumentation using an in-memory buffer.
fn run_and_get_raw_output<F: Fn()>(action: F) -> String {
    let buffer = Arc::new(Mutex::new(vec![]));
    let buffer_clone = buffer.clone();

    let mut default_fields = HashMap::new();
    default_fields.insert("custom_field".to_string(), json!("custom_value"));
    let skipped_fields = vec!["skipped"];
    let formatting_layer = BunyanFormattingLayer::with_default_fields(
        "test".into(),
        move || MockWriter::new(buffer_clone.clone()),
        default_fields,
    )
    .skip_fields(skipped_fields.into_iter())
    .unwrap();
    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::with_default(subscriber, action);

    // Return the formatted output as a string to make assertions against
    let buffer_guard = buffer.lock().unwrap();
    let output = buffer_guard.to_vec();
    String::from_utf8(output).unwrap()
}

// Run a closure and collect the output emitted by the tracing instrumentation using
// an in-memory buffer as structured new-line-delimited JSON.
fn run_and_get_output<F: Fn()>(action: F) -> Vec<Value> {
    run_and_get_raw_output(action)
        .lines()
        .filter(|&l| !l.trim().is_empty())
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
    let skipped = false;
    let new_span = span!(Level::DEBUG, "inner shaving", b, skipped);
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
        assert!(record.get("custom_field").is_some());
    }
}

#[test]
fn time_is_formatted_according_to_rfc_3339() {
    let tracing_output = run_and_get_output(test_action);

    for record in tracing_output {
        let time = record.get("time").unwrap().as_str().unwrap();
        let parsed = time::OffsetDateTime::parse(time, &Rfc3339);
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap();
        assert!(parsed.offset().is_utc());
    }
}

#[test]
fn encode_f64_as_numbers() {
    let f64_value: f64 = 0.5;
    let action = || {
        let span = span!(
            Level::DEBUG,
            "parent_span_f64",
            f64_field = tracing::field::Empty
        );
        let _enter = span.enter();
        span.record("f64_field", f64_value);
        info!("testing f64");
    };
    let tracing_output = run_and_get_output(action);

    for record in tracing_output {
        if record
            .get("msg")
            .and_then(Value::as_str)
            .map_or(false, |msg| msg.contains("testing f64"))
        {
            let observed_value = record.get("f64_field").and_then(|v| v.as_f64());
            assert_some_eq!(observed_value, f64_value);
        }
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

#[test]
fn elapsed_milliseconds_are_present_on_exit_span() {
    let tracing_output = run_and_get_output(test_action);

    for record in tracing_output {
        if record
            .get("msg")
            .and_then(Value::as_str)
            .map_or(false, |msg| msg.ends_with("END]"))
        {
            assert!(record.get("elapsed_milliseconds").is_some());
        }
    }
}

#[test]
fn skip_fields() {
    let tracing_output = run_and_get_output(test_action);

    for record in tracing_output {
        assert!(record.get("skipped").is_none());
    }
}

#[test]
fn set_hostname() {
    let tracing_output = run_and_get_output(test_action);
    let record = tracing_output.first().unwrap();
    let default_hostname = record.get("hostname").unwrap().to_string();

    let expected_hostname = "banana".to_string();

    let buffer = Arc::new(Mutex::new(vec![]));
    let buffer_clone = buffer.clone();

    let formatting_layer =
        BunyanFormattingLayer::new("test".into(), move || MockWriter::new(buffer_clone.clone()))
            .with_hostname(Some(expected_hostname.clone()));
    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::with_default(subscriber, test_action);

    let buffer_guard = buffer.lock().unwrap();
    let output = buffer_guard.to_vec();
    let output = String::from_utf8(output)
        .unwrap()
        .lines()
        .next()
        .map(ToString::to_string)
        .unwrap();
    let output = output.parse::<Value>().unwrap();
    let new_hostname = output["hostname"].as_str().unwrap();

    assert_ne!(default_hostname, new_hostname);
    assert_eq!(new_hostname, expected_hostname);
}

#[test]
fn skip_hostname() {
    let buffer = Arc::new(Mutex::new(vec![]));
    let buffer_clone = buffer.clone();

    let formatting_layer =
        BunyanFormattingLayer::new("test".into(), move || MockWriter::new(buffer_clone.clone()))
            .with_hostname(None);

    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::with_default(subscriber, test_action);

    let buffer_guard = buffer.lock().unwrap();
    let output = buffer_guard.to_vec();
    let output = String::from_utf8(output)
        .unwrap()
        .lines()
        .next()
        .map(ToString::to_string)
        .unwrap();
    let output = output.parse::<Value>().unwrap();

    assert!(output.get("hostname").is_none());
}

#[test]
fn skipping_core_fields_is_not_allowed() {
    let skipped_fields = vec!["level"];

    let result = BunyanFormattingLayer::new("test".into(), || vec![])
        .skip_fields(skipped_fields.into_iter());

    match result {
        Err(err) => {
            assert_eq!(
                "level is a core field in the bunyan log format, it can't be skipped",
                err.to_string()
            );
        }
        Ok(_) => panic!("skipping core fields shouldn't work"),
    }
}

#[cfg(feature = "valuable")]
mod valuable_tests {
    use super::run_and_get_output;
    use serde_json::json;
    use valuable::Valuable;

    #[derive(Valuable)]
    struct ValuableStruct {
        a: u64,
        b: String,
        c: ValuableEnum,
    }

    #[derive(Valuable)]
    #[allow(dead_code)]
    enum ValuableEnum {
        A,
        B(u64),
        C(String),
    }

    #[test]
    fn encode_valuable_composite_types_as_json() {
        let out = run_and_get_output(|| {
            let s = ValuableStruct {
                a: 17,
                b: "Hello, world!".to_string(),
                c: ValuableEnum::B(27),
            };

            tracing::info!(s = s.as_value(), "Test info event");
        });

        assert_eq!(out.len(), 1);
        let entry = &out[0];

        let s_json = entry
            .as_object()
            .expect("expect entry is object")
            .get("s")
            .expect("expect entry.s is present");

        assert_eq!(
            json!({
                "a": 17,
                "b": "Hello, world!",
                "c": {
                    "B": 27,
                },
            }),
            *s_json
        );
    }
}
