import type * as PhalaSdk from "@phala/sdk";
import type * as DevPhase from "devphase";
import type * as DPT from "devphase/etc/typings";
import type { ContractCallResult, ContractQuery } from "@polkadot/api-contract/base/types";
import type { ContractCallOutcome, ContractOptions } from "@polkadot/api-contract/types";
import type { Codec } from "@polkadot/types/types";

export namespace IndexExecutor {
    type InkEnv_Types_AccountId = any;
    type IndexExecutor_IndexExecutor_Error = { ReadCacheFailed: null } | { WriteCacheFailed: null } | { DecodeCacheFailed: null } | { ExecuteFailed: null } | { Unimplemented: null };
    type Result = { Ok: IndexRegistry_Types_Graph } | { Err: IndexExecutor_IndexExecutor_Error };
    type IndexRegistry_Types_AssetGraph = { chain: string, location: number[], name: string, symbol: string, decimals: number };
    type IndexRegistry_Types_TradingPairGraph = { id: number[], asset0: string, asset1: string, dex: string, chain: string };
    type IndexRegistry_Types_BridgeGraph = { chain0: string, chain1: string, assets: [ string, string ][] };
    type IndexRegistry_Types_Graph = { assets: IndexRegistry_Types_AssetGraph[], pairs: IndexRegistry_Types_TradingPairGraph[], bridges: IndexRegistry_Types_BridgeGraph[] };
    type IndexExecutor_IndexExecutor_AccountInfo = { account32: DPT.FixedArray<number, 32>, account20: DPT.FixedArray<number, 20> };

    /** */
    /** Queries */
    /** */
    namespace ContractQuery {
        export interface GetGraph extends DPT.ContractQuery {
            (certificateData: PhalaSdk.CertificateData, options: ContractOptions): DPT.CallResult<DPT.CallOutcome<DPT.IJson<Result>>>;
        }

        export interface GetExecutorAccount extends DPT.ContractQuery {
            (certificateData: PhalaSdk.CertificateData, options: ContractOptions): DPT.CallResult<DPT.CallOutcome<DPT.IJson<IndexExecutor_IndexExecutor_AccountInfo>>>;
        }

        export interface GetWorkerAccount extends DPT.ContractQuery {
            (certificateData: PhalaSdk.CertificateData, options: ContractOptions): DPT.CallResult<DPT.CallOutcome<DPT.IVec<DPT.IJson<IndexExecutor_IndexExecutor_AccountInfo>>>>;
        }

        export interface Execute extends DPT.ContractQuery {
            (certificateData: PhalaSdk.CertificateData, options: ContractOptions): DPT.CallResult<DPT.CallOutcome<DPT.IJson<Result>>>;
        }
    }

    export interface MapMessageQuery extends DPT.MapMessageQuery {
        getGraph: ContractQuery.GetGraph;
        getExecutorAccount: ContractQuery.GetExecutorAccount;
        getWorkerAccount: ContractQuery.GetWorkerAccount;
        execute: ContractQuery.Execute;
    }

    /** */
    /** Transactions */
    /** */
    namespace ContractTx {
        export interface SetRegistry extends DPT.ContractTx {
            (options: ContractOptions, registry: InkEnv_Types_AccountId): DPT.SubmittableExtrinsic;
        }
    }

    export interface MapMessageTx extends DPT.MapMessageTx {
        setRegistry: ContractTx.SetRegistry;
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
