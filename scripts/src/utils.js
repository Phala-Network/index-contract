const fs = require('fs')
const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api')
const { ContractPromise } = require('@polkadot/api-contract')
const PhalaSdk = require('@phala/sdk')
const PhalaSDKTypes = PhalaSdk.types

function loadContractFile(contractFile) {
    const metadata = JSON.parse(fs.readFileSync(contractFile, 'utf8'))
    const constructor = metadata.spec.constructors.find(
      c => c.label == 'default',
    ).selector
    const name = metadata.contract.name
    const wasm = metadata.source.wasm
    return { wasm, metadata, constructor, name }
}

async function createContract(api, pruntimeUrl, contract, contractID) {
    const { api: workerApi } = await PhalaSdk.create({
      api,
      baseURL: pruntimeUrl,
      contractId: contractID,
      autoDeposit: true,
    })
    const contractApi = new ContractPromise(
      workerApi,
      contract.metadata,
      contractID,
    )
    return contractApi
}

module.exports = {
    loadContractFile,
    createContract,
}
