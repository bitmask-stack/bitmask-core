// Methods meant to work with BDK defined within the web::bitcoin module from bitmask-core:
// https://github.com/diba-io/bitmask-core/blob/development/src/web.rs

import * as BMC from "./bitmask_core";

export const hashPassword = (password: string) => BMC.hash_password(password);

export const decryptWallet = async (
  hash: string,
  encryptedDescriptors: string,
  seedPassword = ""
): Promise<Vault> =>
  JSON.parse(await BMC.decrypt_wallet(hash, encryptedDescriptors, seedPassword));

export const upgradeWallet = async (
  hash: string,
  encryptedDescriptors: string,
  seedPassword = ""
): Promise<string> =>
  JSON.parse(
    await BMC.upgrade_wallet(hash, encryptedDescriptors, seedPassword)
  );

export const syncWallets = async (): Promise<void> => BMC.sync_wallets();

export const newWallet = async (
  hash: string,
  seedPassword: string
): Promise<string> => JSON.parse(await BMC.new_wallet(hash, seedPassword));

export const encryptWallet = async (
  mnemonic: string,
  hash: string,
  seedPassword: string
): Promise<string> =>
  JSON.parse(await BMC.encrypt_wallet(mnemonic, hash, seedPassword));

export const getWalletData = async (
  descriptor: string,
  changeDescriptor?: string
): Promise<WalletData> =>
  JSON.parse(await BMC.get_wallet_data(descriptor, changeDescriptor));

export const getNewAddress = async (
  descriptor: string,
  changeDescriptor?: string
): Promise<string> =>
  JSON.parse(await BMC.get_new_address(descriptor, changeDescriptor));

export const sendSats = async (
  descriptor: string,
  changeDescriptor: string,
  address: string,
  amount: bigint,
  feeRate: number,
  broadcast = true
): Promise<TransactionData> =>
  JSON.parse(
    await BMC.send_sats(
      descriptor,
      changeDescriptor,
      address,
      amount,
      broadcast,
      feeRate
    )
  );

export const fundVault = async (
  descriptor: string,
  changeDescriptor: string,
  rgbAddress: string,
  feeRate: number,
  broadcast = true
): Promise<FundVaultDetails> =>
  JSON.parse(
    await BMC.fund_vault(
      descriptor,
      changeDescriptor,
      rgbAddress,
      broadcast,
      feeRate
    )
  );

export const getAssetsVault = async (
  rgbAssetsDescriptorXpub: string,
  _rgbUdasDescriptorXpub?: string
): Promise<FundVaultDetails> =>
  JSON.parse(await BMC.get_assets_vault(rgbAssetsDescriptorXpub));

export const drainWallet = async (
  destination: string,
  descriptor: string,
  changeDescriptor?: string,
  feeRate?: number
): Promise<TransactionData> =>
  JSON.parse(
    await BMC.drain_wallet(destination, descriptor, changeDescriptor, feeRate)
  );

export const bumpFee = async (
  txid: string,
  feeRate: number,
  broadcast: boolean,
  descriptor: string,
  changeDescriptor?: string,
): Promise<TransactionData> =>
  JSON.parse(
    await BMC.bump_fee(txid, feeRate, descriptor, changeDescriptor, broadcast)
  );

// Core type interfaces based on structs defined within the bitmask-core Rust crate:
// https://github.com/diba-io/bitmask-core/blob/development/src/structs.rs

export interface PrivateWalletData {
  xprvkh: string;
  btcDescriptorXprv: string;
  btcChangeDescriptorXprv: string;
  rgbAssetsDescriptorXprv: string;
  rgbUdasDescriptorXprv: string;
  nostrPrv: string;
  nostrNsec: string;
}

export interface PublicWalletData {
  xpub: string;
  xpubkh: string;
  watcherXpub: string;
  btcDescriptorXpub: string;
  btcChangeDescriptorXpub: string;
  rgbAssetsDescriptorXpub: string;
  rgbUdasDescriptorXpub: string;
  nostrPub: string;
  nostrNpub: string;
}

export interface Vault {
  mnemonic: string;
  private: PrivateWalletData;
  public: PublicWalletData;
}

export interface Transaction extends WalletTransaction {
  amount: number;
  asset?: string;
  assetType: string;
  fee: number;
  message?: string;
  note?: string;
}

export interface Activity extends Transaction {
  id: string;
  date: number;
  action: string;
  status: string;
  lightning?: boolean;
  sender?: {
    name: string;
    address: string;
  };
  recipient?: {
    name: string;
    address: string;
    invoice: string;
  };
}

export interface TransactionDetails extends Transaction {
  sender: {
    name: string;
    address: string;
  };
  recipient: {
    name: string;
    address: string;
    invoice: string;
  };
}

export interface TransactionData {
  details: TransactionDataDetails;
  vsize: number;
  feeRate: number;
}

export interface TransactionDataDetails {
  transaction?: Transaction;
  txid: string;
  received: number;
  sent: number;
  fee: number;
  confirmationTime?: ConfirmationTime;
  confirmed?: boolean;
}

export interface ConfirmationTime {
  height: number;
  timestamp: number;
}

export interface WalletTransaction {
  txid: string;
  received: number;
  sent: number;
  fee: number;
  confirmed: boolean;
  confirmationTime: ConfirmationTime;
  vsize: number;
  feeRate: number;
}

export interface WalletBalance {
  immature: number;
  trustedPending: number;
  untrustedPending: number;
  confirmed: number;
}

export interface WalletData {
  wallet?: string;
  name: string;
  address: string;
  balance: WalletBalance;
  transactions: WalletTransaction[];
  utxos: string[];
}

export interface FundVaultDetails {
  assetsOutput?: string;
  udasOutput?: string;
  isFunded: boolean;
  fundTxid: string;
}
