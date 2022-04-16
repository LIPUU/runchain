// block.rs是个模块，它里面不能写mod。它只能写use。并且它use的模块必须被所有bin文件都mod进，不然就等于没有被纳入编译树
use crate::protocol::DIFFICULTY_PREFIX;
use serde::{Deserialize, Serialize};
// 放一些block相关的数据结构和逻辑函数
type Hash = String;
use chrono::prelude::*;
use std::io::Error;
type Timestamp = String;
use crate::pow;
pub struct Chain {
    blocks: Vec<Block>,
}
impl Chain {
    pub fn new() -> Self {
        let genesis_block = Block {
            height: 0,
            previous_hash: "This is RunChain's first block".to_string(),
            timestamp: Utc::now().to_string(),
            merkel_root: "".to_string(),
            nonce: 0,
            upinfo: vec![
                format!("{}{}", "Tonight,you are so beautiful. ", Utc::now()),
                format!("{}{}", "I want you more than any other time. ", Utc::now()),
            ],
        };
        Chain {
            blocks: vec![genesis_block],
        }
    }

    pub fn genesis_hash(&self) -> String {
        let genesis_block = self.blocks.first().unwrap();
        self.calculate_hash(genesis_block).unwrap()
    }
    pub fn block_height(&self) -> usize {
        self.blocks.len()
    }

    pub fn try_add_a_block(&mut self, block: Block) -> Result<(), &str> {
        if self.is_block_vaild(&block) {
            self.blocks.push(block);
            return Ok(());
        }
        Err("add a block error because it is not a vaild block")
    }

    pub fn is_block_vaild(&self, block: &Block) -> bool {
        let previous_block = self.blocks.last().unwrap();
        let previous_block_hash = self.calculate_hash(previous_block).unwrap();
        if block.previous_hash != previous_block_hash {
            println!("block with height:{} has wrong previous hash", block.height);
            return false;
        }

        if !self.calculate_hash(block).unwrap().starts_with("0000") {
            println!("block with height: {} has invalid difficulty", block.height);
            return false;
        }

        if block.height + 1 != previous_block.height {
            println!("block with height: {} has invalid height", block.height);
            return false;
        }

        true
    }

    fn calculate_hash(&self, block: &Block) -> Result<String, Error> {
        let s = format!(
            "{}{}{}{}{}",
            block.height, block.previous_hash, block.timestamp, block.merkel_root, block.nonce
        );
        let s = s.as_bytes();
        let hasher = pow::blake3_base_hash(s);
        let v = hasher.finalize().as_bytes().to_vec();
        let hash = std::str::from_utf8(&v).unwrap();
        Ok(String::from(hash))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    pub height: u64,
    pub previous_hash: Hash,
    pub timestamp: Timestamp,
    pub merkel_root: Hash,
    pub nonce: u128,
    upinfo: Vec<String>,
}
