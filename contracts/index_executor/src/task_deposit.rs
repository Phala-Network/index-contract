use crate::step::StepInput;
use crate::task::Task;
use alloc::{string::String, vec, vec::Vec};
use pink_web3::{
    contract::{tokens::Detokenize, Error as PinkError},
    ethabi::Token,
    types::{Address, U256},
};
use scale::{Decode, Encode};
use xcm::v3::AssetId as XcmAssetId;

#[derive(Debug)]
#[allow(dead_code)]
pub struct EvmDepositData {
    sender: Address,
    token: Address,
    amount: U256,
    pub task: Option<Vec<u8>>,
}

impl Detokenize for EvmDepositData {
    fn from_tokens(tokens: Vec<Token>) -> Result<Self, PinkError>
    where
        Self: Sized,
    {
        if tokens.len() == 1 {
            let deposit_raw = tokens[0].clone();
            match deposit_raw {
                Token::Tuple(deposit_data) => {
                    match (
                        deposit_data[0].clone(),
                        deposit_data[1].clone(),
                        deposit_data[2].clone(),
                    ) {
                        (Token::Address(sender), Token::Address(token), Token::Uint(amount)) => {
                            Ok(EvmDepositData {
                                sender,
                                token,
                                amount,
                                task: None,
                            })
                        }
                        _ => Err(PinkError::InvalidOutputType(String::from(
                            "Return type dismatch",
                        ))),
                    }
                }
                _ => Err(PinkError::InvalidOutputType(String::from(
                    "Unexpected output type",
                ))),
            }
        } else {
            Err(PinkError::InvalidOutputType(String::from("Invalid length")))
        }
    }
}

// Copy from pallet-index
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct SubDepositData {
    pub sender: [u8; 32],
    pub asset: XcmAssetId,
    pub amount: u128,
    pub recipient: Vec<u8>,
    pub task: Vec<u8>,
}

// Define the structures to parse deposit data json
#[allow(dead_code)]
#[derive(Debug)]
pub struct DepositData {
    sender: Vec<u8>,
    amount: u128,
    recipient: Vec<u8>,
    task: Vec<u8>,
}

impl TryFrom<EvmDepositData> for DepositData {
    type Error = &'static str;
    fn try_from(value: EvmDepositData) -> Result<Self, &'static str> {
        Ok(Self {
            sender: value.sender.as_bytes().into(),
            amount: value.amount.try_into().expect("Amount overflow"),
            recipient: vec![],
            task: value.task.ok_or("MiisingSolution")?,
        })
    }
}

impl From<SubDepositData> for DepositData {
    fn from(value: SubDepositData) -> Self {
        Self {
            sender: value.sender.into(),
            amount: value.amount,
            recipient: value.recipient,
            task: value.task,
        }
    }
}

impl DepositData {
    pub fn to_task(
        &self,
        source_chain: &str,
        id: [u8; 32],
        worker: [u8; 32],
    ) -> Result<Task, &'static str> {
        pink_extension::debug!("Trying to parse task data from json string");

        let execution_plan: ExecutionPlan =
            Decode::decode(&mut self.task.as_slice()).map_err(|_| "InvalidTask")?;
        pink_extension::debug!(
            "Parse task data successfully, found {:?} operations",
            execution_plan.len()
        );
        if execution_plan.is_empty() {
            return Err("EmptyTask");
        }
        pink_extension::debug!("Trying to convert task data to task");

        let mut uninitialized_task = Task {
            id,
            source: source_chain.into(),
            sender: self.sender.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            worker,
            ..Default::default()
        };

        for step_input in execution_plan.iter() {
            uninitialized_task
                .steps
                .push(step_input.clone().try_into()?);
        }

        Ok(uninitialized_task)
    }
}

type ExecutionPlan = Vec<StepInput>;
