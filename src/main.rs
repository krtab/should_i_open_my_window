use chrono::{DurationRound, TimeDelta};
use clap::Parser;
use comfy_table::{
    presets::{ASCII_FULL_CONDENSED, UTF8_FULL_CONDENSED},
    Cell, Cells, Table,
};
use open_meteo_rs::{
    forecast::{ForecastResult, Options},
    Client,
};

const TEMP_RANGE: [f64; 13] = [
    16., 16.5, 17., 17.5, 18., 18.5, 19., 19.5, 20., 20.5, 21., 21.5, 22.,
];

static DOC_STR: &str = "Opening the window will bring indoor humidity closer \
                        to the value indicated in the column corresponding to the \
                        indoor temperature";

#[derive(Parser)]
struct Args {
    lat: f64,
    lng: f64,
    /// Forces output to use ASCII only
    #[clap(long)]
    ascii: bool,
}

#[tokio::main]
async fn main() {
    let client = open_meteo_rs::Client::new();
    let mut opts = open_meteo_rs::forecast::Options::default();

    // Location
    let Args { lat, lng, ascii } = Args::parse();
    opts.location = open_meteo_rs::Location { lat, lng };
    opts.elevation = Some(63.1.into());
    opts.temperature_unit = Some(open_meteo_rs::forecast::TemperatureUnit::Celsius);
    opts.time_zone = Some("auto".to_owned());
    opts.forecast_days = Some(7);
    opts.hourly.push("temperature_2m".into());
    opts.hourly.push("relative_humidity_2m".into());
    let forecast = client.forecast(opts).await.unwrap();

    let t_h = print_one_table(&forecast, TableType::Hourly(10), ascii);
    let t_d = print_one_table(&forecast, TableType::Daily(7), ascii);
    println!("{}\n", DOC_STR);
    println!("{t_h}\n");
    println!("{t_d}");
}

enum TableType {
    Hourly(u8),
    Daily(u8),
}

impl TableType {
    fn name(&self) -> &'static str {
        match self {
            TableType::Hourly(_) => "Hourly",
            TableType::Daily(_) => "Daily",
        }
    }

    fn truncate(&self) -> TimeDelta {
        match self {
            TableType::Hourly(_) => TimeDelta::hours(1),
            TableType::Daily(_) => TimeDelta::days(1),
        }
    }

    fn count(&self) -> usize {
        match self {
            &TableType::Hourly(n) => n as usize,
            &TableType::Daily(n) => n as usize,
        }
    }

    fn step(&self) -> usize {
        match self {
            TableType::Hourly(_) => 1,
            TableType::Daily(_) => 24,
        }
    }
}

fn print_one_table(forecast: &ForecastResult, table_type: TableType, ascii: bool) -> Table {
    let mut table = Table::new();
    let sat_press: [f64; TEMP_RANGE.len()] =
        std::array::from_fn(|i| celsius_sat_pres(TEMP_RANGE[i]));
    let mut header =
        vec![Cell::new(String::from(table_type.name()))
            .add_attribute(comfy_table::Attribute::Italic)];
    for temp in TEMP_RANGE {
        let cell = Cell::new(format!("{temp:.1}°C")).add_attribute(comfy_table::Attribute::Bold);
        header.push(cell);
    }
    table.set_header(Cells(header));
    let now = chrono::offset::Local::now()
        .naive_local()
        .duration_trunc(table_type.truncate())
        .unwrap();
    for forecast_hrly in (&forecast.hourly)
        .into_iter()
        .flatten()
        .skip_while(|forecast| forecast.datetime < now)
        .step_by(table_type.step())
        .take(table_type.count())
    {
        let mut row: Vec<Cell> = vec![forecast_hrly.datetime.into()];
        let forecast_temp = forecast_hrly.values["temperature_2m"].value.as_f64().unwrap();
        let forecast_rh = forecast_hrly.values["relative_humidity_2m"]
            .value
            .as_f64()
            .unwrap();
        let forecast_sat_pres = celsius_sat_pres(forecast_temp);
        let forecast_vapor_pressure = forecast_rh * forecast_sat_pres;
        for &sat_pres in &sat_press {
            let rh = forecast_vapor_pressure / sat_pres;
            row.push(rh_cell(rh));
        }
        table.add_row(Cells(row));
    }
    if ascii {
        table.load_preset(ASCII_FULL_CONDENSED);
    } else {
        table.load_preset(UTF8_FULL_CONDENSED);
    }
    table
}

fn rh_cell(rh: f64) -> Cell {
    format!("{rh:.1}%").into()
}

fn celsius_sat_pres(celsius: f64) -> f64 {
    rust_steam::p_sat(celsius_to_kelvin(celsius))
}

fn celsius_to_kelvin(celsius: f64) -> f64 {
    celsius + 273.15
}
