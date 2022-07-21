use csv::{ReaderBuilder, Trim, Writer};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::Read;

type ClientId = u16;
type TransactionId = u32;
type AmountType = Decimal;

#[derive(Debug, Deserialize)]
struct InputCsvRecord {
    #[serde(rename = "type")]
    record_type: String,

    client: ClientId,
    tx: TransactionId,
    amount: Option<AmountType>,
}

#[derive(Debug, Serialize)]
struct OutputCsvRecord {
    client: ClientId,

    #[serde(with = "rust_decimal::serde::str")]
    available: AmountType,

    #[serde(with = "rust_decimal::serde::str")]
    held: AmountType,

    #[serde(with = "rust_decimal::serde::str")]
    total: AmountType,

    locked: bool,
}

/// Both a File and a TcpStream can be accepted.
pub fn process_transactions_and_return_csv_result(
    csv_transaction_stream: impl Read,
) -> Result<String, Box<dyn Error>> {
    let iter_transactions = ReaderBuilder::new()
        .trim(Trim::All)
        .from_reader(csv_transaction_stream)
        .into_deserialize::<InputCsvRecord>()
        .filter_map(|result| result.map_err(|e| eprintln!("Deserialize Error: {e}")).ok())
        .filter_map(|record| {
            record
                .try_into()
                .map_err(|e| eprintln!("Validation Error: {e}"))
                .ok()
        });

    let clients = inner::process_transactions(iter_transactions);

    let csv_output = {
        let mut writer = Writer::from_writer(Vec::new());
        for output_record in clients.into_iter().map(|(id, info)| OutputCsvRecord {
            client: id,
            available: info.available,
            held: info.held,
            total: info.available + info.held,
            locked: info.locked,
        }) {
            writer.serialize(output_record)?;
        }
        String::from_utf8(writer.into_inner()?)?
    };

    Ok(csv_output)
}

mod inner;