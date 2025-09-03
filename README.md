<h1 align="center">tracing-bunyan-formatter</h1>
<div align="center">
 <strong>
   Bunyan formatting for tokio-rs/tracing.
 </strong>
</div>

<br />

<div align="center">
  <!-- Crates version -->
  <a href="https://crates.io/crates/tracing-bunyan-formatter">
    <img src="https://img.shields.io/crates/v/tracing-bunyan-formatter.svg?style=flat-square"
    alt="Crates.io version" />
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/tracing-bunyan-formatter">
    <img src="https://img.shields.io/crates/d/tracing-bunyan-formatter.svg?style=flat-square"
      alt="Download" />
  </a>
  <!-- docs.rs docs -->
  <a href="https://docs.rs/tracing-bunyan-formatter">
    <img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square"
      alt="docs.rs docs" />
  </a>
  <!-- CI -->
  <a href="https://github.com/LukeMathWalker/tracing-bunyan-formatter">
    <img src="https://circleci.com/gh/LukeMathWalker/tracing-bunyan-formatter.svg?style=shield" alt="CircleCI badge" />
  </a>
</div>
<br/>

`tracing-bunyan-formatter` provides two [`Layer`]s implementation to be used on top of
a [`tracing`] [`Subscriber`]:
- [`JsonStorageLayer`], to attach contextual information to spans for ease of consumption by
  downstream [`Layer`]s, via [`JsonStorage`] and [`Span`]'s [`extensions`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/registry/struct.ExtensionsMut.html);
- [`BunyanFormattingLayer`], which emits a [bunyan](https://github.com/trentm/node-bunyan)-compatible formatted record upon entering a span,
 exiting a span and event creation.

**Important**: each span will inherit all fields and properties attached to its parent - this is
currently not the behaviour provided by [`tracing_subscriber::fmt::Layer`](https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/fmt/struct.Layer.html).

## Example

```rust
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing::instrument;
use tracing::info;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

#[instrument]
pub fn a_unit_of_work(first_parameter: u64) {
    for i in 0..2 {
        a_sub_unit_of_work(i);
    }
    info!(excited = "true", "Tracing is quite cool!");
}

#[instrument]
pub fn a_sub_unit_of_work(sub_parameter: u64) {
    info!("Events have the full context of their parent span!");
}

fn main() {
    let formatting_layer = BunyanFormattingLayer::new("tracing_demo".into(), std::io::stdout);
    let subscriber = Registry::default()
        .with(JsonStorageLayer::new())
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    info!("Orphan event without a parent span");
    a_unit_of_work(2);
}
```

## Console output

<div>
<img src="https://raw.githubusercontent.com/LukeMathWalker/tracing-bunyan-formatter/master/images/ConsoleOutput.png" />
</div>
<hr/>

If you pipe the output in the [`bunyan`](https://github.com/trentm/node-bunyan) CLI:
<div>
<img src="https://raw.githubusercontent.com/LukeMathWalker/tracing-bunyan-formatter/master/images/ConsoleBunyanOutput.png" />
</div>
<hr/>

As a pure-Rust alternative check out the [`bunyan` crate](https://crates.io/crates/bunyan).
It includes a CLI binary with similar functionality to the original `bunyan` CLI written in
JavaScript.


## Implementation strategy

The layered approach we have pursued is not necessarily the most efficient,
but it makes it easier to separate different concerns and re-use common logic across multiple [`Layer`]s.

While the current crate has no ambition to provide any sort of general purpose framework on top of
[`tracing-subscriber`]'s [`Layer`] trait, the information collected by [`JsonStorageLayer`] can be leveraged via
its public API by other downstream layers outside of this crate whose main concern is formatting.
It significantly lowers the amount of complexity you have to deal with if you are interested
in implementing your own formatter, for whatever reason or purpose.

You can also add another enrichment layer following the [`JsonStorageLayer`] to collect
additional information about each span and store it in [`JsonStorage`].
We could have pursued this compositional approach to add `elapsed_milliseconds` to each span
instead of baking it in [`JsonStorage`] itself.

## Optional features

You can enable the `arbitrary_precision` feature to handle numbers of arbitrary size losslessly. Be aware of a [known issue with untagged deserialization](https://github.com/LukeMathWalker/tracing-bunyan-formatter/issues/4).

### `valuable`

The `tracing` crate has an unstable feature `valuable` to enable
recording custom composite types like `struct`s and `enum`s. Custom
types must implement the [`valuable`
crate](https://crates.io/crates/valuable)'s `Valuable` trait, which
can be derived with a macro.

To use `tracing` and `tracing-bunyan-formatter` with `valuable`, you must set the following configuration in your binary (as of the current crate versions on 2023-03-29):

1. Enable the feature flag `valuable` for the `tracing` dependency.
2. Add the `--cfg tracing_unstable` arguments to your rustc
   flags (see [`tracing`'s documentation on this][tracing_unstable]).
   This can be done in a few ways:
    1. Adding the arguments to your binary package's
       `.cargo/config.toml` under `build.rustflags`. See the
       [`cargo` config reference documentation][cargo_build_rustflags]).

       Example:

       ```toml
       [build]
       rustflags = "--cfg tracing_unstable"
       ```

    2. Adding the arguments to the `RUSTFLAGS` environment variable when you
       run `cargo`. See the [`cargo` environment variable
       docs][cargo_env_vars]).

       Example:
       ```sh
       RUSTFLAGS="--cfg tracing_unstable" cargo build
       ```

3. Enable the feature flag `valuable` for the `tracing-bunyan-formatter` dependency.
4. Add dependency `valuable`.
5. Optional: if you want to derive the `Valuable` trait for your
   custom types, enable the feature flag `derive` for the `valuable`
   dependency.

See more details in the example in [`examples/valuable.rs`](examples/valuable.rs).

[cargo_build_rustflags]: https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags
[cargo_env_vars]: https://doc.rust-lang.org/cargo/reference/environment-variables.html
[tracing_unstable]: https://docs.rs/tracing/0.1.37/tracing/index.html#unstable-features

## Testing

Just run `cargo test`.

To run extra tests with the `valuable` feature enabled, run:

```sh
RUSTFLAGS='--cfg tracing_unstable' \
cargo test --target-dir target/debug_valuable --features "valuable valuable/derive"

RUSTFLAGS='--cfg tracing_unstable' \
cargo run --example valuable --target-dir target/debug_valuable --features "valuable valuable/derive"
```

[`Layer`]: https://docs.rs/tracing-subscriber/0.2.5/tracing_subscriber/layer/trait.Layer.html
[`JsonStorageLayer`]: https://docs.rs/tracing-bunyan-formatter/0.1.6/tracing_bunyan_formatter/struct.JsonStorageLayer.html
[`JsonStorage`]: https://docs.rs/tracing-bunyan-formatter/0.1.6/tracing_bunyan_formatter/struct.JsonStorage.html
[`BunyanFormattingLayer`]: https://docs.rs/tracing-bunyan-formatter/0.1.6/tracing_bunyan_formatter/struct.BunyanFormattingLayer.html
[`Span`]: https://docs.rs/tracing/0.1.13/tracing/struct.Span.html
[`Subscriber`]: https://docs.rs/tracing-core/0.1.10/tracing_core/subscriber/trait.Subscriber.html
[`tracing`]: https://docs.rs/tracing
[`tracing-subscriber`]: https://docs.rs/tracing-subscriber
