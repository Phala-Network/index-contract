import { IndexRegistry } from '@/typings/IndexRegistry';
import * as PhalaSdk from '@phala/sdk';
import { ApiPromise } from '@polkadot/api';
import type { KeyringPair } from '@polkadot/keyring/types';
import { ContractType } from 'devphase';

import 'dotenv/config';

async function delay(ms: number): Promise<void> {
    return new Promise( resolve => setTimeout(resolve, ms) );
}

describe('Registry tests', () => {
    let registryFactory: IndexRegistry.Factory;
    let registry: IndexRegistry.Contract;
    let registryCodeHash: string;

    let api: ApiPromise;
    let alice : KeyringPair;
    let certAlice : PhalaSdk.CertificateData;
    const txConf = { gasLimit: "10000000000000", storageDepositLimit: null };

    before(async function() {
        registryFactory = await this.devPhase.getFactory(
            ContractType.InkCode,
            './artifacts/index_registry/index_registry.contract'
        );
        registryCodeHash = registryFactory.metadata.source.hash;
        await registryFactory.deploy();
        expect(registryCodeHash.startsWith('0x')).to.be.true;
        
        api = this.api;
        alice = this.devPhase.accounts.alice;
        certAlice = await PhalaSdk.signCertificate({
            api,
            pair: alice,
        });
        console.log('Signer:', alice.address.toString());
        console.log('registry code:', registryCodeHash)
    });

    describe('Registry config', () => {
        before(async function() {
            this.timeout(30_000);
            // Deploy contract
            registry = await registryFactory.instantiate('new', [], {transferToCluster: 10e12});
            console.log('IndexRegistry deployed at', registry.address.toString());
        });

        it('Registry functions should work', async function() {
            // Registry chain:Ethereum
            const ethereumReg = await registry.tx
                .registerChain(txConf, {
                    "name": "Ethereum",
                    "chainType": "Evm",
                    "native": null,
                    "stable": null,
                    "endpoint": "https://rinkeby.infura.io/v3/6d61e7957c1c489ea8141e947447405b",
                    "network": null,
                })
                .signAndSend(alice, {nonce: -1});
            console.log('Register Ethereum', ethereumReg.toHuman());
            await delay(1*1000);
            // Registry chain:Khala
            const khalaReg = await registry.tx
                .registerChain(txConf, {
                    "name": "Khala",
                    "chainType": "Sub",
                    "native": null,
                    "stable": null,
                    "endpoint": "wss://khala-api.phala.network/ws",
                    "network": null,
                })
                .signAndSend(alice, {nonce: -1});
            console.log('Register Khala', khalaReg.toHuman());
            // Registry chain:Karura
            const karuraReg = await registry.tx
                .registerChain(txConf, {
                    "name": "Karura",
                    "chainType": "Sub",
                    "native": null,
                    "stable": null,
                    "endpoint": "wss://karura-rpc-0.aca-api.network",
                    "network": null,
                })
                .signAndSend(alice, {nonce: -1});
            console.log('Register Karura', karuraReg.toHuman());
            const registerPha = await registry.tx
                .registerAsset(txConf, "Ethereum", {
                    "name": "Phala Token",
                    "symbol": "PHA",
                    "decimals": 18,
                    "location": "0x6c5ba91642f10282b576d91922ae6448c9d52f4e",
                })
                .signAndSend(alice, {nonce: -1});
            console.log('Register Ethereum PHA', registerPha.toHuman());
            const graphQuery = await registry.query.getGraph(certAlice, {});
            expect(graphQuery.result.isOk).to.be.true;
            expect(graphQuery.output.asOk.assets[0].location.toHex()).to.be.equal("0x6c5ba91642f10282b576d91922ae6448c9d52f4e");
        })
    });

    // // To keep the blockchain running after the test, remove the "skip" in the following test
    // after('keeps running', async function() {
    //     this.timeout(1000 * 30_000);
    //     await delay(1000 * 30_000);
    // });
});
