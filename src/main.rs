use std::env::args;
use std::error::Error;
use std::fs::File;
use string_error::new_err;
use transaction_engine::process_transactions_and_return_csv_result;

fn main() -> Result<(), Box<dyn Error>> {
    let file = args().nth(1).ok_or(new_err(
        "One commandline argument as path to csv file is required",
    ))?;
    let file = File::open(file)?;
    let output = process_transactions_and_return_csv_result(file)?;
    print!("{output}");

    Ok(())
}

mod transaction_engine;