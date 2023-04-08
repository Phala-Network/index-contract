require('console-stamp')(console, '[HH:MM:ss.l]')
const { ContractPromise } = require('@polkadot/api-contract')
const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api')
const PhalaSdk = require('@phala/sdk')
const PhalaSDKTypes = PhalaSdk.types
const KhalaTypes = require('@phala/typedefs').khalaDev
const fs = require('fs')
const path = require('path')

const { loadContractFile, createContract } = require('utils');

const NODE_ENDPOINT = 'wss://poc5.phala.network/ws'
const PRUNTIME_ENDPOINT = 'https://poc5.phala.network/tee-api-1'
const CONTRACT_ID = '0x90a1cbf2a00c76e16d53cd3568c639eacbb30076138cf38270e4673f37c6a3ff'
const EXE_WORKER = '0x2eaaf908adda6391e434ff959973019fb374af1076edd4fec55b5e6018b1a955'
const SOURCE = 'Moonbeam'

async function loop_task() {
    return new Promise(async (_resolve, reject) => {
        console.log(`Loading contract metedata form file system`)
        const contract = loadContractFile(
            path.join(__dirname, '../../target/ink/index_executor/index_executor.contract'),
        )

        console.log(`Establishing connection with blockchain node`)
        const nodeApi = await ApiPromise.create({
            provider: new WsProvider(NODE_ENDPOINT),
            types: {
                ...KhalaTypes,
                ...PhalaSDKTypes,
            },
        })
        console.log(`Connected to node, create contract object`)
        const executor = await createContract(nodeApi, PRUNTIME_ENDPOINT, contract, CONTRACT_ID)
        console.log(`Contract objected created with contract ID: ${CONTRACT_ID}`)

        const keyring = new Keyring({ type: 'sr25519' })
        const alice = keyring.addFromUri('//Alice')
        const certAlice = await PhalaSdk.signCertificate({
            api: nodeApi,
            pair: alice,
        })

        console.log(`Start query contract periodically...`)

        // Trigger task search every 30 seconds
        setInterval(async () => {
            console.log(`🔍Trigger actived task search from ${SOURCE} for worker ${EXE_WORKER}`)
            await executor.query.run(certAlice,
                {},
                {'Fetch': [SOURCE, EXE_WORKER]}
            )
        }, 30000)

        // Trigger task executing every 10 seconds
        setInterval(async () => {
            console.log(`🐌Trigger task executing`)
            await executor.query.run(certAlice,
                {},
                'Execute'
            )
        }, 10000)
    })
}

async function main() {
    try {
        // never return
        await loop_task()
    } catch (err) {
        console.error(`task run failed: ${err}`)
    }
}
