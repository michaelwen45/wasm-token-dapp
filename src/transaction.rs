//! Data structures for serializing and deserializing [`Transaction`]s and [`Tag`]s.

use crate::{
    error::Error,
    merkle::{generate_data_root, generate_leaves, resolve_proofs, Node, Proof},
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

/// Transaction data structure per [Arweave transaction spec](https://docs.arweave.org/developers/server/http-api#transaction-format).
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Transaction {
    pub format: u8,
    pub id: Base64,
    pub last_tx: Base64,
    pub owner: Base64,
    pub tags: Vec<Tag<Base64>>,
    pub target: Base64,
    #[serde(with = "stringify")]
    pub quantity: u64,
    pub data_root: Base64,
    pub data: Base64,
    #[serde(with = "stringify")]
    pub data_size: u64,
    #[serde(with = "stringify")]
    pub reward: u64,
    pub signature: Base64,
    #[serde(skip)]
    pub chunks: Vec<Node>,
    #[serde(skip)]
    pub proofs: Vec<Proof>,
}

/// Chunk data structure per [Arweave chunk spec](https://docs.arweave.org/developers/server/http-api#upload-chunks).
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Chunk {
    data_root: Base64,
    #[serde(with = "stringify")]
    data_size: u64,
    data_path: Base64,
    #[serde(with = "stringify")]
    pub offset: usize,
    chunk: Base64,
}

/// Serializes and deserializes numbers represented as Strings. Used for `quantity`, `data_size`
/// and `reward` [`Transaction`] fields so that they can be represented as numbers but be serialized
/// to Strings as required by the Arweave spec.
pub mod stringify {
    use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        String::deserialize(deserializer)?
            .parse::<T>()
            .map_err(|e| D::Error::custom(format!("{}", e)))
    }

    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: std::fmt::Display,
    {
        format!("{}", value).serialize(serializer)
    }
}

impl Transaction {
    pub fn clone_with_no_data(&self) -> Result<Self, Error> {
        Ok(Self {
            format: self.format,
            id: self.id.clone(),
            last_tx: self.last_tx.clone(),
            owner: self.owner.clone(),
            tags: self.tags.clone(),
            target: self.target.clone(),
            quantity: self.quantity,
            data_root: self.data_root.clone(),
            data: Base64::default(),
            data_size: self.data_size,
            reward: self.reward,
            signature: self.signature.clone(),
            chunks: Vec::new(),
            proofs: Vec::new(),
        })
    }
    pub fn get_chunk(&self, idx: usize) -> Result<Chunk, Error> {
        Ok(Chunk {
            data_root: self.data_root.clone(),
            data_size: self.data_size,
            data_path: Base64(self.proofs[idx].proof.clone()),
            offset: self.proofs[idx].offset,
            chunk: Base64(
                self.data.0[self.chunks[idx].min_byte_range..self.chunks[idx].max_byte_range]
                    .to_vec(),
            ),
        })
    }
}

/// Implemented on [`Transaction`] to create root [`DeepHashItem`]s used by
/// [`crate::crypto::Provider::deep_hash`] in the creation of a transaction
/// signatures.
pub trait ToItems<'a, T> {
    fn to_deep_hash_item(&'a self) -> Result<DeepHashItem, Error>;
}

impl<'a> ToItems<'a, Transaction> for Transaction {
    fn to_deep_hash_item(&'a self) -> Result<DeepHashItem, Error> {
        match &self.format {
            1 => {
                let mut children: Vec<DeepHashItem> = vec![
                    &self.owner.0[..],
                    &self.target.0,
                    &self.data.0,
                    self.quantity.to_string().as_bytes(),
                    self.reward.to_string().as_bytes(),
                    &self.last_tx.0,
                ]
                .into_iter()
                .map(DeepHashItem::from_item)
                .collect();
                children.push(self.tags.to_deep_hash_item()?);

                Ok(DeepHashItem::from_children(children))
            }
            2 => {
                let mut children: Vec<DeepHashItem> = vec![
                    self.format.to_string().as_bytes(),
                    &self.owner.0,
                    &self.target.0,
                    self.quantity.to_string().as_bytes(),
                    self.reward.to_string().as_bytes(),
                    &self.last_tx.0,
                ]
                .into_iter()
                .map(DeepHashItem::from_item)
                .collect();
                children.push(self.tags.to_deep_hash_item()?);
                children.push(DeepHashItem::from_item(
                    self.data_size.to_string().as_bytes(),
                ));
                children.push(DeepHashItem::from_item(&self.data_root.0));

                Ok(DeepHashItem::from_children(children))
            }
            _ => unreachable!(),
        }
    }
}

/// Transaction tag.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Tag<T> {
    pub name: T,
    pub value: T,
}

/// Implemented to create [`Tag`]s from utf-8 strings.
pub trait FromUtf8Strs<T> {
    fn from_utf8_strs(name: &str, value: &str) -> Result<T, Error>;
}

impl FromUtf8Strs<Tag<Base64>> for Tag<Base64> {
    fn from_utf8_strs(name: &str, value: &str) -> Result<Self, Error> {
        let b64_name = Base64::from_utf8_str(name)?;
        let b64_value = Base64::from_utf8_str(value)?;

        Ok(Self {
            name: b64_name,
            value: b64_value,
        })
    }
}

impl FromUtf8Strs<Tag<String>> for Tag<String> {
    fn from_utf8_strs(name: &str, value: &str) -> Result<Self, Error> {
        let name = String::from(name);
        let value = String::from(value);

        Ok(Self { name, value })
    }
}

impl<'a> ToItems<'a, Vec<Tag<Base64>>> for Vec<Tag<Base64>> {
    fn to_deep_hash_item(&'a self) -> Result<DeepHashItem, Error> {
        if self.len() > 0 {
            Ok(DeepHashItem::List(
                self.iter()
                    .map(|t| t.to_deep_hash_item().unwrap())
                    .collect(),
            ))
        } else {
            Ok(DeepHashItem::Blob(Vec::<u8>::new()))
        }
    }
}

impl<'a> ToItems<'a, Tag<Base64>> for Tag<Base64> {
    fn to_deep_hash_item(&'a self) -> Result<DeepHashItem, Error> {
        Ok(DeepHashItem::List(vec![
            DeepHashItem::Blob(self.name.0.to_vec()),
            DeepHashItem::Blob(self.value.0.to_vec()),
        ]))
    }
}

/// A struct of [`Vec<u8>`] used for all data and address fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Base64(pub Vec<u8>);

impl Default for Base64 {
    fn default() -> Self {
        Base64(vec![])
    }
}

impl std::fmt::Display for Base64 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let string = &base64::display::Base64Display::with_config(&self.0, base64::URL_SAFE_NO_PAD);
        write!(f, "{}", string)
    }
}

/// Converts a base64url encoded string to a Base64 struct.
impl FromStr for Base64 {
    type Err = base64::DecodeError;
    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let result = base64::decode_config(str, base64::URL_SAFE_NO_PAD)?;
        Ok(Self(result))
    }
}

impl Base64 {
    pub fn from_utf8_str(str: &str) -> Result<Self, Error> {
        Ok(Self(str.as_bytes().to_vec()))
    }
    pub fn to_utf8_string(&self) -> Result<String, Error> {
        Ok(String::from_utf8(self.0.clone()).map_err(|_| Error::InvalidTags)?)
    }
}

impl Serialize for Base64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format!("{}", &self))
    }
}

impl<'de> Deserialize<'de> for Base64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Vis;
        impl serde::de::Visitor<'_> for Vis {
            type Value = Base64;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a base64 string")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                base64::decode_config(v, base64::URL_SAFE_NO_PAD)
                    .map(Base64)
                    .map_err(|_| de::Error::custom("failed to decode base64 string"))
            }
        }
        deserializer.deserialize_str(Vis)
    }
}

/// Recursive data structure that facilitates [`crate::crypto::Provider::deep_hash`] accepting nested
/// arrays of arbitrary depth as an argument with a single type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeepHashItem {
    Blob(Vec<u8>),
    List(Vec<DeepHashItem>),
}

impl DeepHashItem {
    pub fn from_item(item: &[u8]) -> DeepHashItem {
        Self::Blob(item.to_vec())
    }
    pub fn from_children(children: Vec<DeepHashItem>) -> DeepHashItem {
        Self::List(children)
    }
}

pub fn merklize(data: Vec<u8>) -> Result<Transaction, Error> {
    let mut chunks = generate_leaves(data.clone())?;
    let root = generate_data_root(chunks.clone())?;
    let data_root = Base64(root.id.clone().into_iter().collect());
    let mut proofs = resolve_proofs(root, None)?;

    // Discard the last chunk & proof if it's zero length.
    let last_chunk = chunks.last().unwrap();
    if last_chunk.max_byte_range == last_chunk.min_byte_range {
        chunks.pop();
        proofs.pop();
    }

    Ok(Transaction {
        format: 2,
        data_size: data.len() as u64,
        data: Base64(data),
        data_root,
        chunks,
        proofs,
        ..Default::default()
    })
}
