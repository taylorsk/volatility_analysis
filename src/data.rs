use crate::api::{Ohlcv, OptionsData};
use chrono::NaiveDate;
use std::collections::HashMap;

pub fn find_closest_date(dates: &[NaiveDate], target_date: NaiveDate) -> Option<NaiveDate> {
    if dates.is_empty() {
        return None;
    }

    let mut closest_date = dates[0];
    let mut min_duration = (dates[0] - target_date).abs();
    for &date in dates.iter().skip(1) {
        let current_duration = (date - target_date).abs();
        if current_duration < min_duration {
            min_duration = current_duration;
            closest_date = date;
        }
    }

    Some(closest_date)
}

pub fn find_closest_num(values: &[f64], target_value: f64) -> Option<f64> {
    values
        .iter()
        .min_by(|&&a, &&b| {
            let diff_a = (a - target_value).abs();
            let diff_b = (b - target_value).abs();

            diff_a
                .partial_cmp(&diff_b)
                .unwrap_or(std::cmp::Ordering::Greater)
        })
        .copied()
}
// calculate historical volatility
pub fn historical_volatility(data: &[Ohlcv], window: usize) -> Vec<Option<f64>> {
    if window < 2 || data.len() < window {
        return vec![None; data.len()];
    }

    let mut log_returns = vec![None];
    for i in 1..data.len() {
        let ret = (data[i].close / data[i - 1].close).ln();
        log_returns.push(Some(ret));
    }

    let mut volatility = vec![None; window - 1];
    for i in window..log_returns.len() {
        let window_returns: Vec<f64> = log_returns[i - window + 1..=i]
            .iter()
            .filter_map(|&x| x)
            .collect();

        if window_returns.len() == window {
            let mean = window_returns.iter().sum::<f64>() / window as f64;
            let variance = window_returns
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / window as f64;
            let std_dev = variance.sqrt();
            volatility.push(Some(std_dev * (252f64).sqrt()));
        } else {
            volatility.push(None);
        }
    }

    volatility
}
// Test for HV and IV accuracy. We compare the HV/IV 'prediction' to the next date.
pub fn iv_accuracy(
    option_data: &[OptionsData],
    ohlcv_data: &[Ohlcv],
    window: usize,
) -> Vec<(String, f64)> {
    let mut accuracy_series: Vec<(String, f64)> = Vec::new();

    for option_datum in option_data {
        let start_index = ohlcv_data.iter().position(|d| d.date == option_datum.date);

        if let Some(start_idx) = start_index {
            let end_index = start_idx.checked_add(window);

            if let Some(end_idx) = end_index {
                if end_idx < ohlcv_data.len() {
                    let s_t = ohlcv_data[start_idx].close;
                    let s_t_plus_w = ohlcv_data[end_idx].close;

                    if s_t > 0.0 && s_t_plus_w > 0.0 {
                        let actual_magnitude = (s_t_plus_w - s_t).abs();
                        let implied_volatility = option_datum.implied_volatility;
                        let time_factor = (window as f64 / 252.0).sqrt();
                        let expected_magnitude = s_t * implied_volatility * time_factor;

                        accuracy_series.push((
                            option_datum.date.clone(),
                            actual_magnitude - expected_magnitude,
                        ));
                    }
                }
            }
        }
    }
    accuracy_series
}

pub fn hv_accuracy(ohlcv_data: &[Ohlcv], window: usize) -> Vec<(String, f64)> {
    let mut accuracy_series: Vec<(String, f64)> = Vec::new();

    let hv_series = historical_volatility(ohlcv_data, window);

    for (start_idx, ohlcv_entry) in ohlcv_data.iter().enumerate() {
        if let Some(Some(hv)) = hv_series.get(start_idx) {
            let end_index = start_idx.checked_add(window);

            if let Some(end_idx) = end_index {
                if end_idx < ohlcv_data.len() {
                    let s_t = ohlcv_entry.close;
                    let s_t_plus_w = ohlcv_data[end_idx].close;

                    if s_t > 0.0 && s_t_plus_w > 0.0 {
                        let actual_magnitude = (s_t_plus_w - s_t).abs();
                        let time_factor = (window as f64 / 252.0).sqrt();
                        let expected_magnitude = s_t * hv * time_factor;

                        accuracy_series.push((
                            ohlcv_entry.date.clone(),
                            actual_magnitude - expected_magnitude,
                        ));
                    }
                }
            }
        }
    }
    accuracy_series
}

pub fn calculate_mae(accuracy_data: &[(String, f64)]) -> Option<f64> {
    let mut total_abs_diff = 0.0;
    let mut count = 0;

    for (_, value) in accuracy_data {
        total_abs_diff += value.abs();
        count += 1;
    }

    if count > 0 {
        Some(total_abs_diff / count as f64)
    } else {
        None
    }
}

pub fn calculate_accuracy_correlation(
    iv_accuracy_data: &[(String, f64)],
    hv_accuracy_data: &[(String, f64)],
) -> Option<f64> {
    let iv_map: HashMap<String, f64> = iv_accuracy_data.iter().cloned().collect();
    let hv_map: HashMap<String, f64> = hv_accuracy_data.iter().cloned().collect();

    let mut paired_data: Vec<(f64, f64)> = Vec::new();

    let mut common_dates: Vec<String> = iv_map.keys().cloned().collect();
    common_dates.retain(|date| hv_map.contains_key(date));
    common_dates.sort();

    for date in common_dates {
        if let (Some(&iv_val), Some(&hv_val)) = (iv_map.get(&date), hv_map.get(&date)) {
            paired_data.push((iv_val, hv_val));
        }
    }

    if paired_data.len() < 2 {
        return None;
    }

    let n = paired_data.len() as f64;

    let sum_x: f64 = paired_data.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = paired_data.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = paired_data.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = paired_data.iter().map(|(x, _)| x.powi(2)).sum();
    let sum_y2: f64 = paired_data.iter().map(|(_, y)| y.powi(2)).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator_x = (n * sum_x2 - sum_x.powi(2)).sqrt();
    let denominator_y = (n * sum_y2 - sum_y.powi(2)).sqrt();

    if denominator_x == 0.0 || denominator_y == 0.0 {
        return Some(0.0);
    }

    Some(numerator / (denominator_x * denominator_y))
}
