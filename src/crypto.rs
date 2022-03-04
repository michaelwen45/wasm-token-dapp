use crate::error::Error;
use crate::transaction::Base64;
use jsonwebkey::JsonWebKey;
use ring::{
    digest::{Context, SHA256},
    rand::{self, SecureRandom},
    signature::{self, KeyPair, RsaKeyPair},
};

/// Struct for for crypto methods.
pub struct Provider {
    pub keypair: RsaKeyPair,
    pub sr: rand::SystemRandom,
}

impl Provider {
    pub fn from_keypair_string(data: String) -> Result<Provider, Error> {
        let jwk_parsed: JsonWebKey = data.parse().unwrap();
        Ok(Self {
            keypair: signature::RsaKeyPair::from_pkcs8(&jwk_parsed.key.as_ref().to_der())
                .map_err(|_| Error::InvalidHash)?,
            sr: rand::SystemRandom::new(),
        })
    }

    /// Returns the full modulus of the stored keypair. Encoded as a Base64Url String,
    /// represents the associated network address. Also used in the calculation of transaction
    /// signatures.
    pub fn keypair_modulus(&self) -> Result<Base64, Error> {
        let modulus = self
            .keypair
            .public_key()
            .modulus()
            .big_endian_without_leading_zero();
        Ok(Base64(modulus.to_vec()))
    }

    pub fn wallet_address(&self) -> Result<Base64, Error> {
        let mut context = Context::new(&SHA256);
        context.update(&self.keypair_modulus()?.0[..]);
        let wallet_address = Base64(context.finish().as_ref().to_vec());
        Ok(wallet_address)
    }

    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, Error> {
        let rng = rand::SystemRandom::new();
        let mut signature = vec![0; self.keypair.public_modulus_len()];
        self.keypair
            .sign(&signature::RSA_PSS_SHA256, &rng, message, &mut signature)
            .map_err(|_| Error::InvalidHash)?;
        Ok(signature)
    }

    pub fn verify(&self, signature: &[u8], message: &[u8]) -> Result<(), Error> {
        let public_key = signature::UnparsedPublicKey::new(
            &signature::RSA_PSS_2048_8192_SHA256,
            self.keypair.public_key().as_ref(),
        );
        public_key
            .verify(message, signature)
            .map_err(|_| Error::InvalidHash)?;
        Ok(())
    }

    pub fn fill_rand(&self, dest: &mut [u8]) -> Result<(), Error> {
        let rand_bytes = self.sr.fill(dest).map_err(|_| Error::InvalidHash)?;
        Ok(rand_bytes)
    }
}
