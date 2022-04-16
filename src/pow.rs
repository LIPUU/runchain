use crate::block;
use crate::protocol::DIFFICULTY_PREFIX;
use block::Block;
use rayon::prelude::*;

// 接受&[u8]作参数的函数其实能够传&Vec<u8>过去

// 最终通过计算得到hash，并把计算hash过程中得到的nonce返回
pub fn pow_v2(block: Block) -> u128 {
    let s = format!(
        "{}{}{}{}{}",
        block.height, block.previous_hash, block.timestamp, block.merkel_root, block.nonce
    );
    let s = s.as_bytes();
    let hasher = blake3_base_hash(s);
    let nonce = (0..u128::MAX)
        .into_par_iter()
        .find_any(|n| {
            let hash = blake3_hash(hasher.clone(), *n);
            &hash[..DIFFICULTY_PREFIX.len()] == DIFFICULTY_PREFIX
        })
        .unwrap();
    nonce
}

// pow hash: Block data + nonce (BE) => hash
pub fn blake3_hash(mut hasher: blake3::Hasher, nonce: u128) -> Vec<u8> {
    hasher.update(&nonce.to_be_bytes()[..]);
    hasher.finalize().as_bytes().to_vec()
}

// 从原始值得到Hash，为了后续操作的方便，用Hasher类型把Hash值包裹起来
pub fn blake3_base_hash(data: &[u8]) -> blake3::Hasher {
    let mut hasher = blake3::Hasher::new();
    hasher.update(data);
    hasher
}
