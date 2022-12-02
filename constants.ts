export class Chain_Ethereum {
    "name": "Ethereum"
    "chainType": "Evm"
    "native": null
    "stable": null
    "endpoint": "https://rinkeby.infura.io/v3/6d61e7957c1c489ea8141e947447405b"
    "network": null
}

export class Chain_Khala {
    "name": "Khala"
    "chainType": "Sub"
    "native": null
    "stable": null
    "endpoint": "wss://khala-api.phala.network/ws"
    "network": null
}

export class Chain_Karura {
    "name": "Karura"
    "chainType": "Sub"
    "native": null
    "stable": null
    "endpoint": "wss://karura-rpc-0.aca-api.network"
    "network": null
}

export class Asset_Pha_Ethereum {
    "name": "Phala Token"
    "symbol": "PHA"
    "decimals": 18
    "location": "0x6c5ba91642f10282b576d91922ae6448c9d52f4e"
}

export class Asset_Weth_Ethereum {
    "name": "Wrapped Ether"
    "symbol": "WETH"
    "decimals": 18
    "location": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
}

export class Asset_Pha_Khala {
    "name": "Phala Token"
    "symbol": "PHA"
    "decimals": 12
    // Encode of MultiLocation: (1, X1(Parachain(2004)))
    "location": "0x010100511f"
}

export class Asset_Pha_Karura {
    "name": "Phala Token"
    "symbol": "PHA"
    "decimals": 12
    // Encode of MultiLocation: (1, X1(Parachain(2004)))
    "location": "0x010100511f"
}

export class Asset_Kar_Karura {
    "name": "Karura"
    "symbol": "KAR"
    "decimals": 12
    // Encode of MultiLocation: (1, X2(Parachain(2000), GeneralKey(0x0080)))
    "location": "0x010200411f06080080"
}

export class Asset_Kusd_Karura {
    "name": "Karura USD"
    "symbol": "kUSD"
    "decimals": 12
    // Encode of MultiLocation: (1, X2(Parachain(2000), GeneralKey(0x0081)))
    "location": "0x010200411f06080081"
}

export class Bridge_Ethereum2Khala_Pha {
    asset0: Asset_Pha_Ethereum
    asset1: Asset_Pha_Khala
}

export class Bridge_Khala2Ethereum_Pha {
    asset0: Asset_Pha_Khala
    asset1: Asset_Pha_Ethereum
}

export class Bridge_Khala2Karura_Pha {
    asset0: Asset_Pha_Khala
    asset1: Asset_Pha_Karura
}

export class Dex_UniswapV2_Pha_Weth_Ethereum {
    id: "0x8867f20c1c63baccec7617626254a060eeb0e61e"
    asset0: Asset_Pha_Ethereum
    asset1: Asset_Weth_Ethereum
    swap_fee: 0
    dev_fee: 0
}

export class Dex_KaruraSwap_Kar_Kusd_Karura {
    id: "lp://KAR/KUSD"
    asset0: Asset_Kar_Karura
    asset1: Asset_Kusd_Karura
    swap_fee: 0
    dev_fee: 0
}

export class Dex_KaruraSwap_Kusd_Pha_Karura {
    id: "lp://KUSD/PHA"
    asset0: Asset_Kar_Karura
    asset1: Asset_Pha_Karura
    swap_fee: 0
    dev_fee: 0
}
