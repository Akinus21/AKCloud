use anyhow::Result;
use ed25519_dalek::{Signature, Signer, Verifier, SigningKey, VerifyingKey, KEYPAIR_LENGTH, SIGNATURE_LENGTH};
use rand::{rngs::OsRng, RngCore};
use std::path::PathBuf;

pub struct Identity {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl Identity {
    pub fn load_or_generate(config_dir: &PathBuf) -> Result<Self> {
        let key_path = config_dir.join("identity.key");

        if key_path.exists() {
            let data = std::fs::read(&key_path)?;
            if data.len() != KEYPAIR_LENGTH {
                anyhow::bail!("Invalid identity key file");
            }

            let signing_key = SigningKey::from_bytes(
                data[..32].try_into()?
            );
            let verifying_key = signing_key.verifying_key();

            Ok(Self { signing_key, verifying_key })
        } else {
            let mut bytes = [0u8; KEYPAIR_LENGTH];
            OsRng.fill_bytes(&mut bytes);
            
            let signing_key = SigningKey::from_bytes(
                bytes[..32].try_into()?
            );
            let verifying_key = signing_key.verifying_key();

            std::fs::create_dir_all(config_dir)?;
            std::fs::write(&key_path, &bytes)?;

            Ok(Self { signing_key, verifying_key })
        }
    }

    pub fn node_id(&self) -> String {
        hex::encode(self.verifying_key.to_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LENGTH] {
        self.signing_key.sign(message).to_bytes()
    }

    pub fn verify(&self, message: &[u8], signature: &[u8; SIGNATURE_LENGTH]) -> bool {
        let sig = Signature::from_bytes(signature);
        self.verifying_key.verify(message, &sig).is_ok()
    }
}