use chrono::{DurationRound, TimeDelta};
use clap::Parser;
use comfy_table::{Cell, Cells, Table};

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
}

#[tokio::main]
async fn main() {
    let client = open_meteo_rs::Client::new();
    let mut opts = open_meteo_rs::forecast::Options::default();

    // Location
    let Args { lat, lng } = Args::parse();
    opts.location = open_meteo_rs::Location { lat, lng };
    opts.elevation = Some(63.1.into());
    opts.temperature_unit = Some(open_meteo_rs::forecast::TemperatureUnit::Celsius);
    opts.wind_speed_unit = Some(open_meteo_rs::forecast::WindSpeedUnit::Kmh);
    opts.precipitation_unit = Some(open_meteo_rs::forecast::PrecipitationUnit::Millimeters);
    opts.time_zone = Some(chrono_tz::Europe::Paris.name().into());

    // Past days (0-2)
    // opts.past_days = Some(2); // !! mutually exclusive with dates

    // Forecast days (0-16)
    opts.forecast_days = Some(2); // !! mutually exclusive with dates
                                  // Hourly parameters
    opts.hourly.push("temperature_2m".into());
    opts.hourly.push("relative_humidity_2m".into());
    let res = client.forecast(opts).await.unwrap();

    let mut table = Table::new();
    let sat_press: [f64; TEMP_RANGE.len()] =
        std::array::from_fn(|i| celsius_sat_pres(TEMP_RANGE[i]));
    let mut header = vec![Cell::new(String::new())];
    for temp in TEMP_RANGE {
        let cell = Cell::new(format!("{temp:.1}°C")).add_attribute(comfy_table::Attribute::Bold);
        header.push(cell);
    }
    table.add_row(Cells(header));
    let now = chrono::offset::Local::now()
        .naive_local()
        .duration_trunc(TimeDelta::hours(1))
        .unwrap();
    for forecast in res
        .hourly
        .into_iter()
        .flatten()
        .skip_while(|forecast| forecast.datetime < now)
        .take(10)
    {
        let mut row: Vec<Cell> = vec![forecast.datetime.into()];
        let forecast_temp = forecast.values["temperature_2m"].value.as_f64().unwrap();
        let forecast_rh = forecast.values["relative_humidity_2m"]
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
    println!("{}\n", DOC_STR);
    println!("{table}");
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
