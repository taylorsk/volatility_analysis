use chrono::{Duration, NaiveDate};
use plotters::prelude::*;

pub fn draw_accuracy_graph(
    iv_accuracy_data: Vec<(String, f64)>,
    hv_accuracy_data: Vec<(String, f64)>,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(output_path, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut all_dates: Vec<NaiveDate> = iv_accuracy_data
        .iter()
        .chain(hv_accuracy_data.iter())
        .filter_map(|(date_str, _)| NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok())
        .collect();
    all_dates.sort();
    let min_date = all_dates
        .first()
        .copied()
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    let max_date = all_dates
        .last()
        .copied()
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    let mut x_axis_weekly_ticks = Vec::new();
    let mut current_date = min_date;
    while current_date <= max_date + Duration::days(6) {
        x_axis_weekly_ticks.push(current_date);
        current_date += Duration::days(7);
    }

    let all_accuracy_values: Vec<f64> = iv_accuracy_data
        .iter()
        .map(|(_, val)| *val)
        .chain(hv_accuracy_data.iter().map(|(_, val)| *val))
        .collect();

    let min_y = all_accuracy_values
        .iter()
        .copied()
        .filter(|v| !v.is_nan())
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less))
        .unwrap_or(0.0)
        .min(0.0);
    let max_y = all_accuracy_values
        .iter()
        .copied()
        .filter(|v| !v.is_nan())
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Greater))
        .unwrap_or(0.1)
        .max(0.1);
    let y_padding = (max_y - min_y) * 0.1;
    let padded_min_y = min_y - y_padding;
    let padded_max_y = max_y + y_padding;

    let mut chart = ChartBuilder::on(&root)
        .caption("IV vs. HV Accuracy", ("sans-serif", 40).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(min_date..max_date, padded_min_y..padded_max_y)?;
    chart
        .configure_mesh()
        .x_desc("Date")
        .x_labels(x_axis_weekly_ticks.len())
        .y_desc("Accuracy Value")
        .y_labels(10)
        .y_label_formatter(&|y| format!("{:.2}", y))
        .draw()?;
    let iv_series_points: Vec<(NaiveDate, f64)> = iv_accuracy_data
        .into_iter()
        .filter_map(|(date_str, val)| {
            NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .ok()
                .and_then(|d| if !val.is_nan() { Some((d, val)) } else { None })
        })
        .collect();
    chart
        .draw_series(LineSeries::new(iv_series_points, &RED).point_size(3))?
        .label("IV Accuracy")
        .legend(|(x, y)| Rectangle::new([(x, y - 5), (x + 20, y + 5)], &RED));

    let hv_series_points: Vec<(NaiveDate, f64)> = hv_accuracy_data
        .into_iter()
        .filter_map(|(date_str, val)| {
            NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .ok()
                .and_then(|d| if !val.is_nan() { Some((d, val)) } else { None })
        })
        .collect();
    chart
        .draw_series(LineSeries::new(hv_series_points, &BLUE).point_size(3))?
        .label("HV Accuracy")
        .legend(|(x, y)| Rectangle::new([(x, y - 5), (x + 20, y + 5)], &BLUE));
    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;
    root.present()?;
    Ok(())
}
