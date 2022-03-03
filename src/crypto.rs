use crate::error::Error;
use crate::transaction::{Base64, DeepHashItem};
use jsonwebkey::JsonWebKey;
use ring::{
    digest::{Context, SHA256, SHA384},
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

    pub fn hash_sha256(&self, message: &[u8]) -> Result<[u8; 32], Error> {
        let mut context = Context::new(&SHA256);
        context.update(message);
        let mut result: [u8; 32] = [0; 32];
        result.copy_from_slice(context.finish().as_ref());
        Ok(result)
    }

    fn hash_sha384(&self, message: &[u8]) -> Result<[u8; 48], Error> {
        let mut context = Context::new(&SHA384);
        context.update(message);
        let mut result: [u8; 48] = [0; 48];
        result.copy_from_slice(context.finish().as_ref());
        Ok(result)
    }

    /// Returns a SHA256 hash of the the concatenated SHA256 hashes of a vector of messages.
    pub fn hash_all_sha256(&self, messages: Vec<&[u8]>) -> Result<[u8; 32], Error> {
        let hash: Vec<u8> = messages
            .into_iter()
            .map(|m| self.hash_sha256(m).unwrap())
            .into_iter()
            .flatten()
            .collect();
        let hash = self.hash_sha256(&hash)?;
        Ok(hash)
    }

    /// Returns a SHA384 hash of the the concatenated SHA384 hashes of a vector messages.
    fn hash_all_sha384(&self, messages: Vec<&[u8]>) -> Result<[u8; 48], Error> {
        let hash: Vec<u8> = messages
            .into_iter()
            .map(|m| self.hash_sha384(m).unwrap())
            .into_iter()
            .flatten()
            .collect();
        let hash = self.hash_sha384(&hash)?;
        Ok(hash)
    }

    /// Concatenates two `[u8; 48]` arrays, returning a `[u8; 96]` array.
    fn concat_u8_48(&self, left: [u8; 48], right: [u8; 48]) -> Result<[u8; 96], Error> {
        let mut iter = left.into_iter().chain(right);
        let result = [(); 96].map(|_| iter.next().unwrap());
        Ok(result)
    }

    /// Calculates data root of transaction in accordance with implementation in [arweave-js](https://github.com/ArweaveTeam/arweave-js/blob/master/src/common/lib/deepHash.ts).
    /// [`DeepHashItem`] is a recursive Enum that allows the function to be applied to
    /// nested [`Vec<u8>`] of arbitrary depth.
    pub fn deep_hash(&self, deep_hash_item: DeepHashItem) -> Result<[u8; 48], Error> {
        let hash = match deep_hash_item {
            DeepHashItem::Blob(blob) => {
                let blob_tag = format!("blob{}", blob.len());
                self.hash_all_sha384(vec![blob_tag.as_bytes(), &blob])?
            }
            DeepHashItem::List(list) => {
                let list_tag = format!("list{}", list.len());
                let mut hash = self.hash_sha384(list_tag.as_bytes())?;

                for child in list.into_iter() {
                    let child_hash = self.deep_hash(child)?;
                    hash = self.hash_sha384(&self.concat_u8_48(hash, child_hash)?)?;
                }
                hash
            }
        };
        Ok(hash)
    }

    pub fn fill_rand(&self, dest: &mut [u8]) -> Result<(), Error> {
        let rand_bytes = self.sr.fill(dest).map_err(|_| Error::InvalidHash)?;
        Ok(rand_bytes)
    }
}
