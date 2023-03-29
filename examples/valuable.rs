use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{
    layer::SubscriberExt,
    Registry,
};
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

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let formatting_layer = BunyanFormattingLayer::new("examples_valuable".into(), std::io::stdout);
    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let s = ValuableStruct {
        a: 17,
        b: "foo".to_string(),
        c: ValuableEnum::B(26),
    };

    tracing::info!(s = s.as_value(), "Test event");

    // Output example pretty printed:
    //
    // {
    //   "v": 0,
    //   "name": "examples_valuable",
    //   "msg": "Test event",
    //   "level": 30,
    //   "hostname": "foo",
    //   "pid": 26071,
    //   "time": "2023-03-29T18:34:38.445454908Z",
    //   "target": "valuable",
    //   "line": 36,
    //   "file": "examples/valuable.rs",
    //   "s": {
    //     "a": 17,
    //     "b": "foo",
    //     "c": {
    //       "B": 26
    //     }
    //   }
    // }

    Ok(())
}
