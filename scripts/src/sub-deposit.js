require('dotenv').config()
const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api')
const BN = require('bn.js');
const bn1e12 = new BN(10).pow(new BN(12));

const PHA_ON_KHALA = '0x010100511f';
const PHA_ON_ETHEREUM = '0xB376b0Ee6d8202721838e76376e81eEc0e2FE864';
const WETH_ON_ETHEREUM = '0xB4FBF271143F4FBf7B91A5ded31805e42b2208d6';
const WORKER_PUBKEY = '0x2eaaf908adda6391e434ff959973019fb374af1076edd4fec55b5e6018b1a955'

/******** OperationJson definition in Rust **************
struct OperationJson {
    op_type: String,
    source_chain: String,
    dest_chain: String,
    spend_asset: Address,
    receive_asset: Address,
    dex: String,
    fee: String,
    cap: String,
    flow: String,
    impact: String,
    spend: String,
}
******************************************************/

function createRequest() {
  let operations = [
    {
      op_type: 'bridge',
      source_chain: 'Khala',
      dest_chain: 'Ethereum',
      spend_asset: PHA_ON_KHALA,
      receive_asset: PHA_ON_ETHEREUM,
      dex: 'null',
      fee: '300',
      cap: '0',
      flow: '301000000000000',
      impact: '0',
      // 1 PHA will be briged, with decimals 12
      spend: '301000000000000',
    },
    {
        op_type: 'swap',
        source_chain: 'Ethereum',
        dest_chain: 'Ethereum',
        spend_asset: PHA_ON_ETHEREUM,
        receive_asset: WETH_ON_ETHEREUM,
        dex: 'UniswapV2',
        fee: '0',
        cap: '0',
        flow: '1000000000000000000',
        impact: '0',
        // 1 PHA with decimals 18
        spend: '1000000000000000000',
      },
  ]
  return JSON.stringify(operations)
}

function getPhaAssetId(api) {
    return api.createType('XcmV3MultiassetAssetId', {
        Concrete: api.createType('XcmV3MultiLocation', {
            parents: 0,
            interior: api.createType('Junctions', 'Here')
        })
    })
}

async function main() {
    const api = await ApiPromise.create({
        provider: new WsProvider(process.env.ENDPOINT || 'ws://localhost:9944'),
    });
    const alice = new Keyring({ type: 'sr25519' }).addFromUri('//Alice');

    return new Promise(async (resolve) => {
        const unsub = await api.tx.palletIndex.depositTask(
            getPhaAssetId(api),
            api.createType('Compact<U128>', bn1e12.mul(new BN(301))),
            // Recipient address on Ethereum
            '0xA29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20',
            WORKER_PUBKEY,
            // Request id
            '0x0000000000000000000000000000000000000000000000000000000000000003',
            createRequest()
        ).signAndSend(alice, (result) => {
            if (result.status.isInBlock) {
                console.log(`Transaction included at blockHash ${result.status.asInBlock}`);
            } else if (result.status.isFinalized) {
                console.log(`Transaction finalized at blockHash ${result.status.asFinalized}`);
                unsub();
                resolve();
            }
        });
    });
}

main()
  .catch(console.error)
  .finally(() => process.exit())
