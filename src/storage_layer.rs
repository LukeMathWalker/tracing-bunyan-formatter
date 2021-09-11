use std::collections::HashMap;
use std::fmt;
use std::time::Instant;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Record};
use tracing::{Id, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// This layer is only concerned with information storage, it does not do any formatting or provide any output.
///
/// It's purpose is to store the fields associated to spans in an easy-to-consume format
/// for downstream layers concerned with emitting a formatted representation of
/// spans or events.
#[derive(Clone, Debug)]
pub struct JsonStorageLayer;

/// `JsonStorage` will collect information about a span when it's created (`new_span` handler)
/// or when new records are attached to it (`on_record` handler) and store it in its `extensions`
/// for future retrieval from other layers interested in formatting or further enrichment.
///
/// We are re-implementing (well, copy-pasting, apart from using an HashMap instead of a BTreeMap)
/// `JsonVisitor` from `tracing-subscriber` given that we can't access/insert/iterate over
/// the underlying BTreeMap using its public API.
///
/// For spans, we also store the duration of each span with the `elapsed_milliseconds` key using
/// the `on_exit`/`on_enter` handlers.
#[derive(Clone, Debug)]
pub struct JsonStorage<'a> {
    values: HashMap<&'a str, serde_json::Value>,
}

impl<'a> JsonStorage<'a> {
    /// Get the set of stored values, as a set of keys and JSON values.
    pub fn values(&self) -> &HashMap<&'a str, serde_json::Value> {
        &self.values
    }
}

/// Get a new visitor, with an empty bag of key-value pairs.
impl Default for JsonStorage<'_> {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}

/// Taken verbatim from tracing-subscriber
impl Visit for JsonStorage<'_> {
    /// Visit a signed 64-bit integer value.
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.values
            .insert(&field.name(), serde_json::Value::from(value));
    }

    /// Visit an unsigned 64-bit integer value.
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.values
            .insert(&field.name(), serde_json::Value::from(value));
    }

    /// Visit a boolean value.
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.values
            .insert(&field.name(), serde_json::Value::from(value));
    }

    /// Visit a string value.
    fn record_str(&mut self, field: &Field, value: &str) {
        self.values
            .insert(&field.name(), serde_json::Value::from(value));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        match field.name() {
            // Skip fields that are actually log metadata that have already been handled
            name if name.starts_with("log.") => (),
            name if name.starts_with("r#") => {
                self.values
                    .insert(&name[2..], serde_json::Value::from(format!("{:?}", value)));
            }
            name => {
                self.values
                    .insert(name, serde_json::Value::from(format!("{:?}", value)));
            }
        };
    }
}

impl<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>> Layer<S>
    for JsonStorageLayer
{
    /// Span creation.
    /// This is the only occasion we have to store the fields attached to the span
    /// given that they might have been borrowed from the surrounding context.
    fn new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");

        // We want to inherit the fields from the parent span, if there is one.
        let mut visitor = if let Some(parent_span) = span.parent() {
            // Extensions can be used to associate arbitrary data to a span.
            // We'll use it to store our representation of its fields.
            // We create a copy of the parent visitor!
            let mut extensions = parent_span.extensions_mut();
            extensions
                .get_mut::<JsonStorage>()
                .map(|v| v.to_owned())
                .unwrap_or_default()
        } else {
            JsonStorage::default()
        };

        let mut extensions = span.extensions_mut();

        // Register all fields.
        // Fields on the new span should override fields on the parent span if there is a conflict.
        attrs.record(&mut visitor);
        // Associate the visitor with the Span for future usage via the Span's extensions
        extensions.insert(visitor);
    }

    fn on_record(&self, span: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let span = ctx.span(span).expect("Span not found, this is a bug");

        // Before you can associate a record to an existing Span, well, that Span has to be created!
        // We can thus rely on the invariant that we always associate a JsonVisitor with a Span
        // on creation (`new_span` method), hence it's safe to unwrap the Option.
        let mut extensions = span.extensions_mut();
        let visitor = extensions
            .get_mut::<JsonStorage>()
            .expect("Visitor not found on 'record', this is a bug");
        // Register all new fields
        values.record(visitor);
    }

    /// When we enter a span **for the first time** save the timestamp in its extensions.
    fn on_enter(&self, span: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(span).expect("Span not found, this is a bug");

        let mut extensions = span.extensions_mut();
        if extensions.get_mut::<Instant>().is_none() {
            extensions.insert(Instant::now());
        }
    }

    /// When we close a span, register how long it took in milliseconds.
    fn on_close(&self, span: Id, ctx: Context<'_, S>) {
        let span = ctx.span(&span).expect("Span not found, this is a bug");

        // Using a block to drop the immutable reference to extensions
        // given that we want to borrow it mutably just below
        let elapsed_milliseconds = {
            let extensions = span.extensions();
            extensions
                .get::<Instant>()
                .map(|i| i.elapsed().as_millis())
                // If `Instant` is not in the span extensions it means that the span was never
                // entered into.
                .unwrap_or(0)
        };

        #[cfg(not(feature = "arbitrary_precision"))]
        // without the arbitrary_precision feature u128 values are not supported,
        // but u64 is still more than enough for our purposes
        let elapsed_milliseconds = elapsed_milliseconds as u64;

        let mut extensions_mut = span.extensions_mut();
        let visitor = extensions_mut
            .get_mut::<JsonStorage>()
            .expect("Visitor not found on 'record', this is a bug");

        if let Ok(elapsed) = serde_json::to_value(elapsed_milliseconds) {
            visitor.values.insert("elapsed_milliseconds", elapsed);
        }
    }
}
