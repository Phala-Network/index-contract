require('dotenv').config();

const fs = require('fs');
const path = require('path')
const { program } = require('commander');
const { Decimal } = require('decimal.js');
const BN = require('bn.js');
const { ApiPromise, Keyring, WsProvider } = require('@polkadot/api');
const { cryptoWaitReady } = require('@polkadot/util-crypto');
const ethers = require('ethers')
const PhalaSdk = require('@phala/sdk')
const PhalaSDKTypes = PhalaSdk.types
const KhalaTypes = require('@phala/typedefs').khalaDev
const { loadContractFile, createContract } = require('./utils');

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
    const chain = config.chains.find(chain => Object.keys(chain)[0] === chainName);
    const endpoint = chain && Object.values(chain)[0];
    return endpoint;
}

function useChainType(chain) {
    if (['ethereum', 'goerli', 'moonbeam', 'astar'].includes(chain)) {
        return 'Evm';
    } else if (['poc3', 'poc5', 'khala', 'phala', 'acala'].includes(chain)) {
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

function useEvmHandler(provider, handler) {
    return new ethers.Contract(
        handler,
        HandlerABI,
        provider
    )
}

async function useExecutor(api, pruntime_endpoint, contract_id) {
    const contract = loadContractFile(
        path.join(__dirname, '../../target/ink/index_executor/index_executor.contract'),
    )
    console.log(`Connected to node, create contract object`)
    return await createContract(api, pruntime_endpoint, contract, contract_id)
}

program
    .option('--config <path>', 'config that contains contract and node informations', process.env.CONFIG || 'config.json')
    .option('--uri <path>', 'the account URI use to sign cert', process.env.URI || '//Alice')


const executor = program
.command('executor')
.description('inDEX executor');

const hander = program
.command('hander')
.description('inDEX handler contract/pallet');

const task = program
.command('task')
.description('inDEX task inspector');

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
    .requiredOption('--worker <worker>', 'worker H160 address', null)
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
            opt.chain.charAt(0).toUpperCase() + opt.chain.slice(1).toLowerCase(),
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
            let endpoint = useChainEndpoint(config, opt.chain.charAt(0).toUpperCase() + opt.chain.slice(1).toLowerCase());
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
