use clap::{Args, Parser, Subcommand};
use rdkafka::{
    config::FromClientConfig,
    consumer::{CommitMode, Consumer, StreamConsumer},
    message::{BorrowedHeaders, Header, Headers, Message, OwnedHeaders},
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use std::{str::FromStr, time::Duration};
use tokio::signal::unix::{signal, SignalKind};

#[derive(Debug, Parser)]
struct CommandLine {
    #[clap(flatten)]
    common_args: CommonArgs,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct CommonArgs {
    #[arg(long, env = "KAFKA_TOPIC_NAME")]
    kafka_topic_name: String,

    #[arg(long, env = "KAFKA_BOOTSTRAP_SERVERS")]
    kafka_bootstrap_servers: String,
}

#[derive(Debug, Subcommand)]
enum Command {
    Producer(Box<ProducerArgs>),
    Consumer(Box<ConsumerArgs>),
}

#[derive(Debug, Args)]
struct ProducerArgs {
    /// Optional message partition.
    ///
    /// If not given, Kafka will assign one.
    #[arg(long)]
    partition: Option<i32>,

    /// Optional message payload.
    #[arg(long)]
    payload: Option<String>,

    /// Optional message key.
    #[arg(long)]
    key: Option<String>,

    /// If `=` character is present in the value, it is treated as a separator between the header name and its value.
    /// If it's not present, the whole value is treated as a header name.
    ///
    /// Can be provided multiple times.
    #[arg(long)]
    header: Vec<MessageHeaderArg>,
}

#[derive(Debug, Clone)]
struct MessageHeaderArg {
    name: String,
    value: Option<String>,
}

impl FromStr for MessageHeaderArg {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, value) = match s.rsplit_once('=') {
            Some((name, value)) => (name.to_string(), Some(value.to_string())),
            None => (s.to_string(), None),
        };

        Ok(Self { name, value })
    }
}

#[derive(Debug, Args)]
struct ConsumerArgs {
    #[arg(long, env = "KAFKA_GROUP_ID")]
    kafka_group_id: String,
}

#[tokio::main]
async fn main() {
    let args = CommandLine::parse();

    match args.command {
        Command::Producer(producer_args) => producer(args.common_args, *producer_args).await,

        Command::Consumer(consumer_args) => consumer(args.common_args, *consumer_args).await,
    }
}

async fn producer(common_args: CommonArgs, producer_args: ProducerArgs) {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", common_args.kafka_bootstrap_servers);
    let producer: FutureProducer =
        FutureProducer::from_config(&config).expect("failed to create Kafka producer");

    println!("Built Kafka producer client");

    let mut headers = OwnedHeaders::new();
    for header in producer_args.header {
        headers = headers.insert(Header {
            key: &header.name,
            value: header.value.as_deref(),
        });
    }
    let headers = (headers.count() > 0).then_some(headers);

    let record = FutureRecord {
        topic: &common_args.kafka_topic_name,
        partition: producer_args.partition,
        payload: producer_args.payload.as_deref(),
        key: producer_args.key.as_deref(),
        timestamp: None,
        headers,
    };

    println!(
        "Sending message to Kafka: TOPIC=({}) KEY=({:?}) PARTITION=({:?}) PAYLOAD=({:?}) HEADERS=({:?})",
        record.topic, record.key, record.partition, record.payload, record.headers,
    );

    let (partition, offset) = producer
        .send(record, Duration::from_secs(5))
        .await
        .expect("failed to deliver the message");

    println!("Message delivered to partition {partition} with offset {offset}");
}

async fn consumer(common_args: CommonArgs, consumer_args: ConsumerArgs) {
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to prepare signal handler");

    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", common_args.kafka_bootstrap_servers);
    config.set("group.id", consumer_args.kafka_group_id);
    let consumer: StreamConsumer =
        StreamConsumer::from_config(&config).expect("failed to create Kafka consumer");
    println!("Built Kafka consumer client");
    println!("Subscribing to topic {}", common_args.kafka_topic_name);
    consumer
        .subscribe(&[&common_args.kafka_topic_name])
        .expect("failed to subscribe to the topic");
    println!("Subscribed to the topic");

    loop {
        let message = tokio::select! {
            message = consumer.recv() => message.expect("failed to receive the next message from Kafka"),
            _ = sigterm.recv() => break,
        };

        println!(
            "Received a message: TOPIC=({}) KEY=({:?}) PARTITION=({}) OFFSET=({}) PAYLOAD=({:?}) HEADERS=({:?})",
            message.topic(),
            message.key().map(String::from_utf8_lossy),
            message.partition(),
            message.offset(),
            message.payload().map(String::from_utf8_lossy),
            message.headers().map(BorrowedHeaders::detach),
        );

        consumer
            .commit_message(&message, CommitMode::Async)
            .expect("failed to commit message");
    }

    println!("Received SIGTERM, exiting");
    consumer
        .unassign()
        .expect("failed to unassign the consumer before exit");
}
