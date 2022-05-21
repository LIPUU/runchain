use crate::block;
use crate::protocol::DIFFICULTY_PREFIX;
use block::Block;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
// 接受&[u8]作参数的函数其实能够传&Vec<u8>过去

// 最终通过计算得到hash，并把计算hash过程中得到的nonce返回
pub fn pow_v2(block: Block, atomicbool: &AtomicBool) -> (u128, bool) {
    let s = format!(
        "{}{:?}{}{:?}",
        block.height, block.previous_hash, block.timestamp, block.merkle_root
    );

    let nonce = (0..u128::MAX)
        .into_par_iter()
        .find_any(|n| {
            if !atomicbool.load(Ordering::Relaxed) {
                // 外界不准继续计算，直接停止计算
                return true;
            }
            let hash = hash_add_n(&s, *n);
            &hash[..DIFFICULTY_PREFIX.len()] == DIFFICULTY_PREFIX // Vec<u8>和&[u8]的关系
        })
        .unwrap();

    let mut flag = true;
    if !atomicbool.load(Ordering::Relaxed) {
        flag = false;
    };

    (nonce, flag)
    // 如果计算是因为标志位变成了false，说明是外界不允许继续计算下去了，返回值的flag设置为false表明该nonce无效
}

fn hash_add_n(s: &String, nonce: u128) -> Vec<u8> {
    let mut hash = Sha256::new();
    let s = s.as_bytes();
    hash.update(s);
    hash.update(format!("{}", nonce));
    hash.finalize().to_vec()
}
