import type * as PhalaSdk from "@phala/sdk";
import type * as DevPhase from "devphase";
import type * as DPT from "devphase/etc/typings";
import type { ContractCallResult, ContractQuery } from "@polkadot/api-contract/base/types";
import type { ContractCallOutcome, ContractOptions } from "@polkadot/api-contract/types";
import type { Codec } from "@polkadot/types/types";

export namespace IndexRegistry {
    type InkEnv_Types_AccountId = any;
    type InkPrimitives_Key = any;
    type InkStorage_Lazy_Mapping_Mapping = { offset_key: InkPrimitives_Key };
    type IndexRegistry_Types_ChainType = { Evm: null } | { Sub: null };
    type IndexRegistry_Types_AssetInfo = { name: string, symbol: string, decimals: number, location: number[] };
    type Option = { None: null } | { Some: number };
    type IndexRegistry_Types_ChainInfo = { name: string, chain_type: IndexRegistry_Types_ChainType, native: Option, stable: Option, endpoint: string, network: Option };
    type IndexRegistry_ChainStore_ChainStore = { info: IndexRegistry_Types_ChainInfo, assets: IndexRegistry_Types_AssetInfo[] };
    type IndexRegistry_Chain_Chain = { store: IndexRegistry_ChainStore_ChainStore };
    type IndexRegistry_Bridge_AssetPair = { asset0: IndexRegistry_Types_AssetInfo, asset1: IndexRegistry_Types_AssetInfo };
    type IndexRegistry_Bridge_Bridge = { name: string, chain0: IndexRegistry_Types_ChainInfo, chain1: IndexRegistry_Types_ChainInfo, assets: IndexRegistry_Bridge_AssetPair[] };
    type IndexRegistry_Dex_DexPair = { id: number[], asset0: IndexRegistry_Types_AssetInfo, asset1: IndexRegistry_Types_AssetInfo, swap_fee: Option, dev_fee: Option };
    type IndexRegistry_Dex_Dex = { id: number[], name: string, chain: IndexRegistry_Types_ChainInfo, pairs: IndexRegistry_Dex_DexPair[] };
    type IndexRegistry_Types_Error = { BadAbi: null } | { BadOrigin: null } | { AssetAlreadyRegistered: null } | { AssetNotFound: null } | { BridgeAlreadyRegistered: null } | { BridgeNotFound: null } | { ChainAlreadyRegistered: null } | { ChainNotFound: null } | { DexAlreadyRegistered: null } | { DexNotFound: null } | { ExtractLocationFailed: null } | { ConstructContractFailed: null } | { Unimplemented: null };
    type Result = { Ok: IndexRegistry_Types_Graph } | { Err: IndexRegistry_Types_Error };
    type IndexRegistry_Types_AssetGraph = { chain: string, location: number[], name: string, symbol: string, decimals: number };
    type IndexRegistry_Types_TradingPairGraph = { id: number[], asset0: string, asset1: string, dex: string, chain: string };
    type IndexRegistry_Types_BridgeGraph = { chain0: string, chain1: string, assets: [ string, string ][] };
    type IndexRegistry_Types_Graph = { assets: IndexRegistry_Types_AssetGraph[], pairs: IndexRegistry_Types_TradingPairGraph[], bridges: IndexRegistry_Types_BridgeGraph[] };

    /** */
    /** Queries */
    /** */
    namespace ContractQuery {
        export interface GetGraph extends DPT.ContractQuery {
            (certificateData: PhalaSdk.CertificateData, options: ContractOptions): DPT.CallResult<DPT.CallOutcome<DPT.IJson<Result>>>;
        }
    }

    export interface MapMessageQuery extends DPT.MapMessageQuery {
        getGraph: ContractQuery.GetGraph;
    }

    /** */
    /** Transactions */
    /** */
    namespace ContractTx {
        export interface RegisterChain extends DPT.ContractTx {
            (options: ContractOptions, info: IndexRegistry_Types_ChainInfo): DPT.SubmittableExtrinsic;
        }

        export interface UnregisterChain extends DPT.ContractTx {
            (options: ContractOptions, name: string): DPT.SubmittableExtrinsic;
        }

        export interface RegisterAsset extends DPT.ContractTx {
            (options: ContractOptions, chain: string, asset: IndexRegistry_Types_AssetInfo): DPT.SubmittableExtrinsic;
        }

        export interface UnregisterAsset extends DPT.ContractTx {
            (options: ContractOptions, chain: string, asset: IndexRegistry_Types_AssetInfo): DPT.SubmittableExtrinsic;
        }

        export interface SetChainNative extends DPT.ContractTx {
            (options: ContractOptions, chain: string, asset: IndexRegistry_Types_AssetInfo): DPT.SubmittableExtrinsic;
        }

        export interface SetChainStable extends DPT.ContractTx {
            (options: ContractOptions, chain: string, asset: IndexRegistry_Types_AssetInfo): DPT.SubmittableExtrinsic;
        }

        export interface SetChainEndpoint extends DPT.ContractTx {
            (options: ContractOptions, chain: string, endpoint: string): DPT.SubmittableExtrinsic;
        }

        export interface RegisterBridge extends DPT.ContractTx {
            (options: ContractOptions, name: string, chain0: IndexRegistry_Types_ChainInfo, chain1: IndexRegistry_Types_ChainInfo): DPT.SubmittableExtrinsic;
        }

        export interface UnregisterBridge extends DPT.ContractTx {
            (options: ContractOptions, name: string): DPT.SubmittableExtrinsic;
        }

        export interface AddBridgeAsset extends DPT.ContractTx {
            (options: ContractOptions, bridge_name: string, pair: IndexRegistry_Bridge_AssetPair): DPT.SubmittableExtrinsic;
        }

        export interface RemoveBridgeAsset extends DPT.ContractTx {
            (options: ContractOptions, bridge_name: string, pair: IndexRegistry_Bridge_AssetPair): DPT.SubmittableExtrinsic;
        }

        export interface RegisterDex extends DPT.ContractTx {
            (options: ContractOptions, name: string, id: number[], chain: IndexRegistry_Types_ChainInfo): DPT.SubmittableExtrinsic;
        }

        export interface UnregisterDex extends DPT.ContractTx {
            (options: ContractOptions, name: string): DPT.SubmittableExtrinsic;
        }

        export interface AddDexPair extends DPT.ContractTx {
            (options: ContractOptions, dex_name: string, pair: IndexRegistry_Dex_DexPair): DPT.SubmittableExtrinsic;
        }

        export interface RemoveDexPair extends DPT.ContractTx {
            (options: ContractOptions, dex_name: string, pair: IndexRegistry_Dex_DexPair): DPT.SubmittableExtrinsic;
        }
    }

    export interface MapMessageTx extends DPT.MapMessageTx {
        registerChain: ContractTx.RegisterChain;
        unregisterChain: ContractTx.UnregisterChain;
        registerAsset: ContractTx.RegisterAsset;
        unregisterAsset: ContractTx.UnregisterAsset;
        setChainNative: ContractTx.SetChainNative;
        setChainStable: ContractTx.SetChainStable;
        setChainEndpoint: ContractTx.SetChainEndpoint;
        registerBridge: ContractTx.RegisterBridge;
        unregisterBridge: ContractTx.UnregisterBridge;
        addBridgeAsset: ContractTx.AddBridgeAsset;
        removeBridgeAsset: ContractTx.RemoveBridgeAsset;
        registerDex: ContractTx.RegisterDex;
        unregisterDex: ContractTx.UnregisterDex;
        addDexPair: ContractTx.AddDexPair;
        removeDexPair: ContractTx.RemoveDexPair;
    }

    /** */
    /** Contract */
    /** */
    export declare class Contract extends DPT.Contract {
        get query(): MapMessageQuery;
        get tx(): MapMessageTx;
    }

    /** */
    /** Contract factory */
    /** */
    export declare class Factory extends DevPhase.ContractFactory {
        instantiate<T = Contract>(constructor: "new", params: never[], options?: DevPhase.InstantiateOptions): Promise<T>;
    }
}
