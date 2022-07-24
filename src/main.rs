use std::env::args;
use std::error::Error;
use std::fs::File;
use string_error::new_err;
use transaction_engine::process_csv_transactions_and_return_csv_client_states;

fn main() -> Result<(), Box<dyn Error>> {
    let file_path = args().nth(1).ok_or(new_err(
        "one commandline argument as path to csv file is required",
    ))?;
    let file = File::open(file_path)?;
    let csv_output = process_csv_transactions_and_return_csv_client_states(file)?;
    print!("{csv_output}");

    Ok(())
}

mod transaction_engine;