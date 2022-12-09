import type * as PhalaSdk from "@phala/sdk";
import type * as DevPhase from "devphase";
import type * as DPT from "devphase/etc/typings";
import type { ContractCallResult, ContractQuery } from "@polkadot/api-contract/base/types";
import type { ContractCallOutcome, ContractOptions } from "@polkadot/api-contract/types";
import type { Codec } from "@polkadot/types/types";

export namespace SemiBridge {
    type InkEnv_Types_AccountId = any;
    type PrimitiveTypes_H160 = any;
    type SemiBridge_SemiBridge_Error = { BadOrigin: null } | { NotConfigurated: null } | { KeyRetired: null } | { KeyNotRetiredYet: null } | { UpstreamFailed: null } | { BadAbi: null } | { FailedToGetStorage: null } | { FailedToDecodeStorage: null } | { FailedToEstimateGas: null } | { FailedToCreateExecutor: null };
    type Result = { Ok: never[] } | { Err: SemiBridge_SemiBridge_Error };
    type PrimitiveTypes_H256 = any;
    type PrimitiveTypes_U256 = any;

    /** */
    /** Queries */
    /** */
    namespace ContractQuery {
        export interface Wallet extends DPT.ContractQuery {
            (certificateData: PhalaSdk.CertificateData, options: ContractOptions): DPT.CallResult<DPT.CallOutcome<DPT.IJson<PrimitiveTypes_H160>>>;
        }

        export interface Transfer extends DPT.ContractQuery {
            (certificateData: PhalaSdk.CertificateData, options: ContractOptions, token_rid: PrimitiveTypes_H256, amount: PrimitiveTypes_U256, recipient: PrimitiveTypes_H256): DPT.CallResult<DPT.CallOutcome<DPT.IJson<Result>>>;
        }
    }

    export interface MapMessageQuery extends DPT.MapMessageQuery {
        wallet: ContractQuery.Wallet;
        transfer: ContractQuery.Transfer;
    }

    /** */
    /** Transactions */
    /** */
    namespace ContractTx {
        export interface Config extends DPT.ContractTx {
            (options: ContractOptions, rpc: string, bridge_address: PrimitiveTypes_H160): DPT.SubmittableExtrinsic;
        }

        export interface SetAccount extends DPT.ContractTx {
            (options: ContractOptions, private_key: number[]): DPT.SubmittableExtrinsic;
        }
    }

    export interface MapMessageTx extends DPT.MapMessageTx {
        config: ContractTx.Config;
        setAccount: ContractTx.SetAccount;
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
        instantiate<T = Contract>(constructor: "default", params: never[], options?: DevPhase.InstantiateOptions): Promise<T>;
    }
}
