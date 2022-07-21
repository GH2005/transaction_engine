use super::{AmountType, ClientId, InputCsvRecord, TransactionId};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;
use std::iter::IntoIterator;
use string_error::into_err;

const DEPOSIT: &str = "deposit";
const WITHDRAWAL: &str = "withdrawal";
const DISPUTE: &str = "dispute";
const RESOLVE: &str = "resolve";
const CHARGEBACK: &str = "chargeback";

#[derive(Debug)]
pub struct Transaction {
    client: ClientId,
    tx: TransactionId,
    tx_type: TransactionType,
}

#[derive(Debug)]
pub enum TransactionType {
    Deposit(AmountType),
    Withdrawal(AmountType),
    Dispute,
    Resolve,
    Chargeback,
}
use TransactionType::*;

impl TryFrom<InputCsvRecord> for Transaction {
    type Error = Box<dyn Error>;

    /// This is input validation.
    /// I can wrap the AmountType into another struct type to use the type system to ensure it's always positive
    /// and always has at most 4 decimal positions, but it's likely an overkill.
    fn try_from(value: InputCsvRecord) -> Result<Self, Self::Error> {
        let verify_amount = |amount: Option<AmountType>| -> Result<AmountType, Self::Error> {
            match amount {
                None => Err(into_err(format!("{value:?}: no valid amount found"))),
                Some(a) => {
                    if AmountType::ZERO < a {
                        const DECIMAL_PORTION_LEN: u32 = 4;
                        Ok(a.round_dp(DECIMAL_PORTION_LEN))
                    } else {
                        Err(into_err(format!("{value:?}: amount must be positive")))
                    }
                }
            }
        };

        Ok(Transaction {
            client: value.client,
            tx: value.tx,
            tx_type: match value.record_type.as_str() {
                DEPOSIT => Deposit(verify_amount(value.amount)?),
                WITHDRAWAL => Withdrawal(verify_amount(value.amount)?),
                DISPUTE => Dispute,
                RESOLVE => Resolve,
                CHARGEBACK => Chargeback,
                _ => return Err(into_err(format!("{value:?}: unknown record_type"))),
            },
        })
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct ClientInfo {
    pub available: AmountType,
    pub held: AmountType,
    pub locked: bool,
}

/// In my opinion, the laziness of an Iterator guarantees that this function process transactions
/// as a stream. Data will not be loaded into memory as a whole at once. If a TcpStream's data rate is
/// low, this function should be synchronously blocked from time to time.
pub fn process_transactions(
    transactions: impl IntoIterator<Item = Transaction>,
) -> HashMap<ClientId, ClientInfo> {
    let mut clients = HashMap::<ClientId, ClientInfo>::new();

    type UnderDispute = bool;
    let mut deposit_transactions_seen =
        HashMap::<TransactionId, (ClientId, AmountType, UnderDispute)>::new();

    for transaction in transactions {
        let client = transaction.client;
        let tx = transaction.tx;

        let client_info = clients.entry(client).or_default();
        if client_info.locked {
            eprintln!("{transaction:?} is ignored: client is locked");
            continue;
        }

        match transaction.tx_type {
            Deposit(amount) => {
                deposit_transactions_seen.insert(tx, (client, amount, false));
                client_info.available += amount;
            }
            Withdrawal(amount) => {
                if client_info.available < amount {
                    eprintln!("{transaction:?} is ignored: not enough available funds");
                } else {
                    client_info.available -= amount;
                }
            }
            Dispute => match deposit_transactions_seen.get_mut(&tx) {
                None => {
                    eprintln!("{transaction:?} is ignored: no previous deposit transaction found");
                }
                Some(&mut (deposit_client, deposit_amount, ref mut deposit_under_dispute)) => {
                    if *deposit_under_dispute {
                        eprintln!("{transaction:?} is ignored: already under dispute");
                    } else if client != deposit_client {
                        eprintln!("{transaction:?} is ignored: the client who files the dispute is different from the one who made the deposit");
                    } else {
                        if client_info.available < deposit_amount {
                            eprintln!("{transaction:?} is ignored: can't file this dispute due to not enough available funds");
                        } else {
                            client_info.available -= deposit_amount;
                            client_info.held += deposit_amount;
                            *deposit_under_dispute = true;
                        }
                    }
                }
            },
            Resolve => match deposit_transactions_seen.get_mut(&tx) {
                None => {
                    eprintln!("{transaction:?} is ignored: no previous dispute transaction found");
                }
                Some(&mut (dispute_client, dispute_amount, ref mut deposit_under_dispute)) => {
                    if !*deposit_under_dispute {
                        eprintln!("{transaction:?} is ignored: not under dispute");
                    } else if client != dispute_client {
                        eprintln!("{transaction:?} is ignored: the client who files the resolve is different from the one who filed the dispute");
                    } else {
                        client_info.available += dispute_amount;
                        client_info.held -= dispute_amount;
                        *deposit_under_dispute = false;
                    }
                }
            },
            Chargeback => match deposit_transactions_seen.get(&tx) {
                None => {
                    eprintln!("{transaction:?} is ignored: no previous dispute transaction found");
                }
                Some(&(dispute_client, dispute_amount, deposit_under_dispute)) => {
                    if !deposit_under_dispute {
                        eprintln!("{transaction:?} is ignored: not under dispute");
                    } else if client != dispute_client {
                        eprintln!("{transaction:?} is ignored: the client who files the chargeback is different from the one who filed the dispute");
                    } else {
                        client_info.held -= dispute_amount;
                        client_info.locked = true;
                        deposit_transactions_seen.remove(&tx);
                    }
                }
            },
        }
    }

    clients
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_and_withdrawal() {
        let clients = process_transactions([
            Transaction {
                client: 3,
                tx: 2,
                tx_type: Deposit(AmountType::from_str_exact("2.3456").unwrap()),
            },
            Transaction {
                client: 1,
                tx: 1,
                tx_type: Deposit(AmountType::from_str_exact("10.3").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 5,
                tx_type: Deposit(AmountType::from_str_exact("0.0001").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 4,
                tx_type: Withdrawal(AmountType::from_str_exact("1.1").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 6,
                tx_type: Withdrawal(AmountType::from_str_exact("100.1").unwrap()),
            },
        ]);

        assert_eq!(
            clients,
            [
                (
                    3,
                    ClientInfo {
                        available: AmountType::from_str_exact("1.2457").unwrap(),
                        held: AmountType::ZERO,
                        locked: false,
                    }
                ),
                (
                    1,
                    ClientInfo {
                        available: AmountType::from_str_exact("10.3").unwrap(),
                        held: AmountType::ZERO,
                        locked: false,
                    }
                ),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn test_dispute() {
        let clients = process_transactions([
            Transaction {
                client: 3,
                tx: 2,
                tx_type: Deposit(AmountType::from_str_exact("2.3456").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 4,
                tx_type: Withdrawal(AmountType::from_str_exact("2").unwrap()),
            },
            Transaction {
                client: 4,
                tx: 2,
                tx_type: Dispute,
            },
            Transaction {
                client: 3,
                tx: 100,
                tx_type: Dispute,
            },
            Transaction {
                client: 3,
                tx: 2,
                tx_type: Dispute,
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Deposit(AmountType::from_str_exact("5.4321").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Dispute,
            },
        ]);

        assert_eq!(
            clients,
            [
                (
                    3,
                    ClientInfo {
                        available: AmountType::from_str_exact("0.3456").unwrap(),
                        held: AmountType::from_str_exact("5.4321").unwrap(),
                        locked: false,
                    }
                ),
                (
                    4,
                    ClientInfo {
                        available: AmountType::ZERO,
                        held: AmountType::ZERO,
                        locked: false,
                    }
                ),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn test_resolve() {
        let clients = process_transactions([
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Deposit(AmountType::from_str_exact("5.4321").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Resolve,
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Dispute,
            },
            Transaction {
                client: 4,
                tx: 10,
                tx_type: Resolve,
            },
            Transaction {
                client: 3,
                tx: 200,
                tx_type: Resolve,
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Resolve,
            },
        ]);

        assert_eq!(
            clients,
            [
                (
                    3,
                    ClientInfo {
                        available: AmountType::from_str_exact("5.4321").unwrap(),
                        held: AmountType::ZERO,
                        locked: false,
                    }
                ),
                (
                    4,
                    ClientInfo {
                        available: AmountType::ZERO,
                        held: AmountType::ZERO,
                        locked: false,
                    }
                ),
            ]
            .into_iter()
            .collect()
        );
    }
    #[test]
    fn test_chargeback() {
        let clients = process_transactions([
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Deposit(AmountType::from_str_exact("5.4321").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Chargeback,
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Dispute,
            },
            Transaction {
                client: 4,
                tx: 10,
                tx_type: Chargeback,
            },
            Transaction {
                client: 3,
                tx: 200,
                tx_type: Chargeback,
            },
            Transaction {
                client: 3,
                tx: 10,
                tx_type: Chargeback,
            },
            Transaction {
                client: 3,
                tx: 11,
                tx_type: Deposit(AmountType::from_str_exact("5.4321").unwrap()),
            },
            Transaction {
                client: 3,
                tx: 12,
                tx_type: Deposit(AmountType::from_str_exact("5.4321").unwrap()),
            },
        ]);

        assert_eq!(
            clients,
            [
                (
                    3,
                    ClientInfo {
                        available: AmountType::ZERO,
                        held: AmountType::ZERO,
                        locked: true,
                    }
                ),
                (
                    4,
                    ClientInfo {
                        available: AmountType::ZERO,
                        held: AmountType::ZERO,
                        locked: false,
                    }
                ),
            ]
            .into_iter()
            .collect()
        );
    }
}