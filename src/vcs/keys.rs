use crate::vcs::{db::fetch, locale, ok, ok_audit, tt};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sqlx::Row;
use std::{
    env::current_dir,
    fs::{File, create_dir_all},
    io::{Read, Write},
    path::Path,
};
use unic_langid::LanguageIdentifier;

pub const AWQ_KEYS_DIRNAME: &str = ".awq/keys";
pub fn awq_sign_message(message: &str) -> String {
    let root_path = current_dir().expect("failed to get current dir");
    let path = root_path.join(AWQ_KEYS_DIRNAME);

    if !path.is_dir() {
        panic!("use awq keygen first");
    }

    let secret = path.join("secret.key");
    let mut file = File::open(secret).expect("failed to get secret key");
    let mut bytes = [0u8; 32];
    file.read_exact(&mut bytes).expect("failed to read key");

    let signing_key = SigningKey::from_bytes(&bytes);

    let signature: Signature = signing_key.sign(message.as_bytes());

    hex::encode(signature.to_bytes())
}

pub fn awq_generate_keypair(lang: &LanguageIdentifier) -> bool {
    if Path::new(AWQ_KEYS_DIRNAME).exists() {
        return false;
    }
    let root_path = current_dir().expect("");
    let identity_dir = root_path.join(AWQ_KEYS_DIRNAME);
    create_dir_all(&identity_dir).expect("failed to create identity");

    let secret_path = identity_dir.join("secret.key");
    let public_path = identity_dir.join("public.key");
    if secret_path.exists() {
        return false;
    }
    let mut rng = OsRng;
    // Génération cryptographique
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key: VerifyingKey = signing_key.verifying_key();

    let mut file = File::create(secret_path).expect("failed to create secret key");
    file.write_all(&signing_key.to_bytes()).expect("");
    file.sync_all().expect("");

    let mut file_pub = File::create(public_path).expect("failed to create public key");
    file_pub.write_all(verifying_key.as_bytes()).expect("");
    file_pub.sync_all().expect("");
    ok(tt(lang, "keys-generated").as_str());
    true
}

pub fn awq_verify_signature(message: &str, signature_hex: &str) -> bool {
    let root_path = current_dir().expect("failed to get current dir");
    let identity_dir = root_path.join(AWQ_KEYS_DIRNAME);
    let public_path = identity_dir.join("public.key");
    if !public_path.exists() {
        return false;
    }
    let mut file = File::open(public_path).expect("no public key");
    let mut bytes = [0u8; 32];
    file.read_exact(&mut bytes).expect("failed to read the key");

    let verifying_key = VerifyingKey::from_bytes(&bytes).expect("bad keys");

    // 2. Decode signature (Hex -> Bytes)
    let signature_bytes = hex::decode(signature_hex).expect("invalid hexadecimal format");

    let signature = Signature::from_slice(&signature_bytes).expect("bad signature");
    verifying_key
        .verify_strict(message.as_bytes(), &signature)
        .is_ok()
}

pub async fn awq_audit() -> bool {
    let mut errors = 0;
    let lang = locale();
    for row in &fetch("SELECT message, signature FROM commits ORDER BY id ASC").await {
        let msg = row.get(0);
        let sign = row.get(1);
        if awq_verify_signature(msg, sign).eq(&false) {
            errors += 1;
        } else {
            ok_audit(&lang, sign);
        }
    }
    errors > 0
}
