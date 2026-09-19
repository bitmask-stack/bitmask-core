// Methods meant to work with RGB contracts defined within the web::rgb module from bitmask-core:
// https://github.com/diba-io/bitmask-core/blob/development/src/web.rs

import * as BMC from "./bitmask_core";

export const fullIssueContract = async (
  nostrHexSk: string,
  request: FullIssueRequest
): Promise<IssueResponse> =>
  JSON.parse(await BMC.full_issue_contract(nostrHexSk, request));

export const createInvoice = async (
  nostrHexSk: string,
  request: InvoiceRequest
): Promise<InvoiceResponse> =>
  JSON.parse(await BMC.rgb_create_invoice(nostrHexSk, request));

export const createPsbt = async (
  nostrHexSk: string,
  request: PsbtRequest
): Promise<PsbtResponse> =>
  JSON.parse(await BMC.create_psbt(nostrHexSk, request));

export const psbtSignFile = async (
  nostrHexSk: string,
  request: SignPsbtRequest
): Promise<SignedPsbtResponse> =>
  JSON.parse(await BMC.psbt_sign_file(nostrHexSk, request));

export const psbtSignAndPublishFile = async (
  _nostrHexSk: string,
  request: SignPsbtRequest
): Promise<SignedPsbtResponse> =>
  JSON.parse(await BMC.psbt_sign_and_publish_file(request));

export const transferAsset = async (
  nostrHexSk: string,
  request: RgbTransferRequest
): Promise<RgbTransferResponse> =>
  JSON.parse(await BMC.transfer_asset(nostrHexSk, request));

export const fullTransferAsset = async (
  nostrHexSk: string,
  request: FullRgbTransferRequest
): Promise<RgbTransferResponse> =>
  JSON.parse(await BMC.full_transfer_asset(nostrHexSk, request));

export const importContract = async (
  nostrHexSk: string,
  request: ImportRequest
): Promise<ImportResponse> =>
  JSON.parse(await BMC.import_contract(nostrHexSk, request.data));

export const acceptTransfer = async (
  nostrHexSk: string,
  request: AcceptRequest
): Promise<AcceptResponse> =>
  JSON.parse(await BMC.accept_transfer(nostrHexSk, request));

export const listContracts = async (
  nostrHexSk: string
): Promise<ContractsResponse> =>
  JSON.parse(await BMC.list_contracts(nostrHexSk, true));

export const hideContract = async (
  nostrHexSk: string,
  contractId: string
): Promise<ContractHiddenResponse> =>
  JSON.parse(await BMC.hidden_contract(nostrHexSk, contractId));

export const listInterfaces = async (
  nostrHexSk: string
): Promise<InterfacesResponse> =>
  JSON.parse(await BMC.list_interfaces(nostrHexSk));

export const listSchemas = async (
  nostrHexSk: string
): Promise<SchemasResponse> => JSON.parse(await BMC.list_schemas(nostrHexSk));

export const createWatcher = async (
  nostrHexSk: string,
  request: WatcherRequest
): Promise<WatcherResponse> =>
  JSON.parse(await BMC.create_watcher(nostrHexSk, request));

export const watcherDetails = async (
  nostrHexSk: string,
  name: string
): Promise<WatcherDetailResponse> =>
  JSON.parse(await BMC.watcher_details(nostrHexSk, name));

export const watcherNextAddress = async (
  nostrHexSk: string,
  name: string,
  _iface: string
): Promise<NextAddressResponse> =>
  JSON.parse(await BMC.watcher_next_address(nostrHexSk, name));

export const watcherNextUtxo = async (
  nostrHexSk: string,
  name: string,
  _iface: string
): Promise<NextUtxoResponse> =>
  JSON.parse(await BMC.watcher_next_utxo(nostrHexSk, name));

export const watcherUnspentUtxos = async (
  nostrHexSk: string,
  name: string,
  _iface: string
): Promise<NextUtxosResponse> =>
  JSON.parse(await BMC.watcher_unspent_utxos(nostrHexSk, name));

export const watcherAddress = async (
  nostrHexSk: string,
  name: string,
  address: string
): Promise<WatcherUtxoResponse> =>
  JSON.parse(await BMC.watcher_address(nostrHexSk, name, address));

export const watcherUtxo = async (
  nostrHexSk: string,
  name: string,
  utxo: string
): Promise<WatcherUtxoResponse> =>
  JSON.parse(await BMC.watcher_utxo(nostrHexSk, name, utxo));

export const listTransfers = async (
  nostrHexSk: string,
  _contractId: string
): Promise<RgbTransfersResponse> =>
  JSON.parse(await BMC.list_transfers(nostrHexSk));

export const saveTransfer = async (
  nostrHexSk: string,
  request: RgbSaveTransferRequest
): Promise<RgbTransferStatusResponse> =>
  JSON.parse(await BMC.save_transfer(nostrHexSk, request));

export const removeTransfer = async (
  nostrHexSk: string,
  request: RgbRemoveTransferRequest
): Promise<RgbTransferStatusResponse> =>
  JSON.parse(await BMC.remove_transfer(nostrHexSk, request));

export const verifyTransfers = async (
  nostrHexSk: string
): Promise<BatchRgbTransferResponse> =>
  JSON.parse(await BMC.verify_transfers(nostrHexSk));

export const decodeInvoice = async (
  invoice: string
): Promise<RgbInvoiceResponse> => JSON.parse(await BMC.decode_invoice(invoice));

export const createOffer = async (
  nostrHexSk: string,
  request: RgbOfferRequest
): Promise<RgbOfferResponse> =>
  JSON.parse(await BMC.create_offer(nostrHexSk, request, []));

export const createBid = async (
  nostrHexSk: string,
  request: RgbBidRequest
): Promise<RgbBidResponse> =>
  JSON.parse(await BMC.create_bid(nostrHexSk, request, []));

export const createSwap = async (
  nostrHexSk: string,
  request: RgbSwapRequest
): Promise<RgbSwapResponse> =>
  JSON.parse(await BMC.create_swap(nostrHexSk, request));

export const createAuctionOffer = async (
  nostrHexSk: string,
  request: RgbAuctionOfferRequest
): Promise<RgbAuctionBidResponse> =>
  JSON.parse(await BMC.create_auction_offers(nostrHexSk, request));


export const createAuctionBid = async (
  nostrHexSk: string,
  request: RgbAuctionBidRequest
): Promise<RgbAuctionBidResponse> =>
  JSON.parse(await BMC.create_auction_bid(nostrHexSk, request));

export const finishAuction = async (
  nostrHexSk: string,
  request: string
): Promise<RgbAuctionOfferResponse[]> =>
  JSON.parse(await BMC.finish_auction_offers(nostrHexSk, request));

export const listAuctions = async (): Promise<RgbOffersResponse> =>
  JSON.parse(await BMC.list_auctions());

export const directSwap = async (
  nostrHexSk: string,
  request: RgbBidRequest
): Promise<RgbSwapResponse> =>
  JSON.parse(await BMC.direct_swap(nostrHexSk, request));

export const publicOffers = async (
  nostrHexSk: string
): Promise<PublicRgbOffersResponse> =>
  JSON.parse(await BMC.public_offers(nostrHexSk));

export const myOrders = async (
  nostrHexSk: string
): Promise<RgbOfferBidsResponse> => JSON.parse(await BMC.my_orders(nostrHexSk));

export const myOffers = async (
  nostrHexSk: string
): Promise<RgbOffersResponse> => JSON.parse(await BMC.my_offers(nostrHexSk));

export const myBids = async (nostrHexSk: string): Promise<RgbBidsResponse> =>
  JSON.parse(await BMC.my_bids(nostrHexSk));

export const importConsignments = async (
  request: ImportConsignmentsRequest
): Promise<boolean> => JSON.parse(await BMC.import_consignments(request));

export const getConsignment = async (
  consigOrReceiptId: string
): Promise<string> => JSON.parse(await BMC.get_consignment(consigOrReceiptId));

export const importUdaData = async (request: MediaRequest): Promise<boolean> =>
  JSON.parse(await BMC.import_uda_data(request));

export const getMedia = async (mediaId: string): Promise<MediaMetadata> =>
  JSON.parse(await BMC.get_media_metadata(mediaId));

export const contractAmount = (amount: bigint, precision: number): bigint =>
  JSON.parse(BMC.contract_amount(amount, precision).toString());

export const contractAmountStr = (amount: bigint, precision: number): String =>
  JSON.parse(BMC.contract_amount_str(amount, precision));

export const contractAmountParseStr = (amount: string, precision: number): String =>
  JSON.parse(BMC.contract_amount_parse_str(amount, precision).toString());

export const contractAmountParseValue = (amount: string, precision: number): bigint =>
  JSON.parse(BMC.contract_amount_parse_value(amount, precision).toString());

export const contractDecimalParseValue = (amount: string): bigint =>
  JSON.parse(BMC.contract_amount_parse_decimal_value(amount).toString());


// Core type interfaces based on structs defined within the bitmask-core Rust crate:
// https://github.com/diba-io/bitmask-core/blob/development/src/structs.rs

export interface ContractFormats {
  /// The contract state (encoded in bech32m)
  legacy: string;
  /// The contract state (encoded in strict)
  strict: string;
  /// The contract state (compiled in armored mode)
  armored: string;
}

export interface GenesisFormats {
  /// The genesis state (encoded in bech32m)
  legacy: string;
  /// The genesis state (encoded in strict)
  strict: string;
  /// The contract state (compiled in armored mode)
  armored: string;
}

/**
 * @deprecated please use `IssueMediaRequest` instead
 */
export interface IssueMetadata {
  uda?: MediaInfo[];
  collectible?: NewCollectible[];
}

export interface FullIssueRequest {
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// Amount of the asset
  supply: bigint;
  /// Precision of the asset
  precision: number;
  /// Seal of the initial owner
  seal: string;
  /// The name of the iface (ex: RGB20)
  iface: string;
  /// contract metadata (only RGB21/UDA)
  meta?: MediaRequest;
}

export interface NewCollectible {
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// attachments and media
  media: MediaInfo[];
}

export interface UDADetail {
  /// the token index of the uda
  tokenIndex: number;
  /// The ticker of the uda
  ticker: string;
  /// Name of the uda
  name: string;
  /// Description of the uda
  description: string;
  /// Media of the uda
  media: MediaInfo[];
  /// The user contract balance
  balance: bigint;
  /// The contract allocations
  allocations: AllocationDetail[];
}

export interface ContractMediaDetail {
  /// the token index of the uda
  tokenIndex: number;
  /// The ticker of the uda
  ticker: string;
  /// Name of the uda
  name: string;
  /// Description of the uda
  description: string;
  /// The user contract balance
  balance: bigint;
  /// The contract allocations
  allocations: AllocationDetail[];
  /// Preview of the uda
  preview?: MediaInfo;
  /// Media of the uda
  media?: MediaInfo;
  /// Attachments of the uda
  attachments: MediaInfo[];
}

/**
 * @deprecated please use `ContractMediaDetail` instead
 */
export interface ContractMetadata {
  uda?: UDADetail;
  collectible?: UDADetail[];
}

export interface IssueResponse {
  /// The contract id
  contractId: string;
  /// The contract impl id
  iimplId: string;
  /// The contract interface
  iface: string;
  /// The Issue Utxo
  issueUtxo: string;
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// Amount of the asset
  supply: bigint;
  /// Precision of the asset
  precision: number;
  /// The contract state (multiple formats)
  contract: ContractFormats;
  /// Genesis
  genesis: GenesisFormats;
  /// attachments and media (only RGB21/UDA)
  meta?: ContractMediaDetail;
}

export interface ImportRequest {
  /// The type data
  /// enum ImportType {
  ///     "contract"
  /// }
  import: string;
  /// The payload data (in hexadecimal)
  data: string;
}

// In structs.rs this is called ContractResponse
export interface ImportResponse {
  /// The contract id
  contractId: string;
  /// The contract impl id
  iimplId: string;
  /// The contract interface
  iface: string;
  /// The ticker of the asset
  ticker: string;
  /// Name of the asset
  name: string;
  /// Description of the asset
  description: string;
  /// Amount of the asset
  supply: bigint;
  /// Precision of the asset
  precision: number;
  /// The user contract balance
  balance: bigint;
  /// The user contract balance
  balanceNormalized: number;
  /// The contract allocations
  allocations: AllocationDetail[];
  /// The contract state (multiple formats)
  contract: ContractFormats;
  /// Genesis
  genesis: GenesisFormats;
  /// attachments and media (only RGB21/UDA)
  meta?: ContractMediaDetail;
}

export interface MediaInfo {
  /// Mime Type of the media
  type: string;
  /// Source (aka. hyperlink) of the media
  source: string;
}

// In structs.rs this is called SimpleContractResponse
export interface SimpleContractResponse {
  /// The contract id
  contractId: string;
  /// The contract interface
  ifaceId: string;
  /// Precision of the asset
  precision: number;
}

export interface InvoiceRequest {
  /// The contract id
  contractId: string;
  /// The contract interface
  iface: string;
  /// Amount of the asset
  amount: string;
  /// UTXO or Blinded UTXO
  seal: string;
  /// Query parameters
  params: { [key: string]: string };
}

export interface InvoiceResponse {
  /// Invoice encoded in Baid58
  invoice: string;
}

export interface PsbtRequest {
  /// Asset UTXOs
  assetInputs: PsbtInputRequest[];
  /// Asset Descriptor Change
  assetDescriptorChange: string;
  /// Asset Terminal Change (default: /10/0)
  assetTerminalChange: string;
  /// Bitcoin UTXOs
  bitcoinInputs: PsbtInputRequest[];
  /// Bitcoin Change Addresses (format: {address}:{amount})
  bitcoinChanges: string[];
  /// Bitcoin Fee
  fee: PsbtFeeRequest;
  /// Allow RBF
  rbf: boolean;
}

interface PsbtInputRequest {
  /// Asset or Bitcoin Descriptor
  descriptor: string;
  /// Asset or Bitcoin UTXO
  utxo: string;
  /// Asset or Bitcoin UTXO Terminal (ex. /0/0)
  utxoTerminal: string;
  /// Asset or Bitcoin Tweak
  tapret?: string;
  /// Asset or Bitcoin Tweak
  sighHash?: PsbtSigHashRequest;
}

interface PsbtSigHashRequest {
  All: string;
  /// 0x2: Sign no outputs --- anyone can choose the destination.
  None: string;
  /// 0x3: Sign the output whose index matches this input's index. If none exists,
  /// sign the hash `0000000000000000000000000000000000000000000000000000000000000001`.
  /// (This rule is probably an unintentional C++ism, but it's consensus so we have
  /// to follow it.)
  Single: string;
  /// 0x81: Sign all outputs but only this input.
  AllPlusAnyoneCanPay: string;
  /// 0x82: Sign no outputs and only this input.
  NonePlusAnyoneCanPay: string;
  /// 0x83: Sign one output and only this input (see `Single` for what "one output" means).
  SinglePlusAnyoneCanPay: string;
}

interface PsbtFeeRequest {
  value?: number;
  feeRate?: number;
}

export interface PsbtResponse {
  /// PSBT encoded in Base64
  psbt: string;
  /// Asset UTXO Terminal (ex. /0/0)
  terminal: string;
}

export interface SignPsbtRequest {
  /// PSBT encoded in Base64
  psbt: string;
  /// Descriptors to Sign
  descriptors: string[];
}

export interface PublishedPsbtResponse {
  /// PSBT is signed?
  sign: boolean;
  /// TX id
  txid: string;
}

export interface SignedPsbtResponse {
  /// PSBT is signed?
  sign: boolean;
  /// PSBT signed
  psbt: string;
}

export interface PublishPsbtRequest {
  /// PSBT encoded in Base64
  psbt: string;
}

export interface RgbTransferRequest {
  /// RGB Invoice
  rgbInvoice: string;
  /// PSBT File Information
  psbt: string;
  /// Asset UTXO Terminal (ex. /0/0)
  terminal: string;
}

export interface FullRgbTransferRequest {
  /// The contract id
  contractId: string;
  /// The contract interface
  iface: string;
  /// RGB Invoice
  rgbInvoice: string;
  /// Asset or Bitcoin Descriptor
  descriptor: string;
  /// Bitcoin Terminal Change
  changeTerminal: string;
  /// Bitcoin Fee
  fee: PsbtFeeRequest;
  /// Bitcoin Change Addresses (format: {address}:{amount})
  bitcoinChanges: string[];
}

export interface RgbTransferResponse {
  /// Consignment ID
  consigId: string;
  /// Consignment encoded (in hexadecimal)
  consig: string;
  /// PSBT File Information with tapret (in hexadecimal)
  psbt: string;
  /// Tapret Commitment (used to spend output)
  commit: string;
  /// Transfer Bitcoin L1 transaction id
  txid: string;
}

export interface AcceptRequest {
  /// Consignment encoded in hexadecimal
  consignment: string;
  /// Force Consignment accept
  force: boolean;
}

export interface AcceptResponse {
  /// Transfer ID
  transferId: string;
  /// Contract ID
  contractId: string;
  /// Transfer accept status
  valid: boolean;
}

export interface ContractsResponse {
  /// List of available contracts
  contracts: ImportResponse[];
}

export interface ContractHiddenResponse {
  /// The contract id
  contractId: string;
  /// is hidden
  hidden: boolean;
}

export interface InterfacesResponse {
  /// List of available interfaces and implementations
  interfaces: InterfaceDetail[];
}

export interface InterfaceDetail {
  /// Interface Name
  name: string;
  /// Interface ID
  iface: string;
  /// Interface ID
  iimpl: string;
}

export interface SchemasResponse {
  /// List of available schemas
  schemas: SchemaDetail[];
}

export interface SchemaDetail {
  /// Schema ID
  schema: string;
  /// Available Interfaces
  ifaces: string[];
}

export interface WatcherRequest {
  /// The watcher name
  name: string;
  /// The xpub will be watch
  xpub: string;
  /// Force recreate
  force: boolean;
}

export interface WatcherResponse {
  /// The watcher name
  name: string;
}

export interface UtxoResponse {
  outpoint: string;
  amount: bigint;
  status: TxStatus;
}

export interface NextUtxoResponse {
  utxo?: UtxoResponse;
}

export interface NextUtxosResponse {
  utxos: UtxoResponse[];
}

export interface NextAddressResponse {
  address: string;
  network: string;
}

export interface WatcherDetailResponse {
  /// Allocations
  contracts: WatcherDetail[];
}

export interface WatcherUtxoResponse {
  utxos: string[];
}

export interface WatcherDetail {
  /// Contract ID
  contractId: string;
  /// Allocations
  allocations: AllocationDetail[];
}

export interface UDAPosition {
  tokenIndex: number;
  fraction: bigint;
}

export type AllocationValue = {
  value?: bigint;
  uda?: UDAPosition;
};

export interface AllocationDetail {
  /// Anchored UTXO
  utxo: string;
  /// Asset Value
  value: AllocationValue;
  /// Derivation Path
  derivation: string;
  /// Derivation Path
  isMine: boolean;
  /// Allocation spent?
  isSpent: boolean;
}

export interface DeclareRequest {
  disclosure: string;
  // disclosure: Declare; // TODO: Revisit after 0.6 release
  changeTransfers: ChangeTansfer[];
  transfers: FinalizeTransfer[];
}

export interface FinalizeTransfer {
  previousUtxo: string;
  consignment: string;
  asset: string;
  beneficiaries: BlindedOrNotOutpoint[];
}

export interface ChangeTansfer {
  previousUtxo: string;
  asset: string;
  change: BlindedOrNotOutpoint;
}

export interface BlindedOrNotOutpoint {
  outpoint: string;
  balance: number;
}

export interface Contract {
  id: string;
  ticker: string;
  name: string;
  description: string;
  allocations: AllocationDetail[];
  balance: bigint;
  genesis: string;
}

export interface RgbSaveTransferRequest {
  /// iFace Name
  iface: string;
  /// Consignment encoded in hexadecimal
  consignment: string;
}

export interface RgbRemoveTransferRequest {
  /// Contract ID
  contractId: string;
  /// Consignment ID
  consigIds: string[];
}

export interface RgbTransferStatusResponse {
  /// Contract ID
  contractId: string;
  /// Transfer ID
  consigStatus: Map<string, boolean>;
}

export interface RgbTransfersResponse {
  /// List of available transfers
  transfers: RgbTransferDetail[];
}

export interface RgbTransferDetail {
  consigId: string;
  status: TxStatus;
  type: TransferType;
}

export interface TxStatus {
  notFound?: any;
  error?: string;
  mempool?: any;
  block?: number;
}

export interface TransferType {
  sended?: any;
  received?: any;
  unknown?: any;
}

export interface RgbInvoiceResponse {
  contractId: string;
  amount: bigint;
}

export interface BatchRgbTransferResponse {
  transfers: BatchRgbTransferItem[];
}

export interface BatchRgbTransferItem {
  contractId: string;
  consigId: string;
  status: TxStatus;
  isAccept: boolean;
  iface: string;
  txid: string;
}

export interface RgbOfferRequest {
  /// The Contract ID
  contractId: string;
  /// The Contract Interface
  iface: string;
  /// Contract Amount
  contractAmount: string;
  /// Bitcoin Price (in sats)
  bitcoinPrice: bigint;
  /// Universal Descriptor
  descriptor: string;
  /// Asset Terminal Change
  changeTerminal: string;
  /// Bitcoin Change Addresses (format: {address}:{amount})
  bitcoinChanges: string[];
  strategy: RgbSwapStrategy;
  expireAt?: number;
}

export enum RgbSwapStrategy {
  Auction = "auction",
  P2P = "p2p",
  HotSwap = "hotswap",
}

export interface RgbAuctionStrategy {
  auction?: string,
  airdrop?: string,
}

export interface RgbAuctionOfferRequest {
  offers: RgbOfferRequest[],

  signKeys: string[],

  strategy: RgbAuctionStrategy,

  fee?: PsbtFeeRequest
}

export interface RgbOfferUpdateRequest {
  /// The Contract ID
  contract_id: string,
  /// The Offer ID
  offer_id: string,
  // Swap PSBT
  offer_psbt: string
}

export interface RgbOfferUpdateResponse {
  /// The Contract ID
  contract_id: string,
  /// The Offer ID
  offer_id: string,
  /// Updated?
  updated: boolean
}

export interface RgbAuctionBidRequest {
  /// The Offer ID
  offerId: string,
  /// Asset Amount
  assetAmount: string,
  /// Universal Descriptor
  descriptor: string,
  /// Bitcoin Terminal Change
  changeTerminal: string,
  /// Descriptors to Sign
  signKeys: string[],
  /// Bitcoin Fee
  fee: PsbtFeeRequest,
}

export interface RgbAuctionBidResponse {
  /// The Bid ID
  bidId: string,
  /// The Offer ID
  offerId: string,
  /// Fee Value
  feeValue: number,
}

export interface RgbMatchResponse {
    /// Transfer ID
    consigId: string,
    /// Offer ID
    offerId: string,
    /// Bid ID
    bidId: string,
}

export interface RgbAuctionOfferResponse {
  /// Offer ID
  offerId: string,
  /// Contract ID
  contractId: string,
  /// Asset/Contract Amount
  assetAmount: number,
  /// Bitcoin Price
  bitcoinPrice: number,
  /// Bundle ID
  bundleId: string,
}

export interface RgbOfferResponse {
  /// The Contract ID
  offerId: string;
  /// The Contract ID
  contractId: string;
  /// Contract Amount
  contractAmount: number;
  /// Bitcoin Price
  bitcoinPrice: bigint;
  /// Seller Address
  sellerAddress: string;
  /// Seller PSBT (encoded in base64)
  sellerPsbt: string;
  /// Bundle ID (collection)
  bundleId?: string,
}

export interface RgbBidRequest {
  /// The Offer ID
  offerId: string;
  /// Asset Amount
  assetAmount: string;
  /// Universal Descriptor
  descriptor: string;
  /// Bitcoin Terminal Change
  changeTerminal: string;
  /// Bitcoin Fee
  fee: PsbtFeeRequest;
}

export interface RgbBidResponse {
  /// The Bid ID
  bidId: string;
  /// The Offer ID
  offerId: string;
  /// Buyer Invoice
  invoice: string;
  /// Final PSBT (encoded in base64)
  swapPsbt: string;
  /// Fee Value
  feeValue: bigint;
}

export interface RgbSwapRequest {
  /// Offer ID
  offerId: string;
  /// Bid ID
  bidId: string;
  /// Swap PSBT
  swapPsbt: string;
}

export interface RgbSwapResponse {
  /// Transfer ID
  consigId: string;
  /// Final Consig
  finalConsig: string;
  /// Final PSBT
  finalPsbt: string;
}

export interface PublicRgbOffersResponse {
  /// Offers
  offers: PublicRgbOfferResponse[];
  /// Public bids
  bids: Map<string, PublicRgbBidResponse[]>;
}

export interface PublicRgbOfferResponse {
  /// Offer ID
  offerId: string;
  /// Contract ID
  contractId: string;
  /// Offer PubKey
  offerPub: string;
  /// Asset/Contract Amount
  assetAmount: bigint;
  /// Bitcoin Price
  bitcoinPrice: bigint;
  /// Initial Offer PSBT
  offerPsbt?: string;
}

export interface RgbAuctionFinishResponse {
  /// Bundle ID
  bundle_id: string,
  /// New Change Outpoint
  outpoint: string,
  /// Sold Items
  sold: Map<string, RgbSwapItem>,
  /// Reamining Items
  remaining: Map<string, RgbSwapItem>,
}

export interface RgbSwapItem {
  /// Contract ID
  contractId: string,
  /// Iface
  iface: string,
  /// Final Consig
  contractAmount: string,
}


export interface PublicRgbBidResponse {
  /// Bid ID
  bidId: string;
  /// Asset/Contract Amount
  assetAmount: bigint;
  /// Bitcoin Price
  bitcoinPrice: bigint;
}

export interface RgbOfferBidsResponse {
  /// Offers
  offers: RgbOfferDetail[];
  /// bids
  bids: RgbBidDetail[];
}

export interface RgbOffersResponse {
  /// Offers
  offers: RgbOfferDetail[];
}

export interface RgbBidsResponse {
  /// Bids
  bids: RgbBidDetail[];
}

export interface RgbOfferDetail {
  /// Contract ID
  contractId: string;
  /// Offer ID
  offerId: string;
  /// Offer Status
  offerStatus: string;
  /// Asset/Contract Amount
  assetAmount: bigint;
  /// Bitcoin Price
  bitcoinPrice: bigint;
}

export interface RgbBidDetail {
  /// Contract ID
  contractId: string;
  /// Bid ID
  bidId: string;
  /// Offer ID
  offerId: string;
  /// Bid Status
  bidStatus: string;
  /// Asset/Contract Amount
  assetAmount: bigint;
  /// Bitcoin Price
  bitcoinPrice: bigint;
}

export interface IssueMediaRequest {
  preview?: MediaInfo;
  media?: MediaInfo;
  attachments: MediaInfo[];
}

export interface MediaRequest {
  preview?: MediaItemRequest;
  media?: MediaItemRequest;
  attachments: MediaItemRequest[];
}

export interface MediaExtractRequest {
  encode: MediaEncode;
  item: MediaItemRequest;
}
export interface MediaItemRequest {
  /// Media Type
  type: string;
  /// Media URI
  uri: string;
}

export interface MediaResponse {
  preview?: MediaView;
  media?: MediaView;
  attachments: MediaView[];
}

export interface MediaView {
  /// Media ID
  id: string;
  /// Media Type
  type: string;
  /// Media Encoded Representation
  source: string;
  /// Media Encoded Type
  encode: MediaEncode;
}

export interface MediaEncode {
  base64?: string;
  sha2?: string;
  blake3?: string;
}
export interface ImportConsignmentsRequest {
  [consignmentId: string]: string;
}
export interface MediaMetadata {
  id: string;
  mime: string;
  uri: string;
  digest: string;
}
