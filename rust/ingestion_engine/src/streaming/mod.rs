pub mod kafka_sink;

pub use kafka_sink::{
    KafkaSentimentEvent, KafkaSink, KafkaSinkConfig, RealtimeSentimentEvent,
};
