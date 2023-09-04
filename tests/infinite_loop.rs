use common::parse_output;
use tracing::{info, info_span};

mod common;

/// Make sure the `tracing::debug!()` calls in `BunyanFormattingLayer` don't cause an infinite loop
/// (and a stack overflow).
#[test]
pub fn no_infinite_loop() {
    // Note: the infinite loop bug could only happen if `BunyanFormattingLayer`
    // is set as the global subscriber. (`tracing` guards against a thread-local
    // subscriber being aquired twice, returning `NONE` the second time, so any
    // `tracing::debug!()` statement withing `BunyanFormattingLayer` are dropped
    // in that case)
    let output = common::run_with_subscriber_and_get_raw_output(
        || {
            info_span!("my span", name = "foo").in_scope(|| {
                info!("Calling foo");
            });
        },
        true,
    );

    // If we get here, that means the code above didn't crash with a stack overflow.
    let logs = parse_output(output);
    // We expect 6 log lines: 3 for the span start, log event, span end, but each one is preceded by
    // the debug log from `BunyanFormattingLayer` regarding using a reserved field.
    assert_eq!(6, logs.len());
}
