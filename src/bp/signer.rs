use std::collections::HashMap;

use amplify::Wrapper;
use bpstd::psbt::{Psbt, Rejected};
use bpstd::secp256k1::{SECP256K1, ecdsa, schnorr as bip340};
use bpstd::{
    InternalKeypair, InternalPk, KeyOrigin, LegacyPk, Sighash, Sign, Signer, TapLeafHash,
    TapMerklePath, TapNodeHash, TapSighash, XOnlyPk, XkeyOrigin, Xpriv, XprivAccount,
};

#[derive(Clone)]
pub struct RefSigner<'a> {
    pub keys: HashMap<&'a XkeyOrigin, &'a Xpriv>,
    pub key_path: bool,
    pub script_path: Option<TapLeafHash>,
}

impl<'a> RefSigner<'a> {
    pub fn new(account: &'a XprivAccount) -> Self {
        Self::with(true, None, [account])
    }

    pub fn new_legacy(iter: impl IntoIterator<Item = &'a XprivAccount>) -> Self {
        Self::with(false, None, iter)
    }

    pub fn new_key_spend(iter: impl IntoIterator<Item = &'a XprivAccount>) -> Self {
        Self::with(true, None, iter)
    }

    pub fn new_script_spent(
        leaf: impl Into<TapLeafHash>,
        iter: impl IntoIterator<Item = &'a XprivAccount>,
    ) -> Self {
        Self::with(false, Some(leaf.into()), iter)
    }

    pub fn with(
        key_path: bool,
        script_path: Option<TapLeafHash>,
        iter: impl IntoIterator<Item = &'a XprivAccount>,
    ) -> Self {
        Self {
            keys: iter
                .into_iter()
                .map(|account| (account.origin(), account.xpriv()))
                .inspect(|(_origin, _xpriv)| {
                    /* if !xpriv.is_testnet() {
                        panic!(
                            "Extended private key with origin {origin} will not be used for \
                             signing since it belongs to mainnet"
                        );
                    }*/
                })
                .collect(),
            key_path,
            script_path,
        }
    }
}

impl<'a> Signer for RefSigner<'a> {
    type Sign<'s>
        = Self
    where
        Self: 's;

    fn approve(&self, _: &Psbt) -> Result<Self::Sign<'_>, Rejected> {
        Ok(self.clone())
    }
}

impl<'a> RefSigner<'a> {
    fn get(&self, origin: Option<&KeyOrigin>) -> Option<Xpriv> {
        let origin = origin?;
        self.keys.iter().find_map(|(xo, xpriv)| {
            if let Some(derivation) = xo.child_derivation(origin) {
                return Some(xpriv.derive_priv(derivation));
            }
            None
        })
    }
}

impl<'a> Sign for RefSigner<'a> {
    fn sign_ecdsa(
        &self,
        message: Sighash,
        pk: LegacyPk,
        origin: Option<&KeyOrigin>,
    ) -> Option<ecdsa::Signature> {
        if !pk.compressed {
            return None;
        }
        let xpriv = self.get(origin)?;
        let sk = xpriv.to_private_ecdsa();
        let pb = sk.public_key(SECP256K1);
        if pb != pk.pubkey {
            return None;
        }
        Some(sk.sign_ecdsa(message.into()))
    }

    fn sign_bip340_key_only(
        &self,
        message: TapSighash,
        pk: InternalPk,
        origin: Option<&KeyOrigin>,
        merkle_root: Option<TapNodeHash>,
    ) -> Option<bip340::Signature> {
        let xpriv = self.get(origin)?;
        if xpriv.to_xonly_pk() != pk.to_xonly_pk() {
            return None;
        }

        let output_pair = InternalKeypair::from(xpriv.to_keypair_bip340())
            .to_output_keypair(merkle_root)
            .0;
        if output_pair.x_only_public_key().0.serialize()
            != pk.to_output_pk(merkle_root).0.to_byte_array()
        {
            return None;
        }
        Some(output_pair.sign_schnorr(message.as_ref()))
    }

    fn sign_bip340_script_path(
        &self,
        message: TapSighash,
        pk: XOnlyPk,
        origin: Option<&KeyOrigin>,
    ) -> Option<bip340::Signature> {
        let xpriv = self.get(origin)?;
        let sk = xpriv.to_keypair_bip340();
        if sk.x_only_public_key().0 != pk.into_inner() {
            return None;
        }
        Some(sk.sign_schnorr(message.as_ref()))
    }

    fn should_sign_script_path(
        &self,
        _index: usize,
        _merkle_path: &TapMerklePath,
        leaf: TapLeafHash,
    ) -> bool {
        self.script_path == Some(leaf)
    }

    fn should_sign_key_path(&self, _index: usize) -> bool {
        self.key_path
    }
}

pub struct MainSigner {
    pub keys: Vec<XprivAccount>,
    pub key_path: bool,
    pub script_path: Option<TapLeafHash>,
}

impl MainSigner {
    pub fn new(account: XprivAccount) -> Self {
        Self::with(true, None, [account])
    }

    pub fn new_legacy(iter: impl IntoIterator<Item = XprivAccount>) -> Self {
        Self::with(false, None, iter)
    }

    pub fn new_key_spend(iter: impl IntoIterator<Item = XprivAccount>) -> Self {
        Self::with(true, None, iter)
    }

    pub fn new_script_spent(
        leaf: impl Into<TapLeafHash>,
        iter: impl IntoIterator<Item = XprivAccount>,
    ) -> Self {
        Self::with(false, Some(leaf.into()), iter)
    }

    pub fn with(
        key_path: bool,
        script_path: Option<TapLeafHash>,
        iter: impl IntoIterator<Item = XprivAccount>,
    ) -> Self {
        Self {
            keys: iter
                .into_iter()
                /*
                .inspect(|account| {
                    if !account.xpriv().is_testnet() {
                        panic!(
                            "Extended private key with origin {} will not be used for signing \
                             since it belongs to mainnet",
                            account.origin()
                        );
                    }
                })
                */
                .collect(),
            key_path,
            script_path,
        }
    }
}

impl Signer for MainSigner {
    type Sign<'s> = RefSigner<'s>;

    fn approve(&self, _: &Psbt) -> Result<Self::Sign<'_>, Rejected> {
        Ok(RefSigner::with(self.key_path, self.script_path, &self.keys))
    }
}
