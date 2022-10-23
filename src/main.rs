use std::sync::Arc;

use chrono::FixedOffset;
use clap::Parser;
use tonic::transport::Server;
use tracing::info;

use crate::handler::external_scaler_server::ExternalScalerServer;
use crate::handler::GrpcHandler;
use crate::working_days::WorkingDays;

mod handler;
mod holidays_loader;
mod working_days;

#[derive(Parser, Debug)]
#[command(name = "working-days-scaler")]
#[command(author = "José V. Almada")]
#[command(version = "1.0")]
#[command(about = "External scaler for KEDA", long_about = None)]
pub struct Args {
    /// Path to the holidays CSV.
    #[arg(short = 'f', long, default_value_t = String::from("holidays.csv"))]
    holidays_file: String,
    /// The port that the gRPC server will be listening.
    #[arg(short = 'p', long, default_value_t = 8080)]
    port: u16,
    /// The time offset in seconds. Value between -86400 and -86400.
    #[arg(short = 't', long, allow_negative_numbers = true, default_value_t = 0)]
    time_offset: i32,
    /// The interval in seconds between IsActiveStream messages stream.
    #[arg(short = 'i', long, default_value_t = 60)]
    push_interval: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let time_offset = FixedOffset::east(args.time_offset);
    info!("Using configured time offset {}.", time_offset);

    let holidays = holidays_loader::load(time_offset, &args.holidays_file)?;
    info!(
        "Loaded {} holidays from {}.",
        holidays.len(),
        args.holidays_file
    );

    let working_days = WorkingDays::build(time_offset, holidays)?;
    info!(
        "Application ready to calculate working days MTD between {} and {}.",
        working_days.start_date, working_days.end_date
    );

    info!(
        "Interval between IsActiveStream messages stream: {} s.",
        args.push_interval
    );

    let addr = format!("[::1]:{}", args.port).parse().unwrap();
    info!("GRPC server listening on {}.", addr);

    let handler = GrpcHandler {
        working_days: Arc::new(working_days),
        push_interval: args.push_interval,
    };

    Server::builder()
        .add_service(ExternalScalerServer::new(handler))
        .serve(addr)
        .await?;

    Ok(())
}
