use pgp::composed::{
    key::SecretKeyParamsBuilder, KeyType, Message as OpenPGPMessage, SignedPublicKey,
    SignedSecretKey,
};
use pgp::crypto::{hash::HashAlgorithm, sym::SymmetricKeyAlgorithm};
use pgp::packet::LiteralData;
use pgp::types::KeyTrait;
use pgp::types::{CompressionAlgorithm, SecretKeyTrait};
use smallvec::*;

fn create_key() -> (SignedSecretKey, SignedPublicKey) {
    let mut key_params = SecretKeyParamsBuilder::default();
    key_params
        .key_type(KeyType::Rsa(2048))
        .can_create_certificates(false)
        .can_sign(true)
        .primary_user_id("WICRS <wicrs@wic.rs>".into())
        .preferred_symmetric_algorithms(smallvec![SymmetricKeyAlgorithm::AES256,])
        .preferred_hash_algorithms(smallvec![HashAlgorithm::SHA2_256,])
        .preferred_compression_algorithms(smallvec![CompressionAlgorithm::ZLIB,]);
    let secret_key_params = key_params
        .build()
        .expect("Must be able to create secret key params");
    let secret_key = secret_key_params
        .generate()
        .expect("Failed to generate a plain key.");
    let passwd_fn = || String::new();
    let secret_key = secret_key
        .sign(passwd_fn)
        .expect("Must be able to sign its own metadata");
    let public_key = secret_key
        .public_key()
        .sign(&secret_key, passwd_fn)
        .expect("Failed to sign public key.");
    (secret_key, public_key)
}

pub fn sign_and_verify() {
    let (secret_key, public_key) = create_key();
    println!(
        "secret_key_id: {}",
        hex::encode(secret_key.key_id().to_vec())
    );
    println!(
        "public_key:\n{}",
        public_key
            .to_armored_string(None)
            .expect("Failed to turn public key into string.")
    );

    let passwd_fn = || String::new();

    let msg_signed = OpenPGPMessage::Literal(LiteralData::from_str("test", "raw string data"))
        .sign(&secret_key, passwd_fn, HashAlgorithm::SHA2_256)
        .expect("Failed to sign message.");
    let msg_signature_str = msg_signed
        .to_armored_string(None)
        .expect("Failed to turn signature into string.");
    println!("signature_armored:\n{}", msg_signature_str);

    msg_signed
        .verify(&public_key)
        .map(|_| println!("Verification successful."))
        .expect("Failed to verify the message.");
}
