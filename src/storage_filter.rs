use std::collections::HashSet;

use tracing_subscriber::registry::{LookupSpan, SpanRef};

/// `FilteringMode` can either be `Include` when allowing only some particular fields
/// to be passed down or `Exclude` when allowing all fields except the specified ones.
#[derive(Clone, Debug)]
pub enum FilteringMode {
    Include,
    Exclude,
}

/// `JsonStorageFilter` will filter the fields passed from the parent span to the child span.
/// It can either be applied to all spans or to a particular span using its name.
#[derive(Clone, Debug)]
pub struct JsonStorageFilter {
    span_name: Option<String>,
    fields: HashSet<String>,
    mode: FilteringMode,
}

impl JsonStorageFilter {
    /// Create a new `JsonStorageFilter`.
    ///
    /// You have to specify:
    /// - `fields`, which are the names of fields affected by this filter;
    /// - `mode`, which is the mode of filtering for this filter.
    pub fn new(fields: HashSet<String>, mode: FilteringMode) -> Self {
        Self {
            span_name: None,
            fields,
            mode,
        }
    }

    /// Create a new `JsonStorageFilter`.
    ///
    /// You have to specify:
    /// - `span_name`, which is the name of the span that will be affected by this filter;
    /// - `fields`, which are the names of fields affected by this filter;
    /// - `mode`, which is the mode of filtering for this filter.
    pub fn for_span(span_name: String, fields: HashSet<String>, mode: FilteringMode) -> Self {
        Self {
            span_name: Some(span_name),
            fields,
            mode,
        }
    }

    pub(crate) fn filter_span<S>(&self, span: &SpanRef<S>) -> bool
    where
        S: for<'a> LookupSpan<'a>,
    {
        if let Some(span_name) = &self.span_name {
            span.name() == span_name
        } else {
            true
        }
    }

    pub(crate) fn filter_field(&self, name: &str) -> bool {
        match self.mode {
            FilteringMode::Include => self.fields.contains(name),
            FilteringMode::Exclude => !self.fields.contains(name),
        }
    }
}
