#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbValidation {
    /// carbonado_server: Error accessing data. Try again.
    InvalidData,
    /// iface_parse_error: You created a token of type '{0}'. We do not support this type of token.
    InvalidAssetType(String),
    /// contract_parse_error: The information '{0}' added as Contract ID is invalid.
    InvalidContract(String),
    /// invoice_parse_error: The information '{0}' added as Invoice is invalid.
    InvalidInvoice(String),
    /// utxo_parse_error: The utxo '{0}' is invalid.
    InvalidUtxo(String),
    /// fund_vault: The fund vault cannot be loaded.
    InvalidFundVault,
    /// invalid_counterpary: only bitcoin is accepted.
    InvalidCounterparty(String),
    /// public_offer_not_found: The offer '{0}' cannot be found
    NoPublicOffer(String),
    /// public_offer_unavailable: The offer '{0}' had filled or cancelled
    NoOpenOffer(String),
    /// contract_not_found: The contract '{0}' does not exist or does not be imported
    NoContract(String),
    /// utxo_not_found: The RGB wallet needs to be funded.
    NoFundVault,
    /// utxo_in_use: The utxo '{0}' already in use.
    UtxoInUse(String),
    /// contract_low: There are not enough tokens (expected: {0} / current: {1})
    NoAllocation(String, String),
    /// bitcoin_low: There are not enough sats (expected: {0} / current: {1})
    NoSats(String, String),
    /// fee_error: Currently, only fixed-value fees are supported.
    NoFee,
    /// no_keys: This operation requires keys for signing.
    NoKeys,
    /// no_assignment: The contract '{0}' does not support transfer operation.
    NoSupport(String),
    /// open_seal: You have open invoices related to the UTXO that will be spent. Spending it may result in a loss of funds.
    OpenSeals(Vec<String>),
    /// open_transfer: You have open transfers, they will soon be processed.
    OpenTransfers(Vec<String>),
    /// open_offers: You have open offers related to the UTXO that will be spent. Spending it may result in a loss
    /// of funds. offer opened: {0}.
    OpenOffers(String),
}
