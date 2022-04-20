// block.rs是个模块，它里面不能写mod。它只能写use。并且它use的模块必须被所有bin文件都mod进，不然就等于没有被纳入编译树
use crate::protocol::DIFFICULTY_PREFIX;
use rs_merkle::{algorithms::Sha256, Hasher, MerkleTree};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256 as sha2_sha256};
// 放一些block相关的数据结构和逻辑函数
type Hash = String;
use chrono::prelude::*;
use std::io::Error;
type Timestamp = String;
use crate::pow;

#[derive(Clone)]
pub struct Chain {
    blocks: Vec<Block>,
}

// &[[u8;32]]
impl Chain {
    pub fn new() -> Self {
        let first=[Sha256::hash("This is RunChain's first block".as_bytes())];
        let merkle_tree = MerkleTree::<Sha256>::from_leaves(&first);

        let merkle_root = match merkle_tree.root() {
                Some(value) => value,
                None => {
                    
                    [202, 151, 129, 18, 202, 27, 189, 202, 250, 194, 49, 179, 154, 35, 220, 77, 167, 134, 239, 248, 20, 124, 78, 114, 185, 128, 119, 133, 175, 238, 72, 187]
                }
            };

        let merkle_root:[u8;32] = merkle_root;

        let upinfo=vec![
            format!("{}{}", "Tonight,you are so beautiful. ", Utc::now()),
            format!("{}{}", "I want you more than any other time. ", Utc::now()),
        ];


        let genesis_block = Block {
            height: 0,
            previous_hash:vec![202, 151, 129, 18, 202, 27, 189, 202, 250, 194, 49, 179, 154, 35, 220, 77, 167, 134, 239, 248, 20, 124, 78, 114, 185, 128, 119, 133, 175, 238, 72, 187],
            timestamp: Utc::now().to_string(),
            merkle_root,
            nonce: 0,
            upinfo
        };
        Chain {
            blocks: vec![genesis_block],
        }
    }

    pub fn show_chain(&self){
        for item in &self.blocks{
            println!("💋block:{:?}",item)
        }
    }

    pub fn genesis_hash(&self) -> Vec<u8> {
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

    pub fn last_block(&self) -> &Block {
        self.blocks.last().unwrap()
    }

    pub fn last_n_blocks(&self, n: usize) -> Vec<Block> {
        assert!(n <= self.blocks.len());
        self.blocks
            .clone()
            .into_iter()
            .skip(self.blocks.len() - n)
            .collect()
    }

    pub fn is_block_vaild(&self, block: &Block) -> bool {
        let previous_block = self.last_block();
        let previous_block_hash = self.calculate_hash(previous_block).unwrap();
        if block.previous_hash != previous_block_hash {
            println!("block with height:{} has wrong previous hash", block.height);
            return false;
        }

        if &self.calculate_hash(block).unwrap()[..DIFFICULTY_PREFIX.len()]!=DIFFICULTY_PREFIX {
            println!("block with height: {} has invalid difficulty", block.height);
            return false;
        }

        if block.height != previous_block.height+1 {
            println!("block with height: {} has invalid height", block.height);
            return false;
        }

        true
    }

    pub fn calculate_hash(&self, block: &Block) -> Result<Vec<u8>, Error> {
        let mut hasher = sha2_sha256::new();
        let s = format!(
            "{}{:?}{}{:?}{}",
            block.height, block.previous_hash, block.timestamp, block.merkle_root, block.nonce
        );
        let s = s.as_bytes();
        hasher.update(s);
        Ok(hasher.finalize().to_vec())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub height: usize,
    pub previous_hash: Vec<u8>,
    pub timestamp: Timestamp,
    pub merkle_root: [u8;32],
    pub nonce: u128,
    pub upinfo: Vec<String>,
}
