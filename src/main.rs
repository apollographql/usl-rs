use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum, ValueHint};
use plotlib::page::Page;
use plotlib::repr::Plot;
use plotlib::style::{PointMarker, PointStyle};
use plotlib::view::ContinuousView;

use usl::{Measurement, Model};

/// Build and evaluate Universal Scalability Law models.
#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Opts {
    /// Path to input CSV file.
    #[clap(action, value_hint = ValueHint::FilePath)]
    input: PathBuf,

    /// Specify which two parameters are supplied in the input CSV
    /// (the third parameter will be derived using Little's Law)
    #[arg(value_enum, short, long, default_value_t = MeasurementKind::ConcurrencyAndThroughput)]
    kind: MeasurementKind,

    /// Output format for the model's result.
    #[arg(value_enum, short, long, default_value_t = OutputFormat::Text)]
    output: OutputFormat,

    /// Predict the throughput at the given concurrency levels.
    #[clap(action, short, long, num_args = 1..)]
    predictions: Vec<u32>,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
enum OutputFormat {
    /// Print the model as text.
    Text,
    /// Print the model as JSON.
    Json,
    /// Show a plot of the provided and modeled data.
    Plot,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MeasurementKind {
    /// System's throughput at a given level of concurrency.
    ConcurrencyAndThroughput,
    /// System's latency at a given level of concurrency.
    ConcurrencyAndLatency,
    /// System's latency at a given level of throughput.
    ThroughputAndLatency,
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let measurements = read_measurements(&opts.input, opts.kind)?;
    let model = Model::build(&measurements);
    match opts.output {
        OutputFormat::Json => {
            let json = serde_json::to_string(&model)?;
            println!("{}", json);
        }
        OutputFormat::Plot => {
            println!("{}", model);
            let observed = measurements.iter().map(|m| (m.n, m.x)).collect::<Vec<(f64, f64)>>();
            let max_n = observed.iter().map(|&(n, _)| n).fold(0.0, f64::max);
            let max_y = observed.iter().map(|&(_, y)| y).fold(0.0, f64::max);
            let observed =
                Plot::new(observed).point_style(PointStyle::new().marker(PointMarker::Square));

            let predicted = (0..(max_n as usize))
                .step_by(max_n as usize / 10)
                .map(|n| (n as f64, model.throughput_at_concurrency(n as f64)))
                .collect();
            let predicted =
                Plot::new(predicted).point_style(PointStyle::new().marker(PointMarker::Circle));

            let extrapolated = opts
                .predictions
                .iter()
                .map(|&n| (n as f64, model.throughput_at_concurrency(n)))
                .collect();
            let extrapolated =
                Plot::new(extrapolated).point_style(PointStyle::new().marker(PointMarker::Cross));

            let v = ContinuousView::new()
                .add(observed)
                .add(predicted)
                .add(extrapolated)
                .x_range(0.0, max_n)
                .y_range(0.0, max_y)
                .x_label("concurrency")
                .y_label("throughput");

            println!("{}", Page::single(&v).dimensions(100, 20).to_text().unwrap());
        }
        OutputFormat::Text => println!("{}", model),
    }

    for &n in &opts.predictions {
        println!("{},{}", n, model.throughput_at_concurrency(n));
    }

    Ok(())
}

fn read_measurements(path: &PathBuf, kind: MeasurementKind) -> Result<Vec<Measurement>> {
    let mut measurements = Vec::new();
    let mut input = csv::Reader::from_path(path)?;
    for record in input.records() {
        let record = record?;
        let record_0 = record[0].parse()?;
        let record_1 = record[1].parse()?;
        let m = match kind {
            MeasurementKind::ConcurrencyAndThroughput => {
                Measurement::concurrency_and_throughput(record_0, record_1)
            }
            MeasurementKind::ConcurrencyAndLatency => Measurement::concurrency_and_latency(
                record_0, record_1
            ),
            MeasurementKind::ThroughputAndLatency => Measurement::throughput_and_latency(
                record_0, record_1
            ),
        };
        measurements.push(m);
    }
    Ok(measurements)
}