use sequoia_openpgp as openpgp;
use openpgp::types::Curve;
use openpgp::cert::prelude::*;
use openpgp::packet::prelude::*;
use openpgp::armor::*;

pub const SECRET_KEY_PATH: &str = "data/secret_key_sequoia";
pub const PUBLIC_KEY_PATH: &str = "data/secret_key_sequoia.pub";

fn test() {
    let key = Key4::generate_ecc(true, Curve::Ed25519).unwrap();
    let key_pair = key.into_keypair().unwrap();
    dbg!(key_pair.public().fingerprint());
    dbg!(key_pair.secret().fingerprint());
    let secret_key_writer = Writer::new(std::fs::File::create(SECRET_KEY_PATH), Kind::SecretKey).unwrap();
    secret_key_writer.write(key_pair.secret());
    secret_key_writer.finalize();
    let public_key_writer = Writer::new(std::fs::File::create(PUBLIC_KEY_PATH), Kind::PublicKey).unwrap();
    public_key_writer.write(key_pair.public());
    public_key_writer.finalize();
}
