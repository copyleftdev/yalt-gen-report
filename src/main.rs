use structopt::StructOpt;
use rusqlite::{Connection, Result};
use plotters::prelude::*;
use plotters::style::{WHITE, BLUE, GREEN, RED, BLACK};
use std::path::Path;

#[derive(StructOpt, Debug)]
#[structopt(name = "report-generator")]
struct Opt {
    /// Path to the SQLite database file
    #[structopt(short, long)]
    database: String,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let conn = Connection::open(&opt.database)?;

    // Total requests
    let total_requests: i64 = conn.query_row(
        "SELECT COUNT(*) FROM metrics",
        [],
        |row| row.get(0),
    )?;
    println!("Total Requests: {}", total_requests);

    // Total successful requests
    let total_successful_requests: i64 = conn.query_row(
        "SELECT COUNT(*) FROM metrics WHERE success = 1",
        [],
        |row| row.get(0),
    )?;
    println!("Total Successful Requests: {}", total_successful_requests);

    // Total failed requests
    let total_failed_requests: i64 = conn.query_row(
        "SELECT COUNT(*) FROM metrics WHERE success = 0",
        [],
        |row| row.get(0),
    )?;
    println!("Total Failed Requests: {}", total_failed_requests);

    // Requests per second
    let mut stmt = conn.prepare(
        "SELECT 
            timestamp / 1 AS second, 
            COUNT(*) AS requests_per_second
        FROM 
            metrics
        GROUP BY 
            second
        ORDER BY 
            second",
    )?;
    let requests_per_second = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?
    .collect::<Result<Vec<(i64, i64)>>>()?;

    // Success and failure rates per second
    let mut stmt = conn.prepare(
        "SELECT 
            timestamp / 1 AS second, 
            SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) AS successful_requests,
            SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) AS failed_requests,
            COUNT(*) AS total_requests,
            (SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) * 1.0 / COUNT(*)) * 100 AS success_rate,
            (SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) * 1.0 / COUNT(*)) * 100 AS failure_rate
        FROM 
            metrics
        GROUP BY 
            second
        ORDER BY 
            second",
    )?;
    let success_failure_rates = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(4)?, row.get::<_, f64>(5)?))
    })?
    .collect::<Result<Vec<(i64, f64, f64)>>>()?;

    // Generate charts
    let root_area = BitMapBackend::new("requests_per_second.png", (1024, 768)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root_area)
        .caption("Requests Per Second", ("sans-serif", 50).into_font())
        .margin(5)
        .x_label_area_size(50)
        .y_label_area_size(50)
        .build_cartesian_2d(
            requests_per_second.first().unwrap().0..requests_per_second.last().unwrap().0,
            0..requests_per_second.iter().map(|x| x.1).max().unwrap()
        ).unwrap();

    chart.configure_mesh().draw().unwrap();
    chart.draw_series(LineSeries::new(
        requests_per_second.iter().map(|x| (x.0, x.1)),
        &BLUE,
    )).unwrap();
    root_area.present().unwrap(); // Ensure the chart is saved

    let root_area = BitMapBackend::new("success_failure_rates.png", (1024, 768)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root_area)
        .caption("Success and Failure Rates Per Second", ("sans-serif", 50).into_font())
        .margin(5)
        .x_label_area_size(50)
        .y_label_area_size(50)
        .build_cartesian_2d(
            success_failure_rates.first().unwrap().0..success_failure_rates.last().unwrap().0,
            0.0..100.0
        ).unwrap();

    chart.configure_mesh().draw().unwrap();
    chart.draw_series(LineSeries::new(
        success_failure_rates.iter().map(|x| (x.0, x.1)),
        &GREEN,
    )).unwrap().label("Success Rate").legend(|(x, y)| PathElement::new(vec![(x - 10, y), (x + 10, y)], &GREEN));

    chart.draw_series(LineSeries::new(
        success_failure_rates.iter().map(|x| (x.0, x.2)),
        &RED,
    )).unwrap().label("Failure Rate").legend(|(x, y)| PathElement::new(vec![(x - 10, y), (x + 10, y)], &RED));

    chart.configure_series_labels().background_style(&WHITE).border_style(&BLACK).draw().unwrap();
    root_area.present().unwrap(); // Ensure the chart is saved

    Ok(())
}
