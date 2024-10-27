use chrono::{DurationRound, NaiveDate, TimeDelta};
use clap::Parser;
use comfy_table::{
    presets::{ASCII_FULL_CONDENSED, UTF8_FULL_CONDENSED},
    Cell, Cells, Table,
};
use itertools::Itertools;
use open_meteo_rs::forecast::ForecastResultHourly;

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

    let this_day_and_hour = chrono::offset::Local::now()
        .naive_local()
        .duration_trunc(TimeDelta::hours(1))
        .unwrap();

    let hourly_forecast = forecast
        .hourly
        .iter()
        .flatten()
        .skip_while(|forecast| forecast.datetime < this_day_and_hour)
        .map(ForeCastItem::from_api)
        .take(10);
    let t_h = print_one_table(hourly_forecast, TableType::Hourly, ascii);
    println!("{}\n", DOC_STR);
    println!("{t_h}\n");

    let daily_groups = forecast
        .hourly
        .iter()
        .flatten()
        .chunk_by(|item| item.datetime.date());
    let daily_forcast_avg = daily_groups
        .into_iter()
        .map(|(date, group)| average_daily(date, group))
        .take(7);
    let t_d = print_one_table(daily_forcast_avg, TableType::Daily, ascii);
    println!("{t_d}");
}

fn extract_temp_rh(item: &ForecastResultHourly) -> (f64, f64) {
    (
        item.values["temperature_2m"].value.as_f64().unwrap(),
        item.values["relative_humidity_2m"].value.as_f64().unwrap(),
    )
}

fn average_daily<'a>(
    date: NaiveDate,
    forcast: impl Iterator<Item = &'a ForecastResultHourly>,
) -> ForeCastItem {
    let mut count = 0.;
    let mut temperature = 0.;
    let mut relative_humidity = 0.;
    for item in forcast {
        count += 1.;
        let (temp, rh) = extract_temp_rh(item);
        temperature += temp;
        relative_humidity += rh;
    }
    temperature /= count;
    relative_humidity /= count;
    ForeCastItem {
        datetime_repr: date.format("%A, %b %d").to_string(),
        temperature,
        relative_humidity,
    }
}

enum TableType {
    Hourly,
    Daily,
}

impl TableType {
    fn name(&self) -> &'static str {
        match self {
            TableType::Hourly => "Hourly",
            TableType::Daily => "Daily",
        }
    }
}

struct ForeCastItem {
    datetime_repr: String,
    temperature: f64,
    relative_humidity: f64,
}

impl ForeCastItem {
    fn from_api(api_item: &ForecastResultHourly) -> Self {
        let datetime_repr = format!("{}", api_item.datetime.format("%a %H:%M"));
        let (temperature, relative_humidity) = extract_temp_rh(api_item);
        Self {
            datetime_repr,
            temperature,
            relative_humidity,
        }
    }
}

fn print_one_table(
    forecast: impl Iterator<Item = ForeCastItem>,
    table_type: TableType,
    ascii: bool,
) -> Table {
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
    for forecast_item in forecast {
        let mut row: Vec<Cell> = vec![format!(
            "{datetime} ({temp:.1}°C)",
            datetime = forecast_item.datetime_repr,
            temp = forecast_item.temperature
        )
        .into()];
        let forecast_sat_pres = celsius_sat_pres(forecast_item.temperature);
        let forecast_vapor_pressure = forecast_item.relative_humidity * forecast_sat_pres;
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
