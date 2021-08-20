#![allow(clippy::needless_doctest_main)]
//! # A Bunyan formatter for [`tracing`]
//!
//! `tracing-bunyan-formatter` provides two [`Layer`]s implementation to be used on top of
//! a [`tracing`] [`Subscriber`]:
//! - [`JsonStorageLayer`], to attach contextual information to spans for ease of consumption by
//!   downstream [`Layer`]s, via [`JsonStorage`] and [`Span`]'s [`extensions`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/registry/struct.ExtensionsMut.html);
//! - [`BunyanFormattingLayer`]`, which emits a [bunyan](https://github.com/trentm/node-bunyan)-compatible formatted record upon entering a span,
//!  existing a span and event creation.
//!
//! **Important**: each span will inherit all fields and properties attached to its parent - this is
//! currently not the behaviour provided by [`tracing_subscriber::fmt::Layer`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/fmt/struct.Layer.html).
//!
//! ## Example
//!
//! ```rust
//! use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
//! use tracing::instrument;
//! use tracing::info;
//! use tracing_subscriber::Registry;
//! use tracing_subscriber::layer::SubscriberExt;
//!
//! #[instrument]
//! pub fn a_unit_of_work(first_parameter: u64) {
//!     for i in 0..2 {
//!         a_sub_unit_of_work(i);
//!     }
//!     info!(excited = "true", "Tracing is quite cool!");
//! }
//!
//! #[instrument]
//! pub fn a_sub_unit_of_work(sub_parameter: u64) {
//!     info!("Events have the full context of their parent span!");
//! }
//!
//! fn main() {
//!     let formatting_layer = BunyanFormattingLayer::new("tracing_demo".into(), std::io::stdout);
//!     let subscriber = Registry::default()
//!         .with(JsonStorageLayer)
//!         .with(formatting_layer);
//!     tracing::subscriber::set_global_default(subscriber).unwrap();
//!
//!     info!("Orphan event without a parent span");
//!     a_unit_of_work(2);
//! }
//! ```
//!
//! ## Console output
//!
//! <div>
//! <img src="https://raw.githubusercontent.com/LukeMathWalker/tracing-bunyan-formatter/master/images/ConsoleOutput.png" />
//! </div>
//! <hr/>
//!
//! If you pipe the output in the [`bunyan`](https://github.com/trentm/node-bunyan) CLI:
//! <div>
//! <img src="https://raw.githubusercontent.com/LukeMathWalker/tracing-bunyan-formatter/master/images/ConsoleBunyanOutput.png" />
//! </div>
//! <hr/>
//!
//!
//! ## Implementation strategy
//!
//! The layered approach we have pursued is not necessarily the most efficient,
//! but it makes it easier to separate different concerns and re-use common logic across multiple [`Layer`]s.
//!
//!
//!
//! While the current crate has no ambition to provide any sort of general purpose framework on top of
//! [`tracing-subscriber`]'s [`Layer`] trait, the information collected by [`JsonStorageLayer`] can be leveraged via
//! its public API by other downstream layers outside of this crate whose main concern is formatting.
//! It significantly lowers the amount of complexity you have to deal with if you are interested
//! in implementing your own formatter, for whatever reason or purpose.
//!
//! You can also add another enrichment layer following the [`JsonStorageLayer`] to collect
//! additional information about each span and store it in [`JsonStorage`].
//! We could have pursued this compositional approach to add `elapsed_milliseconds` to each span
//! instead of baking it in [`JsonStorage`] itself.
//!
//! ## Optional features
//! 
//! You can enable the `arbitrary_precision` feature to handle numbers of arbitrary size losslessly. Be aware of a [known issue with untagged deserialization](https://github.com/LukeMathWalker/tracing-bunyan-formatter/issues/4).
//!
//! [`Layer`]: https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/layer/trait.Layer.html
//! [`JsonStorageLayer`]: struct.JsonStorageLayer.html
//! [`JsonStorage`]: struct.JsonStorage.html
//! [`BunyanFormattingLayer`]: struct.BunyanFormattingLayer.html
//! [`Span`]: https://docs.rs/tracing/0.1.13/tracing/struct.Span.html
//! [`Subscriber`]: https://docs.rs/tracing-core/0.1.10/tracing_core/subscriber/trait.Subscriber.html
//! [`tracing`]: https://docs.rs/tracing
//! [`tracing`]: https://docs.rs/tracing-subscriber
mod formatting_layer;
mod storage_layer;

pub use formatting_layer::*;
pub use storage_layer::*;
