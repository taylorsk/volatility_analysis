use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use tokio;
mod api;
mod data;
mod graph;
use crate::api::{historical_data, options_data, Ohlcv, OptionsData};
use crate::data::{
    calculate_accuracy_correlation, calculate_mae, find_closest_date, find_closest_num,
    hv_accuracy, iv_accuracy,
};
use crate::graph::draw_accuracy_graph;
use chrono::{Duration, NaiveDate};

// Let's just say i was vibing while 'coding' most of this
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let key = "".to_string(); // AlphaVantage API key
    let symbol = "SPY".to_string();
    //These are the windows. You may want to change the max_options_requests and
    //fetch_interval_days if you're changing these.
    let hv_window_days = 30;
    let iv_option_target_window_days = 30;

    let max_options_requests = 24; // Limit for options_data calls, necessary for free api
    let fetch_interval_days = 14; // Fetch options data every 2 weeks, cus cant get every day

    let mut ohlcv_data = historical_data(key.clone(), symbol.clone())
        .await
        .expect("Failed to fetch historical OHLCV data.");

    if ohlcv_data.is_empty() {
        eprintln!(
            "No OHLCV data fetched for {}. Cannot proceed with calculations.",
            symbol
        );
        return Ok(());
    }

    let mut latest_date: Option<NaiveDate> = None;
    for ohlcv_entry in ohlcv_data.iter() {
        if let Ok(date) = NaiveDate::parse_from_str(&ohlcv_entry.date, "%Y-%m-%d") {
            if latest_date.is_none() || date > latest_date.unwrap() {
                latest_date = Some(date);
            }
        }
    }

    let Some(latest_date_actual) = latest_date else {
        eprintln!("Could not determine the latest date from OHLCV data.");
        return Ok(());
    };

    let twelve_months_ago = latest_date_actual - Duration::days(365);

    ohlcv_data.retain(|ohlcv| {
        if let Ok(date) = NaiveDate::parse_from_str(&ohlcv.date, "%Y-%m-%d") {
            date >= twelve_months_ago && date <= latest_date_actual // Ensure it's within the 12-month window
        } else {
            false // If date parsing fails, exclude the entry
        }
    });

    // Optional: Sort the data by date if it's not already,
    // which can be helpful for sequential processing later.
    ohlcv_data.sort_by(|a, b| {
        NaiveDate::parse_from_str(&a.date, "%Y-%m-%d")
            .unwrap_or_default()
            .cmp(&NaiveDate::parse_from_str(&b.date, "%Y-%m-%d").unwrap_or_default())
    });

    println!(
        "Filtered OHLCV data to the most recent 12 months, {} entries remaining (from {} to {}).",
        ohlcv_data.len(),
        twelve_months_ago,
        latest_date_actual
    );

    let hv_accuracy_full_results = hv_accuracy(&ohlcv_data, hv_window_days);
    println!(
        "\nHV Accuracy (first 100 entries): {:?}",
        hv_accuracy_full_results
            .iter()
            .take(100)
            .collect::<Vec<_>>()
    );

    let ohlcv_map: HashMap<NaiveDate, &Ohlcv> = ohlcv_data
        .iter()
        .filter_map(|ohlcv| {
            NaiveDate::parse_from_str(&ohlcv.date, "%Y-%m-%d")
                .ok()
                .map(|d| (d, ohlcv))
        })
        .collect();

    let mut all_relevant_options: Vec<OptionsData> = Vec::new();
    let mut fetched_options_dates: HashSet<NaiveDate> = HashSet::new(); // Track dates for which options are successfully processed

    let mut options_requests_count = 0;
    let mut last_fetch_date: Option<NaiveDate> = None;

    for ohlcv_entry in ohlcv_data.iter() {
        if options_requests_count >= max_options_requests {
            println!(
                "\nMax options requests ({}) reached. Stopping.",
                max_options_requests
            );
            break; // Exit the loop if we've hit our request limit
        }

        let current_date_naive = match NaiveDate::parse_from_str(&ohlcv_entry.date, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => {
                eprintln!(
                    "Could not parse OHLCV date: {}. Skipping.",
                    ohlcv_entry.date
                );
                continue;
            }
        };

        // If we've already processed options for this date, skip it.
        if fetched_options_dates.contains(&current_date_naive) {
            println!(
                "Skipping {}: Options for this date already processed.",
                current_date_naive.format("%Y-%m-%d")
            );
            continue;
        }

        // Implement the bi-weekly fetch logic
        let should_fetch = match last_fetch_date {
            Some(last_date) => {
                let duration = current_date_naive.signed_duration_since(last_date);
                duration.num_days() >= fetch_interval_days as i64
            }
            None => true, // Always fetch the first available date
        };

        if !should_fetch {
            println!(
                "Skipping {}: Not yet two weeks since last options fetch.",
                current_date_naive.format("%Y-%m-%d")
            );
            continue;
        }

        let current_date_str = ohlcv_entry.date.clone();
        let current_ohlcv_close = ohlcv_entry.close;

        let Some(target_expiration_date) = current_date_naive
            .checked_add_signed(Duration::days(iv_option_target_window_days as i64))
        else {
            println!(
                "Skipping {}: Could not calculate target expiration date.",
                current_date_str
            );
            continue;
        };

        println!(
            "Fetching options for {} (Request {}/{})",
            current_date_str,
            options_requests_count + 1,
            max_options_requests
        );
        let options_chain_result =
            options_data(key.clone(), symbol.clone(), current_date_str.clone()).await;
        options_requests_count += 1; // Increment immediately after calling the API

        let Ok(mut options_chain_for_day) = options_chain_result else {
            eprintln!(
                "Skipping {}: Error fetching options data: {:?}",
                current_date_str,
                options_chain_result.unwrap_err()
            );
            continue;
        };

        if options_chain_for_day.is_empty() {
            println!(
                "Skipping {}: No options data found for this date after fetch.",
                current_date_str
            );
            continue;
        }

        let available_expirations: Vec<NaiveDate> = options_chain_for_day
            .iter()
            .filter_map(|opt| NaiveDate::parse_from_str(&opt.expiration, "%Y-%m-%d").ok())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let Some(closest_expiration_date) =
            find_closest_date(&available_expirations, target_expiration_date)
        else {
            println!(
                "Skipping {}: No closest expiration date found (target: {}). Available expirations: {:?}",
                current_date_str, target_expiration_date, available_expirations
            );
            continue;
        };

        if !ohlcv_map.contains_key(&closest_expiration_date) {
            println!(
                "Skipping options from {} targeting expiry {}: No OHLCV data for this actual expiry date. Cannot calculate realized volatility.",
                current_date_str, closest_expiration_date
            );
            continue;
        }

        let options_at_closest_expiry: Vec<OptionsData> = options_chain_for_day
            .drain(..)
            .filter(|opt| {
                NaiveDate::parse_from_str(&opt.expiration, "%Y-%m-%d").ok()
                    == Some(closest_expiration_date)
            })
            .collect();

        if options_at_closest_expiry.is_empty() {
            println!(
                "Skipping {}: No options found for closest expiration date {}.",
                current_date_str, closest_expiration_date
            );
            continue;
        }

        let mut available_strikes_temp: Vec<f64> = options_at_closest_expiry
            .iter()
            .map(|opt| opt.strike)
            .collect();

        available_strikes_temp.sort_unstable_by(|a, b| {
            a.partial_cmp(b)
                .expect("Strike prices should be comparable and not NaN")
        });

        available_strikes_temp.dedup();

        let available_strikes: Vec<f64> = available_strikes_temp;
        let Some(closest_strike) = find_closest_num(&available_strikes, current_ohlcv_close) else {
            println!(
                "Skipping {}: No closest strike found (target close: {}). Available strikes: {:?}",
                current_date_str, current_ohlcv_close, available_strikes
            );
            continue;
        };

        if let Some(target_option) = options_at_closest_expiry.into_iter().find(|opt| {
            opt.strike == closest_strike
                && NaiveDate::parse_from_str(&opt.expiration, "%Y-%m-%d").ok()
                    == Some(closest_expiration_date)
        }) {
            all_relevant_options.push(target_option.clone());
            fetched_options_dates.insert(current_date_naive);
            last_fetch_date = Some(current_date_naive); // Update the last fetch date
            println!(
                "Successfully processed option for {}. Total relevant options: {}",
                current_date_str,
                all_relevant_options.len()
            );
        } else {
            println!(
                "Skipping {}: Could not find a specific option matching closest strike {} and expiry {}.",
                current_date_str, closest_strike, closest_expiration_date
            );
        }
    }

    if options_requests_count >= max_options_requests {
        println!("\nMax options requests ({}) reached.", max_options_requests);
    }
    println!(
        "\nTotal options_data requests made: {}",
        options_requests_count
    );
    println!(
        "Total relevant options collected for IV accuracy: {}",
        all_relevant_options.len()
    );

    let iv_accuracy_results = iv_accuracy(
        &all_relevant_options,
        &ohlcv_data,
        iv_option_target_window_days,
    );

    println!(
        "\nIV Accuracy (first 100 entries, based on {} options): {:?}",
        all_relevant_options.len(),
        iv_accuracy_results.iter().take(100).collect::<Vec<_>>()
    );

    // Filter HV accuracy results to match the dates present in iv_accuracy_results
    let iv_dates: HashSet<NaiveDate> = iv_accuracy_results
        .iter()
        .map(|(date_str, _)| NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap())
        .collect();

    let hv_accuracy_filtered_results: Vec<(String, f64)> = hv_accuracy_full_results
        .into_iter()
        .filter(|(date_str, _)| {
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_or(false, |date| iv_dates.contains(&date))
        })
        .collect();

    println!(
        "\nFiltered HV Accuracy (first 100 entries): {:?}",
        hv_accuracy_filtered_results
            .iter()
            .take(100)
            .collect::<Vec<_>>()
    );
    if let Some(mae) = calculate_mae(&iv_accuracy_results) {
        println!("Mean Absolute Error (MAE) of IV: {:.4}", mae);
    } else {
        println!("Could not calculate MAE. Not enough common data points.");
    }

    if let Some(mae) = calculate_mae(&hv_accuracy_filtered_results) {
        println!("Mean Absolute Error (MAE) of HV: {:.4}", mae);
    } else {
        println!("Could not calculate MAE. Not enough common data points.");
    }

    if let Some(correlation) =
        calculate_accuracy_correlation(&iv_accuracy_results, &hv_accuracy_filtered_results)
    {
        println!("Correlation between IV and HV accuracy: {:.4}", correlation);
    } else {
        println!("Could not calculate correlation. Not enough common data points.");
    }

    if !iv_accuracy_results.is_empty() || !hv_accuracy_filtered_results.is_empty() {
        let output_file = "accuracy_comparison.png";
        match draw_accuracy_graph(
            iv_accuracy_results,
            hv_accuracy_filtered_results,
            output_file,
        ) {
            Ok(_) => println!("Graph generated successfully at {}", output_file),
            Err(e) => eprintln!("Failed to generate graph: {}", e),
        }
    } else {
        println!("No data for IV or HV accuracy, skipping graph generation.");
    }
    // Assuming you have iv_accuracy_data and hv_accuracy_data available

    Ok(())
}
