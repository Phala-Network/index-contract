# InDex PhatContract Repo

![inDex Execution Engine Diagram](./doc/img/index%20execution%20engine.png)

## Terminology

- Task

    A Task contains bunch of steps, each of step represents an operation on a blockchain. It can be one of `Bridge`, `Swap`, and `Transfer`

- Request

    A request contains task information and worker account that will be used to execute the task

- Handler

    Proxy contract deployed on every source chain that we supported. Handler contract contains a task queue that only whitelisted workers can claim(consume) task from it. Solidity version implementation can be found [here](https://github.com/Phala-Network/index-solidity)

- Worker

    An account used to temporary hold user assets and send transaction to blockchain. Worker accounts are pre-derived inside Phat contract, those key will never reveal for the outside

## Prepare environment

If your are new for Ink! smart contract language, please head to [Parity Ink document](https://paritytech.github.io/ink/)
or you can checkout our [PhatContract document](https://wiki.phala.network/en-us/build/general/intro/) for the details.

- Install dependencies

```sh
 $ yarn
```

## Compile contract

This repo contains several contracts, each of them can be compiled and deployed individually, for example build `<target-contract>`:

```sh
 $ cd contracts/<target-contract>
 $ cargo contract build
```

or use Devphase

```sh
 $ yarn devphase compile
```

You can also specify which the contract to build by adding the contract name. The name should be
in snake case, consistent with the directory names under `contracts/`.

## Test with Devphase

```sh
 $ yarn devphase test
```

## Launch a standalone local test stack for custom testing

1. start the local stack.

```sh
 $ yarn devphase stack
```

2. Init the testnet (currently by [this script](https://github.com/shelvenzhou/phala-blockchain-setup))

```sh
# edit .env file
 $ node src/setup-drivers.js
```

3. You can also dump the contract log from the log server driver with the same scripts:

```sh
 $ node src/dump-logs.js
```

The tests are written in TypeScript at `./tests/*.test.ts`. The logs are output to `./logs/{date}`
directory.

## Deploy contract on live network

You can either use [phala/sdk](https://github.com/Phala-Network/js-sdk) or the [Webpage App](https://phat.phala.network/) deploy the contract, we highly recommend use the Webpage App to save your time.
