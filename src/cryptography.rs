use ed25519_dalek::{Signer, Verifier};

pub fn sign<'a, T>(original: &[u8], signing_key: &T) -> Vec<u8>
where
    T: Signer<ed25519::Signature>,
{
    let signature: &[u8; 64] = &signing_key.sign(&original).into();
    let result: Vec<u8> = signature.iter().map(|n| *n).collect();
    assert!(result.len() == 64);
    result
}

pub fn verify(public_key: &Vec<u8>, original_message: &String, signature: &Vec<u8>) -> bool {
    assert!(signature.len() == 64);
    // &Vec<u8> -> &[u8]-> [u8;64]-> ed25519_dalek::Signature
    let signature: &[u8] = &*signature; // as_ref
    let signature: [u8; 64] = signature.try_into().unwrap();
    let signature: ed25519_dalek::Signature = signature.into();

    // 思路和上面类似，&Vec<u8> -> &[u8] -> public_key
    let public_key = ed25519_dalek::PublicKey::from_bytes(public_key).unwrap();
    public_key
        .verify(original_message.as_bytes(), &signature)
        .is_ok()
}
