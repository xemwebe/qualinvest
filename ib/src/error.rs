use std::num::{ParseFloatError, ParseIntError};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unvollständige oder fehlerhafte Kauftransaktion")]
    InvalidBuyTransaction,
    #[error("Unvollständige oder fehlerhafte Verkaufstransaktion")]
    InvalidSellTransaction,
    #[error("Wechselkurs für {0} nicht gefunden")]
    CurrencyNotFound(String),
    #[error("Fehler beim Lesen der CSV-Datei: {0}")]
    Csv(#[from] csv::Error),
    #[error("Record nicht gefunden in CSV-Datei")]
    RecordNotFound,
    #[error("Umwandeln von String in Zahl fehlgeschlagen")]
    ParsingNumberFailed,
    #[error("Umwandeln von String in Datum fehlgeschlagen")]
    FailedToParseDate,
    #[error("Umwandeln von Timestamp in Datum fehlgeschlagen")]
    FailedToConvertDate,
    #[error("Symbol {0} nicht gefunden in Finanzinstrumenten")]
    SymbolNotFound(String),
    #[error("Ungültiger Montsname {0}")]
    InvalidMonthName(String),
    #[error("Invalid integer: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("Datum des Kontoauszugs konnte nicht gefunden werden")]
    DateNotFound,
    #[error("Parsen eines Symbols von einer Beschreibung ist fehlgeschlagen")]
    FailedToParseSymbolsFromDescription,
    #[error("Parsen der Kapitalmaßnahme aus Bechreibung '{0}' fehlgeschlagen")]
    FailedToParseKapitalmaßnahme(String),
    #[error("Parsen der Jurisdiktion aus Bechreibung '{0}' fehlgeschlagen")]
    FailedToParseJurisdiction(String),
    #[error("Handelsmenge is leer")]
    LeereMenge,
    #[error("FIFO Stand ist aktueller als Verkaufsdatum")]
    FifoIstNeuer,
    #[error("Leerverkäufe werden nicht unterstützt:")]
    KeineLeerverkäufe,
    #[error("Kontoauszug für das Jahr {0} fehlt")]
    KontoauszugFehlt(u32),
    #[error("Zeile mit Gesamtzinsen in EUR nicht gefunden")]
    GesamtZinsenNichtGefunden,
    #[error("Deserialisation fehlgeschlagen: {0}")]
    DesirialisationFehlgeschlagen(#[from] serde_json::Error),
    #[error("Cash Bericht muss 5 Spalten enthalten")]
    CashBerichtUngültig,
    #[error("Parsen einer Fließkommazahl fehlgeschlagen: {0}")]
    ParseFloatFailed(#[from] ParseFloatError),
    #[error("IO-Fehler: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Startposition für ETF {0} nicht gefunden")]
    PassendeEtfStartPositionFehlt(String),
    #[error("Basisrate für das Jahr {0} nicht gefunden")]
    BasisRateFehlt(u32),
    #[error("ETF Endposition enthält mehr als eine Positions für Symbol {0}")]
    EtfEndPositionUngleichEins(String),
    #[error("Failed to parse header of table group")]
    FailedToParseHeaderOfTable(String),
}

pub type Result<T> = std::result::Result<T, Error>;
