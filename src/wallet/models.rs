use crate::bp::models::BtcAccount;
use crate::rgb::models::RgbAccount;
use rgbcore::ChainNet;

#[derive(Debug)]
#[allow(dead_code)]
pub struct UserAccount {
    // Common
    chain_net: ChainNet,

    // Wallets
    rgb: RgbAccount,
    bitcoin: BtcAccount,
}

impl UserAccount {
    pub fn new(chain_net: ChainNet, rgb_account: RgbAccount, btc_account: BtcAccount) -> Self {
        Self {
            chain_net,
            rgb: rgb_account,
            bitcoin: btc_account,
        }
    }
}
