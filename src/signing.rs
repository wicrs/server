use chrono;
use pgp::composed::{key::SecretKeyParamsBuilder, KeyType, PublicKey, SignedSecretKey};
use pgp::crypto::{hash::HashAlgorithm, sym::SymmetricKeyAlgorithm, PublicKeyAlgorithm};
use pgp::packet::{write_packet, SignatureType, SignatureVersion, Subpacket};
use pgp::types::KeyTrait;
use pgp::types::{CompressionAlgorithm, PublicKeyTrait, SecretKeyTrait, Version};
use pgp::Signature;
use sha2::{Digest, Sha256};
use smallvec::*;
use std::io::Cursor;

const DATA: &str = "haha";

fn create_key() -> (SignedSecretKey, PublicKey) {
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
    let signed_secret_key = secret_key
        .sign(passwd_fn)
        .expect("Must be able to sign its own metadata");
    println!("secret_key_id: {}", hex::encode(signed_secret_key.key_id().to_vec()));
    let public_key = signed_secret_key.public_key();
    (signed_secret_key, public_key)
}

pub fn sign_and_verify() {
    let now = chrono::Utc::now();

    let passwd_fn = || String::new();

    let (secret_key, public_key) = create_key();
    let digest = {
        let mut hasher = Sha256::new();
        hasher.update(DATA);
        hasher.finalize()
    };
    let digest = digest.as_slice();

    // creates the cryptographic core of the signature without any metadata
    let signature = secret_key
        .create_signature(passwd_fn, HashAlgorithm::SHA2_256, digest)
        .expect("Failed to crate signature");

    // the signature can already be verified
    public_key
        .verify_signature(HashAlgorithm::SHA2_256, digest, &signature)
        .expect("Failed to validate signature");

    // wraps the signature in the apropriate package fmt ready to be serialized
    let signature = Signature::new(
        Version::Old,
        SignatureVersion::V4,
        SignatureType::Binary,
        PublicKeyAlgorithm::RSA,
        HashAlgorithm::SHA2_256,
        [digest[0], digest[1]],
        signature.to_vec(),
        vec![
            Subpacket::SignatureCreationTime(now),
            Subpacket::Issuer(secret_key.key_id()),
        ],
        vec![],
    );

    // sign and and write the package (the package written here is NOT rfc4880 compliant)
    let mut signature_bytes = Vec::with_capacity(1024);

    let mut buff = Cursor::new(&mut signature_bytes);
    write_packet(&mut buff, &signature).expect("Write must succeed");

    let raw_signature = signature.signature;
    public_key
        .verify_signature(HashAlgorithm::SHA2_256, digest, &raw_signature)
        .expect("Verify must succeed");
}
