use bdk_wallet::KeychainKind;

use crate::bitcoin::get_wallet;
use crate::constants::{CoordinatorError, get_coordinator_xpub, get_network};
use crate::structs::{NextAddressResponse, SecretString};

/// Next coordinator receive address from `COORDINATOR_XPUB`.
/// Empty xpub is [`CoordinatorError::EmptyXpub`], not a panic.
pub async fn coordinator_next_address() -> Result<NextAddressResponse, CoordinatorError> {
    let xpub = get_coordinator_xpub().await;
    if xpub.trim().is_empty() {
        return Err(CoordinatorError::EmptyXpub);
    }

    let descriptor = if xpub.contains("tr(") || xpub.contains("wpkh(") {
        xpub
    } else {
        format!("tr({xpub}/0/*)")
    };

    let wallet = get_wallet(&SecretString(descriptor), None)
        .await
        .map_err(|err| CoordinatorError::InvalidXpub(err.to_string()))?;
    let (address, keychain) = {
        let mut wallet = wallet.lock().await;
        let revealed = wallet.reveal_next_address(KeychainKind::External);
        (
            revealed.address.to_string(),
            format!("0/{}", revealed.index),
        )
    };

    Ok(NextAddressResponse {
        address,
        network: get_network().await,
        keychain,
    })
}
