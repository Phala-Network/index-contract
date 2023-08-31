const fs = require('fs')
const { OnChainRegistry, PinkContractPromise } = require('@phala/sdk')
const PhalaSdk = require('@phala/sdk')

function loadContractFile(contractFile) {
    const metadata = JSON.parse(fs.readFileSync(contractFile, 'utf8'))
    const constructor = metadata.spec.constructors.find(
      c => c.label == 'default',
    ).selector
    const name = metadata.contract.name
    const wasm = metadata.source.wasm
    return { wasm, metadata, constructor, name }
}

async function createContract(api, _pruntimeUrl, contract, contractID) {
    const phatRegistry = await OnChainRegistry.create(api);
    const contractKey = await phatRegistry.getContractKey(contractID)
    return new PinkContractPromise(
      api,
      phatRegistry,
      contract.metadata,
      contractID,
      contractKey,
    );
}

async function delay(ms) {
  return new Promise( resolve => setTimeout(resolve, ms) );
}

module.exports = {
    loadContractFile,
    createContract,
    delay,
}
