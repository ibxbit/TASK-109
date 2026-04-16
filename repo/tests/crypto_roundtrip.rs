//! Integration test: end-to-end FieldCipher encrypt/decrypt across
//! cipher instances built from the same key (simulates two separate
//! processes sharing the same FIELD_ENCRYPTION_KEY).

use vitalpath::crypto::FieldCipher;

fn key() -> [u8; 32] {
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(13).wrapping_add(1);
    }
    k
}

#[test]
fn ciphertext_decryptable_by_a_separate_cipher_with_same_key() {
    let writer = FieldCipher::new(&key(), "v1");
    let (ct, nonce) = writer.encrypt("payload").unwrap();

    let reader = FieldCipher::new(&key(), "v1");
    assert_eq!(reader.decrypt(&ct, &nonce).unwrap(), "payload");
}

#[test]
fn many_unique_ciphertexts_are_all_decryptable() {
    let cipher = FieldCipher::new(&key(), "v1");
    let plaintexts: Vec<String> = (0..32).map(|i| format!("note-{}", i)).collect();
    let encrypted: Vec<_> = plaintexts
        .iter()
        .map(|p| cipher.encrypt(p).unwrap())
        .collect();

    // Every ciphertext is unique (because the nonce is fresh).
    let unique_cts: std::collections::HashSet<_> =
        encrypted.iter().map(|(ct, _)| ct.clone()).collect();
    assert_eq!(unique_cts.len(), plaintexts.len());

    for (plain, (ct, nonce)) in plaintexts.iter().zip(encrypted.iter()) {
        assert_eq!(&cipher.decrypt(ct, nonce).unwrap(), plain);
    }
}

#[test]
fn cross_key_decryption_always_fails() {
    let writer = FieldCipher::new(&key(), "v1");
    let (ct, nonce) = writer.encrypt("private").unwrap();

    let mut other_key = key();
    other_key[31] ^= 0x01; // Flip a single bit.
    let reader = FieldCipher::new(&other_key, "v1");
    assert!(reader.decrypt(&ct, &nonce).is_err());
}
