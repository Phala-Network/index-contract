require('console-stamp')(console, '[HH:MM:ss.l]')
const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api')
const PhalaSdk = require('@phala/sdk')
const PhalaSDKTypes = PhalaSdk.types
const KhalaTypes = require('@phala/typedefs').khalaDev
const path = require('path')

const { loadContractFile, createContract } = require('./utils');

const NODE_ENDPOINT = 'wss://poc5.phala.network/ws'
const PRUNTIME_ENDPOINT = 'https://poc5.phala.network/tee-api-1'
const CONTRACT_ID = '0x73764ed5e41d6d702a8ba80475a4d46a9597876b055ac6866e05a4cfee2b9db6'
const EXE_WORKER = '0xf455d5ae94e19db3ff7e045602a449febdf713ec26941b9005da8f80ddbcab43'
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
        }, 15000)

        // Trigger task executing every 10 seconds
        // setInterval(async () => {
        //     console.log(`🐌Trigger task executing`)
        //     await executor.query.run(certAlice,
        //         {},
        //         'Execute'
        //     )
        // }, 10000)
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

main()