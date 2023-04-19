# Play With inDEX

Suppose we are playing on poc5, after deployed executor and keystore contract, add the owner account mnemonic to the .env file

```sh
URI="<your mnemonic"
```
then update `executor_contract_id` and `key_store_contract_id` in `scripts/src/config.poc5.json` with the proper contract id

Now we are ready to start config executor and keystore contract

## Whitelist executor id on keystore contract

Only whitelisted executor id can get private keys of worker account from keystore contract, `cd scripts` and execute

```sh
node src/console.js --config src/config.poc5.json keystore set-executor
```

## Config executor contract

### 1. Fund executor account
- first query executor account (derived inside executor contract, not executor contract id) by executing

```sh
node src/console.js --config src/config.poc5.json executor account --balance
```

with `--balance` will get balance returned

- send some PHA to executor account to pay transaction fee when config rollup

### 2. Setup executor

We need to config executor contract after deployed, stuff contains:
1) import worker key from KeyStore contract (call executor.config);
2) claim rollup storage (call executor.setup_rollup);
3) setup worker account in rollup storage (call executor.setup_worker_on_rollup);
4) resume executor (call executor.resume_executor);

Now, issue command

```sh
node src/console.js --config src/config.poc5.json executor setup --resume
```

with `--resume` will unpause the executor (executor is paused by default after deployed)

### 3. Set graph

Use tablizer tool, clone source code from [here](), and
```sh
rm db.sqlite && ./bin/dev parse -c dotflow.yaml

./bin/dev contract -n wss://poc5.phala.network/ws -r https://poc5.phala.network/tee-api-1 -a <executor contract id> -s "<mnemonic>" --set
```

## Run scheduler

- get worker account info

```sh
node src/console.js --config src/config.poc5.json worker list
```

- update worker account in config.poc5.json
- execute `node src/scheduler.js`

** Don't forget fund the worker account on specific chain to pay gas fee **

## Others

- You may need to let worker approve the handler contract

For example you want to approve `Moonbeam bridge contract: 0x70085a09D30D6f8C4ecF6eE10120d1847383BB57` sepnd your  asset:

```sh
node src/console.js --config src/config.poc5.json worker approve \
--worker <worker account32> \
--chain Moonbeam \
--token 0xAcc15dC74880C9944775448304B263D191c6077F \
--spender 0x70085a09D30D6f8C4ecF6eE10120d1847383BB57 \
--amount 10000000000000000000000
```

This command will call `executor.worker_approve` where worker account will send ERC20 approve transaction on behalf of the call

- You may need to deposit your task with handler contract

Suppose you got solution data like below:

```sh
SOLUTION_DATA="[{\"op_type\":\"swap\",\"source_chain\":\"Moonbeam\",\"dest_chain\":\"Moonbeam\",\"spend_asset\":\"0xAcc15dC74880C9944775448304B263D191c6077F\",\"receive_asset\":\"0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080\",\"dex\":\"StellaSwap\",\"fee\":\"0\",\"cap\":\"0\",\"flow\":\"2000000000000000000\",\"impact\":\"0\",\"spend\":\"2000000000000000000\"},{\"op_type\":\"bridge\",\"source_chain\":\"Moonbeam\",\"dest_chain\":\"Acala\",\"spend_asset\":\"0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080\",\"receive_asset\":\"0x010200411f06080002\",\"dex\":\"null\",\"fee\":\"0\",\"cap\":\"0\",\"flow\":\"1400000000\",\"impact\":\"0\",\"spend\":\"1400000000\"}]"
```

then deposit task in `Handler` contract we deployed on Moonbeam for test:

```sh
node src/console.js --config src/config.poc5.json handler deposit --chain Moonbeam \
--asset 0xAcc15dC74880C9944775448304B263D191c6077F \
--amount 2000000000000000000 \
--recipient 0x7804e66ec9eea3d8daf6273ffbe0a8af25a8879cf43f14d0ebbb30941f578242 \
--worker "0xbfd542cf8d41e84b70a96c9b379913be6917acfb" \
--id "0x0000000000000000000000000000000000000000000000000000000000000005" \
--data  $SOLUTION_DATA \
--key <sender private key>
```
Note `--id` specifies the task id, you can generate it on your way
