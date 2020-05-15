use nats;
use quicli::prelude::*;
use structopt::StructOpt;

/// NATS utility that can perform basic publish, subscribe, request and reply functions.
#[derive(Debug, StructOpt)]
struct Cli {
    /// NATS server
    #[structopt(long, short, default_value = "demo.nats.io")]
    server: String,
    // Optional credentials for NATS 2.x
    #[structopt(long, default_value = "")]
    creds: String,
    /// Command: pub, sub, request, reply
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug, Clone)]
enum Command {
    /// The type of operation, can be one of pub, sub, qsub, req, reply.
    #[structopt(name = "pub", about = "Publishes a message to a given subject")]
    Pub { subject: String, msg: String },
    #[structopt(name = "sub", about = "Subscribes to a given subject")]
    Sub { subject: String },
    #[structopt(name = "request", about = "Sends a request and waits on reply")]
    Request { subject: String, msg: String },
    #[structopt(name = "reply", about = "Listens for requests and sends the reply")]
    Reply { subject: String, resp: String },
}

fn main() -> CliResult {
    let args = Cli::from_args();
    let nc: nats::Connection;

    if args.creds.len() > 0 {
        nc = nats::ConnectionOptions::new()
            .with_name("nats-box rust example")
            .with_credentials(&args.creds)
            .connect(&args.server)?;
    } else {
        nc = nats::ConnectionOptions::new()
            .with_name("nats-box rust example")
            .connect(&args.server)?;
    }

    match args.cmd {
        Command::Pub { subject, msg } => {
            nc.publish(&subject, &msg)?;
            println!("Published to '{}': '{}'", subject, msg);
        }
        Command::Sub { subject } => {
            let sub = nc.subscribe(&subject)?;
            println!("Listening on '{}'", subject);
            for msg in sub.messages() {
                println!("Received a {}", msg);
            }
        }
        Command::Request { subject, msg } => {
            println!("Waiting on response for '{}'", subject);
            let resp = nc.request(&subject, &msg)?;
            println!("Response is {}", resp);
        }
        Command::Reply { subject, resp } => {
            let sub = nc.queue_subscribe(&subject, "rust-box")?;
            println!("Listening for requests on '{}'", subject);
            for msg in sub.messages() {
                println!("Received a request {}", msg);
                msg.respond(&resp)?;
            }
        }
    }

    Ok(())
}
