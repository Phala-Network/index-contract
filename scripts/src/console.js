require('dotenv').config();

const fs = require('fs');
const path = require('path')
const { program } = require('commander');
const { Decimal } = require('decimal.js');
const BN = require('bn.js');
const { ApiPromise, Keyring, WsProvider } = require('@polkadot/api');
const { cryptoWaitReady } = require('@polkadot/util-crypto');
const { stringToHex } = require('@polkadot/util');
const ethers = require('ethers')
const PhalaSdk = require('@phala/sdk')
const PhalaSDKTypes = PhalaSdk.types
const KhalaTypes = require('@phala/typedefs').khalaDev
const { loadContractFile, createContract, delay } = require('./utils');

const ERC20ABI = require('./ERC20ABI.json')
const HandlerABI = require('./HandlerABI.json')

function run(afn) {
    function runner(...args) {
        afn(...args)
            .then(process.exit)
            .catch(console.error)
            .finally(() => process.exit(-1));
    };
    return runner;
}

function useConfig() {
    const { config } = program.opts();
    return JSON.parse(fs.readFileSync(config, 'utf8'));
}

function useChainEndpoint(config, chainName) {
    const chain = config.chains.find(chain => Object.keys(chain)[0].toLowerCase() === chainName);
    const endpoint = chain && Object.values(chain)[0];
    return endpoint;
}

function useChainHandler(config, chainName) {
    const chain = config.handlers.find(chain => Object.keys(chain)[0].toLowerCase() === chainName);
    const handler = chain && Object.values(chain)[0];
    return handler;
}

function useChainType(chain) {
    if (['ethereum', 'goerli', 'moonbeam', 'astarevm'].includes(chain)) {
        return 'Evm';
    } else if (['astar', 'poc3', 'poc5', 'khala', 'phala', 'acala'].includes(chain)) {
        return 'Sub';
    } else {
        throw new Error(`Unrecognized chain type: ${chain}`);
    }
}

async function useApi(endpoint) {
    const wsProvider = new WsProvider(endpoint);
    const api = await ApiPromise.create({
        provider: wsProvider,
        types: {
            ...KhalaTypes,
            ...PhalaSDKTypes,
        },
    });
    return api;
}

async function useCert(uri, api) {
    await cryptoWaitReady();
    const keyring = new Keyring({ type: 'sr25519' })
    const account = keyring.addFromUri(uri)
    return await PhalaSdk.signCertificate({
        api: api,
        pair: account,
    });
}

async function usePair(uri) {
    await cryptoWaitReady();
    const keyring = new Keyring({ type: 'sr25519' })
    return keyring.addFromUri(uri)
}

function useEtherProvider(endpoint) {
    return new ethers.JsonRpcProvider(endpoint)
}

function useERC20Token(provider, token) {
    return new ethers.Contract(
        token,
        ERC20ABI,
        provider
    )
}

function useEvmHandler(config, chainName, key) {
    let endpoint = useChainEndpoint(config, chainName);
    let provider = useEtherProvider(endpoint);
    let handlerAddress = useChainHandler(config, chainName);

    const wallet = new ethers.Wallet(key, provider)
    return new ethers.Contract(
        handlerAddress,
        HandlerABI,
        wallet
    )
}

async function useExecutor(api, pruntime_endpoint, contract_id) {
    const contract = loadContractFile(
        path.join(__dirname, '../../target/ink/index_executor/index_executor.contract'),
    )
    console.log(`Connected to node, create contract object`)
    return await createContract(api, pruntime_endpoint, contract, contract_id)
}

async function useKeystore(api, pruntime_endpoint, contract_id) {
    const contract = loadContractFile(
        path.join(__dirname, '../../target/ink/key_store/key_store.contract'),
    )
    console.log(`Connected to node, create contract object`)
    return await createContract(api, pruntime_endpoint, contract, contract_id)
}

program
    .option('--config <path>', 'config that contains contract and node informations', process.env.CONFIG || 'config.json')
    .option('--uri <URI>', 'the account URI use to sign cert', process.env.URI || '//Alice')
    .option('--storage-url <storage-url>', 'base url of firebase', process.env.STORAGE_URL)
    .option('--storage-key <storage-key>', 'access token of firebase', process.env.STORAGE_KEY)


const keystore = program
.command('keystore')
.description('inDEX keystore');

keystore
    .command('set-executor')
    .description('set executor contract id to keystore')
    .action(run(async (opt) => {
        let { uri } = program.opts();
        let config = useConfig();
        let api = await useApi(config.node_wss_endpoint);
        let cert = await useCert(uri, api);
        let pair = await usePair(uri);
        let keystore = await useKeystore(api, config.pruntine_endpoint, config.key_store_contract_id);
        // costs estimation
        let { gasRequired, storageDeposit } = await keystore.query.setExecutor(cert, {}, config.executor_contract_id);
        // transaction / extrinct
        let options = {
            gasLimit: gasRequired.refTime,
            storageDepositLimit: storageDeposit.isCharge ? storageDeposit.asCharge : null,
        };
        await keystore.tx.setExecutor(options, config.executor_contract_id).signAndSend(pair, { nonce: -1 });
    }));

const executor = program
.command('executor')
.description('inDEX executor');

executor
    .command('setup')
    .description('setup executor, stuff contains 1) call config; 3) setup worker account in remote storage')
    .option('--resume', 'resume executor', false)
    .option('--import-key', 'import worker keys from keystore contract', true)
    .action(run(async (opt) => {
        let { uri, storageUrl, storageKey } = program.opts();
        if (storageUrl === undefined || storageKey === undefined) {
            throw new Error("Storage URL and Key must be provided");
        }
        let config = useConfig();
        let api = await useApi(config.node_wss_endpoint);
        let executor = await useExecutor(api, config.pruntine_endpoint, config.executor_contract_id);
        let cert = await useCert(uri, api);
        let pair = await usePair(uri);

        {
            // costs estimation
            let { gasRequired, storageDeposit } = await executor.query.configEngine(cert, {},
                storageUrl,
                storageKey,
                config.key_store_contract_id,
                opt.resume,
            );
            // transaction / extrinct
            let options = {
                gasLimit: gasRequired.refTime,
                storageDepositLimit: storageDeposit.isCharge ? storageDeposit.asCharge : null,
            };
            await executor.tx.configEngine(options,
                storageUrl,
                storageKey,
                config.key_store_contract_id,
                opt.resume
            ).signAndSend(pair, { nonce: -1 });
            console.log(`✅ Config executor`)
        }

        await delay(10*1000);   // 10 seconds
        {
            await executor.query.configStorage(cert, {});
            console.log(`✅ Config storage`)
        }

        if (opt.resume !== false) {
            // costs estimation
            let { gasRequired, storageDeposit } = await executor.query.resumeExecutor(cert, {});
            // transaction / extrinct
            let options = {
                gasLimit: gasRequired.refTime,
                storageDepositLimit: storageDeposit.isCharge ? storageDeposit.asCharge : null,
            };
            await executor.tx.resumeExecutor(options).signAndSend(pair, { nonce: -1 });
            console.log(`✅ Resume executor`);
        }
        console.log(`🎉 Finished executor configuration!`);
    }));

executor
    .command('worker')
    .description('return worker accounts of the executor')
    .option('--free', 'list worker account that currently not been allocated', false)
    .action(run(async (opt) => {
        // TODO
    }));

executor
    .command('task')
    .description('return tasks id list that currently running')
    .action(run(async (opt) => {
        // TODO
    }));

const task = program
    .command('task')
    .description('inDEX task inspector');
    
task
    .command('list')
    .description('Return tasks existing in local cache')
    .option('--id <taskId>', 'task id', null)
    .action(run(async (opt) => {
        // TODO
        // If id is not set, return all tasks existing in local cache
    }));

const handler = program
.command('handler')
.description('inDEX handler contract/pallet');

handler
    .command('list')
    .description('list handler account deployed on chains')
    .option('--chain <chain>', 'chain name', null)
    .action(run(async (opt) => {
        // TODO
        // If chain not given, list handker on all supported chains
    }));

handler
    .command('set-worker')
    .description('whitelist worker on handler')
    .requiredOption('--chain <chain>', 'chain name', null)
    .requiredOption('--worker <worker>', 'worker to run the task', null)
    .requiredOption('--key <key>', 'key of Handler contract admin', null)

    .action(run(async (opt) => {
        let config = useConfig();
        if (useChainType(opt.chain.toLowerCase()) === 'Evm') {
            let handler = useEvmHandler(config, opt.chain.toLowerCase(), opt.key)
            let tx = await handler.setWorker(
                opt.worker,
                {
                  gasLimit: 2000000,
                }
            );
            console.log(`Whitelist worker on ${opt.chain}: ${tx.hash}`);
        } else {    // Sub
            throw new Error("not implemented")
        }
    }));

handler
    .command('deposit')
    .description('deposit task on specified chains')
    .requiredOption('--chain <chain>', 'chain name', null)
    .requiredOption('--asset <asset>', 'asset address or encoded location', null)
    .requiredOption('--amount <amount>', 'amount of the asset to deposit', null)
    .requiredOption('--recipient <recipient>', 'recipient account on dest chain', null)
    .requiredOption('--worker <worker>', 'worker to run the task', null)
    .requiredOption('--id <id>', 'pre-generated id of the task', null)
    .requiredOption('--data <data>', 'data(solution) of the task', null)
    .requiredOption('--key <key>', 'key of depositor', null)

    .action(run(async (opt) => {
        let config = useConfig();
        if (useChainType(opt.chain.toLowerCase()) === 'Evm') {
            let handler = useEvmHandler(config, opt.chain.toLowerCase(), opt.key)
            let tx = await handler.deposit(
                opt.asset,
                opt.amount,
                opt.recipient,
                opt.worker,
                opt.id,
                opt.data,
                {
                //   gasLimit: 2000000,
                }
            );
            console.log(`Deposited task on ${opt.chain}: ${tx.hash}`);
        } else {    // Sub
            throw new Error("not implemented")
        }
    }));

handler
    .command('task')
    .description('list actived tasks that belong to the given worker')
    .requiredOption('--chain <chain>', 'chain name', null)
    .requiredOption('--worker <worker>', 'woker H160 address on EVM chain, sr25519 public key on substrate chain', null)
    .action(run(async (opt) => {
        // TODO
    }));

handler
    .command('balance')
    .description('Return balance of the given asset that handler holds')
    .requiredOption('--chain <chain>', 'chain name', null)
    .requiredOption('--asset <asset>', 'asset H160 address on EVM chain, encoded MultiLocation on substrate chain', null)
    .action(run(async (opt) => {
        // TODO
    }));

const worker = program
.command('worker')
.description('inDEX worker account management');

worker
    .command('list')
    .description('list worker accounts')
    .option('--worker <worker>', 'worker sr25519 public key', null)
    .action(run(async (opt) => {
        let { uri } = program.opts();
        let config = useConfig();
        let api = await useApi(config.node_wss_endpoint);
        let executor = await useExecutor(api, config.pruntine_endpoint, config.executor_contract_id);
        let cert = await useCert(uri, api);
        let ret = (await executor.query.getWorkerAccounts(cert,
            {},
        ));
        let workers = ret.output.asOk.toJSON().ok;
        if (opt.worker !== null) {
            console.log(workers.find(worker => worker.account32.toLowerCase() === opt.worker.toLowerCase()));
        } else {
            console.log(JSON.stringify(workers, null, 2));
        }
    }));

worker
    .command('approve')
    .description('approve ERC20 for specific asset')
    .requiredOption('--worker <worker>', 'worker sr25519 public key', null)
    .requiredOption('--chain <chain>', 'chain name', null)
    .requiredOption('--token <token>', 'ERC20 token contract address', null)
    .requiredOption('--spender <spender>', 'spender H160 address', null)
    .requiredOption('--amount <amount>', 'the amount set to allowance', null)

    .action(run(async (opt) => {
        let { uri } = program.opts();
        let config = useConfig();
        let api = await useApi(config.node_wss_endpoint);
        let executor = await useExecutor(api, config.pruntine_endpoint, config.executor_contract_id);
        let cert = await useCert(uri, api);

        console.log(`Call Executor::worker_approve to approve ERC20 for specific asset`);
        let queryRecipient = await executor.query.workerApprove(cert,
            {},
            opt.worker,
            opt.chain.toLowerCase(),
            opt.token,
            opt.spender,
            opt.amount,
        );
        console.log(`Query recipient: ${JSON.stringify(queryRecipient, null, 2)}`);
    }));

worker
    .command('balance')
    .description('get the firee blance of the worker account on specific chain')
    .requiredOption('--chain <chain>', 'Chain name', null)
    .requiredOption('--asset <asset>', 'Asset location<smart contract address for encoded multilocation>', null)
    .requiredOption('--worker <worker_account>', 'Worker account', null)
    .action(run (async (opt) => {
        if (opt.chain) {
            let config = useConfig();
            let endpoint = useChainEndpoint(config, opt.chain.toLowerCase());
            if (useChainType(opt.chain.toLowerCase()) === 'Evm') {
                let provider = useEtherProvider(endpoint);
                if (opt.asset === null) {
                    console.log(await provider.getBalance(opt.worker));
                } else {
                    let token = useERC20Token(provider, opt.asset);
                    console.log(await token.balanceOf(opt.worker));
                }
            } else {    // Sub
                let api = await useApi(endpoint);
                if (opt.asset === null) {
                    const accountData = await api.query.system.account(opt.worker);
                    const freeBalance = accountData.data.free.toString();
                    console.log(freeBalance);
                } else {
                    throw new Error(`Not support balance query for asset: ${opt.asset}`);
                }
            }
        } else {
            throw new Error("Please provide the chain name");
        }
    }));


program.parse(process.argv);
