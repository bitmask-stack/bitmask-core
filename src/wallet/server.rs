#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum ServerErrors {
    /// Retrieve '{0}' carbonado causes error. {1}
    CarbonadoRetrieve(String, String),
    /// Write '{0}' carbonado causes error. {1}
    CarbonadoWrite(String, String),
    /// Decode '{0}' causes error. {1}
    Decode(String, String),
    /// Encode '{0}' causes error. {1}
    Encode(String, String),
    /// File '{0}' not found.
    NoFile(String),
}
