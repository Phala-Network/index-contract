# Play With inDEX

Suppose we are playing on poc5, after deployed executor and keystore contract, add the owner account mnemonic and cloud storage base url, access token (only support Google firebase so far) to the .env file

```sh
URI="<your mnemonic>"
STORAGE_URL="<your storage base url>"
STORAGE_KEY="<your storage access token>"
```

then update the value of `executor_contract_id` and `key_store_contract_id` in `scripts/src/config.poc5.json` with the proper contract id that you deployed just now.

Now we are ready to start contract configuration

## [1] Whitelist executor contract id on keystore contract

Instead of change worker accounts everytime we deployed executor engine, we split the worker accounts generation to keystore contract. Only whitelisted executor contract id can get private keys of worker accounts from keystore contract, `cd scripts` and execute

```sh
node src/console.js --config src/config.poc5.json keystore set-executor
```

## [2] Config executor contract

We need to config executor contract after deployed, stuff contains:
1) import worker key from KeyStore contract (call `executor.config`);
3) setup worker account in remote storage (call `executor.setup_worker_on_storage`);
3) [Optional] resume executor (call `executor.resume_executor`);

Now, issue command

```sh
node src/console.js --config src/config.poc5.json executor setup --resume
```

with `--resume` will unpause the executor (executor is paused by default after deployed). You will get the output like below:
```sh
✅ Config executor
✅ Config storage
✅ Resume executor
🎉 Finished executor configuration!
```

## Run scheduler

The scheduler is responsible for scheduling the execution of inDEX engine. It will call `executor.run` periodically to 1) fetch tasks that distributed to a specific worker from handler on source chain and 2) run tasks that are successfully claimed by the worker.

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
