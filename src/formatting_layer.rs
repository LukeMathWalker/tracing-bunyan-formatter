use crate::storage_layer::JsonStorage;
use ahash::{HashSet, HashSetExt};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use time::format_description::well_known::Rfc3339;
use tracing::{Event, Id, Metadata, Subscriber};
use tracing_core::metadata::Level;
use tracing_core::span::Attributes;
use tracing_log::AsLog;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::SpanRef;
use tracing_subscriber::Layer;

/// Keys for core fields of the Bunyan format (https://github.com/trentm/node-bunyan#core-fields)
const BUNYAN_VERSION: &str = "v";
const LEVEL: &str = "level";
const NAME: &str = "name";
const HOSTNAME: &str = "hostname";
const PID: &str = "pid";
const TIME: &str = "time";
const MESSAGE: &str = "msg";
const _SOURCE: &str = "src";

const BUNYAN_REQUIRED_FIELDS: [&str; 7] =
    [BUNYAN_VERSION, LEVEL, NAME, HOSTNAME, PID, TIME, MESSAGE];

/// Convert from log levels to Bunyan's levels.
fn to_bunyan_level(level: &Level) -> u16 {
    match level.as_log() {
        log::Level::Error => 50,
        log::Level::Warn => 40,
        log::Level::Info => 30,
        log::Level::Debug => 20,
        log::Level::Trace => 10,
    }
}

/// This layer is exclusively concerned with formatting information using the [Bunyan format](https://github.com/trentm/node-bunyan).
/// It relies on the upstream `JsonStorageLayer` to get access to the fields attached to
/// each span.
pub struct BunyanFormattingLayer<W: for<'a> MakeWriter<'a> + 'static> {
    make_writer: W,
    pid: u32,
    hostname: Option<String>,
    bunyan_version: u8,
    name: String,
    default_fields: HashMap<String, Value>,
    skip_fields: HashSet<String>,
}

/// This error will be returned in [`BunyanFormattingLayer::skip_fields`] if trying to skip a core field.
#[non_exhaustive]
#[derive(Debug)]
pub struct SkipFieldError(String);

impl fmt::Display for SkipFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is a core field in the bunyan log format, it can't be skipped",
            &self.0
        )
    }
}

impl std::error::Error for SkipFieldError {}

impl<W: for<'a> MakeWriter<'a> + 'static> BunyanFormattingLayer<W> {
    /// Create a new `BunyanFormattingLayer`.
    ///
    /// You have to specify:
    /// - a `name`, which will be attached to all formatted records according to the [Bunyan format](https://github.com/trentm/node-bunyan#log-record-fields);
    /// - a `make_writer`, which will be used to get a `Write` instance to write formatted records to.
    ///
    /// ## Using stdout
    ///
    /// ```rust
    /// use tracing_bunyan_formatter::BunyanFormattingLayer;
    ///
    /// let formatting_layer = BunyanFormattingLayer::new("tracing_example".into(), std::io::stdout);
    /// ```
    ///
    /// If you prefer, you can use closure syntax:
    ///
    /// ```rust
    /// use tracing_bunyan_formatter::BunyanFormattingLayer;
    ///
    /// let formatting_layer = BunyanFormattingLayer::new("tracing_example".into(), || std::io::stdout());
    /// ```
    pub fn new(name: String, make_writer: W) -> Self {
        Self::with_default_fields(name, make_writer, HashMap::new())
    }

    /// Add default fields to all formatted records.
    ///
    /// ```rust
    /// use std::collections::HashMap;
    /// use serde_json::json;
    /// use tracing_bunyan_formatter::BunyanFormattingLayer;
    ///
    /// let mut default_fields = HashMap::new();
    /// default_fields.insert("custom_field".to_string(), json!("custom_value"));
    /// let formatting_layer = BunyanFormattingLayer::with_default_fields(
    ///     "test".into(),
    ///     std::io::stdout,
    ///     default_fields,
    /// );
    /// ```
    pub fn with_default_fields(
        name: String,
        make_writer: W,
        default_fields: HashMap<String, Value>,
    ) -> Self {
        Self {
            make_writer,
            name,
            pid: std::process::id(),
            hostname: Some(gethostname::gethostname().to_string_lossy().into_owned()),
            bunyan_version: 0,
            default_fields,
            skip_fields: HashSet::new(),
        }
    }

    pub fn with_hostname(mut self, hostname: Option<String>) -> Self {
        self.hostname = hostname;
        self
    }

    /// Add fields to skip when formatting with this layer.
    ///
    /// It returns an error if you try to skip a required core Bunyan field (e.g. `name`).
    /// You can skip optional core Bunyan fields (e.g. `line`, `file`, `target`).
    ///
    /// ```rust
    /// use tracing_bunyan_formatter::BunyanFormattingLayer;
    ///
    /// let skipped_fields = vec!["skipped"];
    /// let formatting_layer = BunyanFormattingLayer::new("test".into(), std::io::stdout)
    ///     .skip_fields(skipped_fields.into_iter())
    ///     .expect("One of the specified fields cannot be skipped");
    /// ```
    pub fn skip_fields<Fields, Field>(mut self, fields: Fields) -> Result<Self, SkipFieldError>
    where
        Fields: Iterator<Item = Field>,
        Field: Into<String>,
    {
        for field in fields {
            let field = field.into();
            if BUNYAN_REQUIRED_FIELDS.contains(&field.as_str()) {
                return Err(SkipFieldError(field));
            }
            self.skip_fields.insert(field);
        }

        Ok(self)
    }

    fn serialize_bunyan_core_fields(
        &self,
        map_serializer: &mut impl SerializeMap<Error = serde_json::Error>,
        message: &str,
        level: &Level,
    ) -> Result<(), std::io::Error> {
        map_serializer.serialize_entry(BUNYAN_VERSION, &self.bunyan_version)?;
        map_serializer.serialize_entry(NAME, &self.name)?;
        map_serializer.serialize_entry(MESSAGE, &message)?;
        map_serializer.serialize_entry(LEVEL, &to_bunyan_level(level))?;
        if let Some(hostname) = &self.hostname {
            map_serializer.serialize_entry(HOSTNAME, hostname)?;
        }
        map_serializer.serialize_entry(PID, &self.pid)?;
        if let Ok(time) = &time::OffsetDateTime::now_utc().format(&Rfc3339) {
            map_serializer.serialize_entry(TIME, time)?;
        }
        Ok(())
    }

    fn serialize_field<V>(
        &self,
        map_serializer: &mut impl SerializeMap<Error = serde_json::Error>,
        key: &str,
        value: &V,
    ) -> Result<(), std::io::Error>
    where
        V: Serialize + ?Sized,
    {
        if !self.skip_fields.contains(key) {
            map_serializer.serialize_entry(key, value)?;
        }

        Ok(())
    }

    /// Given a span, it serialised it to a in-memory buffer (vector of bytes).
    fn serialize_span<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>(
        &self,
        span: &SpanRef<S>,
        ty: Type,
    ) -> Result<Vec<u8>, std::io::Error> {
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buffer);
        let mut map_serializer = serializer.serialize_map(None)?;
        let message = format_span_context(span, ty);
        self.serialize_bunyan_core_fields(&mut map_serializer, &message, span.metadata().level())?;
        // Additional metadata useful for debugging
        // They should be nested under `src` (see https://github.com/trentm/node-bunyan#src )
        // but `tracing` does not support nested values yet
        self.serialize_field(&mut map_serializer, "target", span.metadata().target())?;
        self.serialize_field(&mut map_serializer, "line", &span.metadata().line())?;
        self.serialize_field(&mut map_serializer, "file", &span.metadata().file())?;

        // Add all default fields
        for (key, value) in self.default_fields.iter() {
            // Make sure this key isn't reserved. If it is reserved,
            // silently ignore
            if !BUNYAN_REQUIRED_FIELDS.contains(&key.as_str()) {
                self.serialize_field(&mut map_serializer, key, value)?;
            }
        }

        let extensions = span.extensions();
        if let Some(visitor) = extensions.get::<JsonStorage>() {
            for (key, value) in visitor.values() {
                // Make sure this key isn't reserved. If it is reserved,
                // silently ignore
                if !BUNYAN_REQUIRED_FIELDS.contains(key) {
                    self.serialize_field(&mut map_serializer, key, value)?;
                }
            }
        }
        map_serializer.end()?;
        // We add a trailing new line.
        buffer.write_all(b"\n")?;
        Ok(buffer)
    }

    /// Given an in-memory buffer holding a complete serialised record, flush it to the writer
    /// returned by self.make_writer.
    ///
    /// If we write directly to the writer returned by self.make_writer in more than one go
    /// we can end up with broken/incoherent bits and pieces of those records when
    /// running multi-threaded/concurrent programs.
    fn emit(&self, buffer: &[u8], meta: &Metadata<'_>) -> Result<(), std::io::Error> {
        self.make_writer.make_writer_for(meta).write_all(buffer)
    }
}

/// The type of record we are dealing with: entering a span, exiting a span, an event.
#[derive(Clone, Debug)]
pub enum Type {
    EnterSpan,
    ExitSpan,
    Event,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match self {
            Type::EnterSpan => "START",
            Type::ExitSpan => "END",
            Type::Event => "EVENT",
        };
        write!(f, "{}", repr)
    }
}

/// Ensure consistent formatting of the span context.
///
/// Example: "[AN_INTERESTING_SPAN - START]"
fn format_span_context<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>(
    span: &SpanRef<S>,
    ty: Type,
) -> String {
    format!("[{} - {}]", span.metadata().name().to_uppercase(), ty)
}

/// Ensure consistent formatting of event message.
///
/// Examples:
/// - "[AN_INTERESTING_SPAN - EVENT] My event message" (for an event with a parent span)
/// - "My event message" (for an event without a parent span)
fn format_event_message<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>(
    current_span: &Option<SpanRef<S>>,
    event: &Event,
    event_visitor: &JsonStorage<'_>,
) -> String {
    // Extract the "message" field, if provided. Fallback to the target, if missing.
    let mut message = event_visitor
        .values()
        .get("message")
        .and_then(|v| match v {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        })
        .unwrap_or_else(|| event.metadata().target())
        .to_owned();

    // If the event is in the context of a span, prepend the span name to the message.
    if let Some(span) = &current_span {
        message = format!("{} {}", format_span_context(span, Type::Event), message);
    }

    message
}

impl<S, W> Layer<S> for BunyanFormattingLayer<W>
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // Events do not necessarily happen in the context of a span, hence lookup_current
        // returns an `Option<SpanRef<_>>` instead of a `SpanRef<_>`.
        let current_span = ctx.lookup_current();

        let mut event_visitor = JsonStorage::default();
        event.record(&mut event_visitor);

        // Opting for a closure to use the ? operator and get more linear code.
        let format = || {
            let mut buffer = Vec::new();

            let mut serializer = serde_json::Serializer::new(&mut buffer);
            let mut map_serializer = serializer.serialize_map(None)?;

            let message = format_event_message(&current_span, event, &event_visitor);
            self.serialize_bunyan_core_fields(
                &mut map_serializer,
                &message,
                event.metadata().level(),
            )?;
            // Additional metadata useful for debugging
            // They should be nested under `src` (see https://github.com/trentm/node-bunyan#src )
            // but `tracing` does not support nested values yet
            self.serialize_field(&mut map_serializer, "target", event.metadata().target())?;
            self.serialize_field(&mut map_serializer, "line", &event.metadata().line())?;
            self.serialize_field(&mut map_serializer, "file", &event.metadata().file())?;

            // Add all default fields
            for (key, value) in self.default_fields.iter().filter(|(key, _)| {
                key.as_str() != "message" && !BUNYAN_REQUIRED_FIELDS.contains(&key.as_str())
            }) {
                self.serialize_field(&mut map_serializer, key, value)?;
            }

            // Add all the other fields associated with the event, expect the message we already used.
            for (key, value) in event_visitor
                .values()
                .iter()
                .filter(|(&key, _)| key != "message" && !BUNYAN_REQUIRED_FIELDS.contains(&key))
            {
                self.serialize_field(&mut map_serializer, key, value)?;
            }

            // Add all the fields from the current span, if we have one.
            if let Some(span) = &current_span {
                let extensions = span.extensions();
                if let Some(visitor) = extensions.get::<JsonStorage>() {
                    for (key, value) in visitor.values() {
                        // Make sure this key isn't reserved. If it is reserved,
                        // silently ignore
                        if !BUNYAN_REQUIRED_FIELDS.contains(key) {
                            self.serialize_field(&mut map_serializer, key, value)?;
                        }
                    }
                }
            }
            map_serializer.end()?;
            // We add a trailing new line.
            buffer.write_all(b"\n")?;

            Ok(buffer)
        };

        let result: std::io::Result<Vec<u8>> = format();
        if let Ok(formatted) = result {
            let _ = self.emit(&formatted, event.metadata());
        }
    }

    fn on_new_span(&self, _attrs: &Attributes, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        if let Ok(serialized) = self.serialize_span(&span, Type::EnterSpan) {
            let _ = self.emit(&serialized, span.metadata());
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("Span not found, this is a bug");
        if let Ok(serialized) = self.serialize_span(&span, Type::ExitSpan) {
            let _ = self.emit(&serialized, span.metadata());
        }
    }
}
