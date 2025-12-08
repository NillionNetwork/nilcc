use crate::config::HeartbeatVerifierConfig;
use anyhow::Context;
use bitcoin::{
    NetworkKind,
    bip32::{ChildNumber, Xpriv},
    key::Secp256k1,
};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::{Arc, Mutex},
};

struct Inner {
    available_keys: BTreeSet<usize>,
}

#[derive(Clone, Copy)]
struct Keypair {
    private: [u8; 32],
    public: [u8; 33],
}

#[derive(Clone)]
pub struct VerifierKeys {
    keys: Arc<Vec<Keypair>>,
    inner: Arc<Mutex<Inner>>,
}

impl VerifierKeys {
    pub fn new(config: &HeartbeatVerifierConfig, key_count: usize) -> anyhow::Result<Self> {
        let engine = Secp256k1::new();
        let network = if config.testnet { NetworkKind::Test } else { NetworkKind::Main };
        let master_key = Xpriv::new_master(network, &config.seed).context("Failed to generate verifier master key")?;
        // Generate N keys and keep their private/public keys
        let mut keys = Vec::new();
        for index in 0..key_count as u32 {
            let key_path = config.base_derivation_path.child(ChildNumber::Hardened { index });
            let key = master_key.derive_priv(&engine, &key_path).context("Failed to derive private key")?;
            let private = key.private_key.secret_bytes();
            let public = key.private_key.public_key(&engine).serialize();
            keys.push(Keypair { private, public });
        }
        let inner = Inner { available_keys: (0..key_count).collect() };
        Ok(Self { keys: keys.into(), inner: Arc::new(Mutex::new(inner)) })
    }

    pub fn next_key(&self) -> Result<VerifierKey, NoMoreKeys> {
        let mut inner = self.inner.lock().expect("lock poisoned");
        let key_index = inner.available_keys.pop_first().ok_or(NoMoreKeys)?;
        let key = self.keys[key_index];
        Ok(VerifierKey { key, key_index, inner: self.inner.clone() })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("no more verifier keys available")]
pub struct NoMoreKeys;

pub struct VerifierKey {
    key: Keypair,
    key_index: usize,
    inner: Arc<Mutex<Inner>>,
}

impl VerifierKey {
    pub fn secret_key(&self) -> [u8; 32] {
        self.key.private
    }

    pub fn public_key(&self) -> [u8; 33] {
        self.key.public
    }
}

impl Drop for VerifierKey {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().expect("lock poisoned");
        inner.available_keys.insert(self.key_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    static CONFIG: LazyLock<HeartbeatVerifierConfig> = LazyLock::new(|| HeartbeatVerifierConfig {
        base_derivation_path: "m/44'/60'".parse().unwrap(),
        seed: [0; 64],
        testnet: false,
    });

    #[test]
    fn generation() {
        let total = 5;
        let keys = VerifierKeys::new(&CONFIG, 5).expect("creating keys");
        let mut generated = Vec::new();
        for _ in 0..total {
            generated.push(keys.next_key().unwrap());
        }
        let generated: Vec<_> = generated.into_iter().map(|k| hex::encode(k.public_key())).collect();
        let expected = &[
            "0328f3e75d8fc83798185eccac691ddf4f8ef84c0b809f234ecbab126399eb9772",
            "0327f8882c340c08887116ce4dae2f671d911c4726df7c8451615b1b9587dde69f",
            "03ea9c27b861af2e7765785f752aecef6df773cc6aafc66f38e3cdfd25282a6ff2",
            "038aada3d4bac0c04a9d222822ba54397d83380be045f60c8b215627032f10d904",
            "03e7e59381a771a0f534ec8af212d41e12321a905f849451575239853bc2e164fc",
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn key_reuse() {
        let keys = VerifierKeys::new(&CONFIG, 1).expect("creating keys");
        let key = keys.next_key().unwrap();
        assert!(keys.next_key().is_err(), "key reused");

        drop(key);
        assert!(keys.next_key().is_ok(), "key not available after drop");
    }
}
