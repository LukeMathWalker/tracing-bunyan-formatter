use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mock_writer::MockWriter;
use serde_json::{json, Value};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::layer::SubscriberExt;

mod mock_writer;

// Run a closure and collect the output emitted by the tracing instrumentation using an in-memory buffer.
//
// If `global` is `true` the subscriber is globally installed for all threads. If `false`, it is
// only set for the current thread, for the duration of `action`.
pub fn run_with_subscriber_and_get_raw_output<F: Fn()>(action: F, global: bool) -> String {
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
    let subscriber = tracing_subscriber::Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);

    if global {
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to install global subscriber");
        action();
    } else {
        tracing::subscriber::with_default(subscriber, action);
    }

    // Return the formatted output as a string to make assertions against
    let buffer_guard = buffer.lock().unwrap();
    let output = buffer_guard.to_vec();
    String::from_utf8(output).unwrap()
}

pub fn parse_output(output: String) -> Vec<Value> {
    output
        .lines()
        .filter(|&l| !l.trim().is_empty())
        .inspect(|l| println!("{}", l))
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}
