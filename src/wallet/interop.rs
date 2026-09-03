use crate::rgb::models::{AssetParseError, AssetType};
use bpstd::{
    DerivationParseError, DerivationPath, HardenedIndex, IdxBase, Keychain, Network,
    OriginParseError, XkeyOrigin, XkeyParseError, Xpriv, XprivAccount, Xpub, XpubDerivable,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Eq, PartialEq, Debug, Display, Error)]
#[display(doc_comments)]
pub enum BpInteropError {
    /// invalid derivation format '{0}'
    InvalidDerivation(DerivationParseError),
    /// invalid xpub format '{0}'
    InvalidKey(XkeyParseError),
    /// invalid origin format '{0}'
    InvalidOrigin(OriginParseError),
    /// invalid legacy key format '{0}'
    LegacyParse(String),
    /// invalid key format '{0}'
    Parse(String),
}

#[derive(Clone, Debug, Display)]
pub enum WalletType {
    #[display("taproot")]
    Taproot,
    #[display("segwit")]
    SegWit,
}

#[derive(Clone, Debug, Display)]
pub enum DerivationType {
    #[display("all")]
    All,
    #[display("bitcoin")]
    Bitcoin,
    #[display("rgb")]
    RGB,
}

#[derive(Clone, Debug, Display)]
#[display("{xpub}")]
pub struct XpubKeyBp {
    pub xpub: XpubDerivable,
}

impl XpubKeyBp {
    pub fn new(
        xdesc: &str,
        network: Network,
        ty: WalletType,
        dt: DerivationType,
    ) -> Result<Self, BpInteropError> {
        if xdesc.contains("tr") || xdesc.contains("wpkh") || xdesc.contains('[') {
            XpubKeyBp::from_xpub_bdk(xdesc, network, ty, dt)
        } else {
            XpubKeyBp::from_xpub(xdesc, network, ty, dt)
        }
    }

    fn from_xpub_bdk(
        xdesc: &str,
        network: Network,
        ty: WalletType,
        dt: DerivationType,
    ) -> Result<Self, BpInteropError> {
        let descr_format: String = xdesc.replace("tr(", "").replace("wpkh(", "");
        if let Some((origin, xpub)) = descr_format.trim_start_matches('[').split_once(']')
            && let Some((xpub, _)) = xpub.split_once('/')
        {
            let xpub: Xpub = xpub.parse().map_err(BpInteropError::InvalidKey)?;
            let origin: XkeyOrigin = origin.parse().map_err(BpInteropError::InvalidOrigin)?;
            let schema = match ty {
                WalletType::Taproot => "86h",
                WalletType::SegWit => "84h",
            };

            let account: u8 = network.is_testnet().into();
            let account = format!("{schema}/{account}h/0h");
            let derivation: DerivationPath<HardenedIndex> =
                account.parse().map_err(BpInteropError::InvalidDerivation)?;

            let fp = origin.master_fp();
            let origin = XkeyOrigin::new(fp, derivation);
            let terminals = match dt {
                DerivationType::All => "0;1;9;10;20;21;25;30",
                DerivationType::Bitcoin => "0;1",
                DerivationType::RGB => "9;10",
            };

            let desc = format!("[{origin}]{xpub}/<{terminals}>/*");
            let desc = XpubDerivable::from_str(&desc)
                .map_err(|err| BpInteropError::Parse(err.to_string()))?;
            return Ok(Self { xpub: desc });
        }
        Err(BpInteropError::LegacyParse(
            "invalid descriptor format".to_string(),
        ))
    }

    fn from_xpub(
        xpub: &str,
        network: Network,
        ty: WalletType,
        dt: DerivationType,
    ) -> Result<Self, BpInteropError> {
        let xpub = Xpub::from_str(xpub).map_err(|err| BpInteropError::Parse(err.to_string()))?;
        let fp = xpub.fingerprint();
        let schema = match ty {
            WalletType::Taproot => "86h",
            WalletType::SegWit => "84h",
        };

        let account: u8 = network.is_testnet().into();
        let account = format!("{schema}/{account}h/0h");
        let derivation: DerivationPath<HardenedIndex> =
            account.parse().map_err(BpInteropError::InvalidDerivation)?;

        let origin = XkeyOrigin::new(fp, derivation);
        let terminals = match dt {
            DerivationType::All => "0;1;9;10;20;21;25",
            DerivationType::Bitcoin => "0;1",
            DerivationType::RGB => "9;10",
        };

        let desc = format!("[{origin}]{xpub}/<{terminals}>/*");
        let desc =
            XpubDerivable::from_str(&desc).map_err(|err| BpInteropError::Parse(err.to_string()))?;
        let desc = Self { xpub: desc };

        Ok(desc)
    }
}

#[derive(Clone, Getters, Debug, Display)]
#[display("[{origin}]{xpriv}")]
pub struct XprivBpKey {
    origin: XkeyOrigin,
    xpriv: Xpriv,
    ty: WalletType,
}

impl XprivBpKey {
    pub fn new(xacc_priv: &str, _: Network) -> Result<Self, BpInteropError> {
        let ty = if xacc_priv.contains("tr") {
            WalletType::Taproot
        } else {
            WalletType::SegWit
        };
        if xacc_priv.contains("tr") || xacc_priv.contains("wpkh") || xacc_priv.contains('[') {
            XprivBpKey::from_xpriv_bdk(xacc_priv)
        } else {
            XprivBpKey::from_xpriv(xacc_priv, &ty)
        }
    }

    fn from_xpriv_bdk(xdesc: &str) -> Result<Self, BpInteropError> {
        let ty = if xdesc.contains("wpkh") {
            WalletType::SegWit
        } else {
            WalletType::Taproot
        };

        let descr_format: String = xdesc.replace("tr(", "").replace("wpkh(", "");
        if let Some((origin, xpriv)) = descr_format.trim_start_matches('[').split_once(']')
            && let Some((xpriv, _)) = xpriv.split_once('/')
        {
            let xpriv: Xpriv = xpriv.parse().map_err(BpInteropError::InvalidKey)?;

            let origin: XkeyOrigin = origin.parse().map_err(BpInteropError::InvalidOrigin)?;
            return Ok(Self { xpriv, origin, ty });
        }
        Err(BpInteropError::LegacyParse(
            "invalid descriptor format".to_string(),
        ))
    }

    fn from_xpriv(xpriv_account: &str, ty: &WalletType) -> Result<Self, BpInteropError> {
        let xpriv_account = XprivAccount::from_str(xpriv_account)
            .map_err(|err| BpInteropError::Parse(err.to_string()))?;

        let desc = Self {
            xpriv: *xpriv_account.xpriv(),
            origin: xpriv_account.origin().clone(),
            ty: ty.clone(),
        };
        Ok(desc)
    }
}

#[derive(Display, Getters, Eq, PartialEq, Serialize, Deserialize, Debug, Clone, Default)]
#[display("{keychain}")]
pub struct WalletTerminal {
    keychain: Keychain,
}

impl WalletTerminal {
    pub fn is_bitcoin(self) -> bool {
        [0, 1].contains(&self.keychain.index())
    }

    pub fn is_rgb(self) -> bool {
        [9, 10].contains(&self.keychain.index())
    }

    pub fn to_alt(self) -> Keychain {
        if self.is_rgb() {
            Keychain::with(9)
        } else {
            Keychain::with(1)
        }
    }
}

impl FromStr for WalletTerminal {
    type Err = AssetParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match AssetType::from_str(s) {
            Ok(asset) => Ok(Self {
                keychain: asset.to_keychain(),
            }),
            _ => Err(AssetParseError::NoSupport(s.to_string())),
        }
    }
}
