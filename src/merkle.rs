//! Functionality for chunking file data and calculating and verifying root ids.

use crate::{error::Error, transaction::DeepHashItem};
use borsh::BorshDeserialize;
use ring::digest::{Context, SHA256, SHA384};
use sha2::{digest::DynDigest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn perf_to_system(amt: f64) -> SystemTime {
    let secs = (amt as u64) / 1_000;
    let nanos = (((amt as u64) % 1_000) as u32) * 1_000_000;
    UNIX_EPOCH + Duration::new(secs, nanos)
}

/// Single struct used for original data chunks (Leaves) and branch nodes (hashes of pairs of child nodes).
#[derive(Debug, PartialEq, Clone)]
pub struct Node {
    pub id: [u8; HASH_SIZE],
    pub data_hash: Option<[u8; HASH_SIZE]>,
    pub min_byte_range: usize,
    pub max_byte_range: usize,
    pub left_child: Option<Box<Node>>,
    pub right_child: Option<Box<Node>>,
}

/// Concatenated ids and offsets for full set of nodes for an original data chunk, starting with the root.
#[derive(Debug, PartialEq, Clone)]
pub struct Proof {
    pub offset: usize,
    pub proof: Vec<u8>,
}

/// Populated with data from deserialized [`Proof`] for original data chunk (Leaf [`Node`]).
#[repr(C)]
#[derive(BorshDeserialize, Debug, PartialEq, Clone)]
pub struct LeafProof {
    data_hash: [u8; HASH_SIZE],
    notepad: [u8; NOTE_SIZE - 8],
    offset: [u8; 8],
}

/// Populated with data from deserialized [`Proof`] for branch [`Node`] (hash of pair of child nodes).
#[derive(BorshDeserialize, Debug, PartialEq, Clone)]
pub struct BranchProof {
    left_id: [u8; HASH_SIZE],
    right_id: [u8; HASH_SIZE],
    notepad: [u8; NOTE_SIZE - 8],
    offset: [u8; 8],
}

/// Includes methods to deserialize [`Proof`]s.
pub trait ProofDeserialize<T> {
    fn try_from_proof_slice(slice: &[u8]) -> Result<T, Error>;
    fn offset(&self) -> u32;
}

impl ProofDeserialize<LeafProof> for LeafProof {
    fn try_from_proof_slice(slice: &[u8]) -> Result<Self, Error> {
        let proof = LeafProof::try_from_slice(slice).map_err(|_| Error::InvalidProof)?;
        Ok(proof)
    }
    fn offset(&self) -> u32 {
        let mut bytes: [u8; 4] = Default::default();
        bytes.copy_from_slice(self.offset.split_at(4).1);
        u32::from_be_bytes(bytes)
    }
}

impl ProofDeserialize<BranchProof> for BranchProof {
    fn try_from_proof_slice(slice: &[u8]) -> Result<Self, Error> {
        let proof = BranchProof::try_from_slice(slice).map_err(|_| Error::InvalidProof)?;
        Ok(proof)
    }
    fn offset(&self) -> u32 {
        let mut bytes: [u8; 4] = Default::default();
        bytes.copy_from_slice(self.offset.split_at(4).1);
        u32::from_be_bytes(bytes)
    }
}

pub const MAX_CHUNK_SIZE: usize = 256 * 1024;
pub const MIN_CHUNK_SIZE: usize = 32 * 1024;
pub const HASH_SIZE: usize = 32;
const NOTE_SIZE: usize = 32;

/// Includes a function to convert a number to a Vec of 32 bytes per the Arweave spec.
pub trait Helpers<T> {
    fn to_note_vec(&self) -> Vec<u8>;
}

impl Helpers<u32> for u32 {
    fn to_note_vec(&self) -> Vec<u8> {
        let mut note = vec![0; NOTE_SIZE - 4];
        note.extend((*self as u32).to_be_bytes());
        note
    }
}

/// Generates data chunks from which the calculation of root id starts.
pub fn generate_leaves(data: Vec<u8>) -> Result<Vec<Node>, Error> {
    let mut data_chunks: Vec<&[u8]> = data.chunks(MAX_CHUNK_SIZE).collect();

    #[allow(unused_assignments)]
    let mut last_two = Vec::new();

    if data_chunks.len() > 1 && data_chunks.last().unwrap().len() < MIN_CHUNK_SIZE {
        last_two = data_chunks.split_off(data_chunks.len() - 2).concat();
        let chunk_size = last_two.len() / 2 + (last_two.len() % 2 != 0) as usize;
        data_chunks.append(&mut last_two.chunks(chunk_size).collect::<Vec<&[u8]>>());
    }

    if data_chunks.last().unwrap().len() == MAX_CHUNK_SIZE {
        data_chunks.push(&[]);
    }

    let (chunk_ranges, _) =
        data_chunks
            .iter()
            .fold((Vec::new(), 0), |(mut ranges, min_byte_range), chunk| {
                let max_byte_range = min_byte_range + &chunk.len();
                ranges.push((min_byte_range, max_byte_range));
                (ranges, max_byte_range)
            });

    let mut context = Sha256::default();
    let leaves = data_chunks
        .iter()
        .zip(chunk_ranges)
        .map(|(chunk, (min_byte_range, max_byte_range))| {
            let data_hash = hash_sha256(chunk, &mut context).unwrap();
            let offset = (max_byte_range as u32).to_note_vec();
            let id = hash_all_sha256(vec![&data_hash, &offset], &mut context).unwrap();

            Node {
                id,
                data_hash: Some(data_hash),
                min_byte_range,
                max_byte_range,
                left_child: None,
                right_child: None,
            }
        })
        .collect();
    Ok(leaves)
}

/// Hashes together a single branch node from a pair of child nodes.
pub fn hash_branch(left: Node, right: Node, context: &mut dyn DynDigest) -> Result<Node, Error> {
    let max_byte_range = (left.max_byte_range as u32).to_note_vec();
    let id = hash_all_sha256(vec![&left.id, &right.id, &max_byte_range], context)?;
    Ok(Node {
        id,
        data_hash: None,
        min_byte_range: left.max_byte_range,
        max_byte_range: right.max_byte_range,
        left_child: Some(Box::new(left)),
        right_child: Some(Box::new(right)),
    })
}

/// Builds one layer of branch nodes from a layer of child nodes.
pub fn build_layer<'a>(nodes: Vec<Node>, context: &mut dyn DynDigest) -> Result<Vec<Node>, Error> {
    let mut layer = Vec::<Node>::with_capacity(nodes.len() / 2 + (nodes.len() % 2 != 0) as usize);
    let mut nodes_iter = nodes.into_iter();
    while let Some(left) = nodes_iter.next() {
        if let Some(right) = nodes_iter.next() {
            layer.push(hash_branch(left, right, context).unwrap());
        } else {
            layer.push(left);
        }
    }
    Ok(layer)
}

/// Builds all layers from leaves up to single root node.
pub fn generate_data_root(mut nodes: Vec<Node>) -> Result<Node, Error> {
    let mut context = Sha256::default();
    while nodes.len() > 1 {
        nodes = build_layer(nodes, &mut context)?;
    }
    let root = nodes.pop().unwrap();
    Ok(root)
}

/// Calculates [`Proof`] for each data chunk contained in root [`Node`].
pub fn resolve_proofs(node: Node, proof: Option<Proof>) -> Result<Vec<Proof>, Error> {
    let mut proof = if let Some(proof) = proof {
        proof
    } else {
        Proof {
            offset: 0,
            proof: Vec::new(),
        }
    };
    match node {
        // Leaf
        Node {
            data_hash: Some(data_hash),
            max_byte_range,
            left_child: None,
            right_child: None,
            ..
        } => {
            proof.offset = max_byte_range - 1;
            proof.proof.extend(data_hash);
            proof.proof.extend((max_byte_range as u32).to_note_vec());
            return Ok(vec![proof]);
        }
        // Branch
        Node {
            data_hash: None,
            min_byte_range,
            left_child: Some(left_child),
            right_child: Some(right_child),
            ..
        } => {
            proof.proof.extend(left_child.id.clone());
            proof.proof.extend(right_child.id.clone());
            proof.proof.extend((min_byte_range as u32).to_note_vec());

            let mut left_proof = resolve_proofs(*left_child, Some(proof.clone()))?;
            let right_proof = resolve_proofs(*right_child, Some(proof))?;
            left_proof.extend(right_proof);
            return Ok(left_proof);
        }
        _ => unreachable!(),
    }
}

/// Validates chunk of data against provided [`Proof`].
pub fn validate_chunk(
    mut root_id: [u8; HASH_SIZE],
    chunk: Node,
    proof: Proof,
    context: &mut dyn DynDigest,
) -> Result<(), Error> {
    match chunk {
        Node {
            data_hash: Some(data_hash),
            max_byte_range,
            ..
        } => {
            // Split proof into branches and leaf. Leaf is at the end and branches are ordered
            // from root to leaf.
            let (branches, leaf) = proof
                .proof
                .split_at(proof.proof.len() - HASH_SIZE - NOTE_SIZE);

            // Deserialize proof.
            let branch_proofs: Vec<BranchProof> = branches
                .chunks(HASH_SIZE * 2 + NOTE_SIZE)
                .map(|b| BranchProof::try_from_proof_slice(b).unwrap())
                .collect();
            let leaf_proof = LeafProof::try_from_proof_slice(leaf)?;

            // Validate branches.
            for branch_proof in branch_proofs.iter() {
                // Calculate the id from the proof.
                let id = hash_all_sha256(
                    vec![
                        &branch_proof.left_id,
                        &branch_proof.right_id,
                        &branch_proof.offset().to_note_vec(),
                    ],
                    context,
                )?;

                // Ensure calculated id correct.
                if !(id == root_id) {
                    return Err(Error::InvalidProof.into());
                }

                // If the offset from the proof is greater than the offset in the data chunk,
                // then the next id to validate against is from the left.
                root_id = match max_byte_range > branch_proof.offset() as usize {
                    true => branch_proof.right_id,
                    false => branch_proof.left_id,
                }
            }

            // Validate leaf: both id and data_hash are correct.
            let id = hash_all_sha256(
                vec![&data_hash, &(max_byte_range as u32).to_note_vec()],
                context,
            )?;
            if !(id == root_id) & !(data_hash == leaf_proof.data_hash) {
                return Err(Error::InvalidProof.into());
            }
        }
        _ => {
            unreachable!()
        }
    }
    Ok(())
}

pub fn hash_sha256_old(message: &[u8]) -> Result<[u8; 32], Error> {
    let mut context = Context::new(&SHA256);
    context.update(message);
    let mut result: [u8; 32] = [0; 32];
    result.copy_from_slice(context.finish().as_ref());
    Ok(result)
}

pub fn hash_sha256(message: &[u8], context: &mut dyn DynDigest) -> Result<[u8; 32], Error> {
    context.update(message);
    let mut result = [0u8; 32];
    let hash = context.finalize_reset();
    result.copy_from_slice(&hash);
    Ok(result)
}

fn hash_sha384(message: &[u8]) -> Result<[u8; 48], Error> {
    let mut context = Context::new(&SHA384);
    context.update(message);
    let mut result: [u8; 48] = [0; 48];
    result.copy_from_slice(context.finish().as_ref());
    Ok(result)
}

/// Returns a SHA256 hash of the the concatenated SHA256 hashes of a vector of messages.
pub fn hash_all_sha256(
    messages: Vec<&[u8]>,
    context: &mut dyn DynDigest,
) -> Result<[u8; 32], Error> {
    let hash: Vec<u8> = messages
        .into_iter()
        .map(|m| hash_sha256(m, context).unwrap())
        .into_iter()
        .flatten()
        .collect();
    let hash = hash_sha256(&hash, context)?;
    Ok(hash)
}

/// Returns a SHA384 hash of the the concatenated SHA384 hashes of a vector messages.
fn hash_all_sha384(messages: Vec<&[u8]>) -> Result<[u8; 48], Error> {
    let hash: Vec<u8> = messages
        .into_iter()
        .map(|m| hash_sha384(m).unwrap())
        .into_iter()
        .flatten()
        .collect();
    let hash = hash_sha384(&hash)?;
    Ok(hash)
}

/// Concatenates two `[u8; 48]` arrays, returning a `[u8; 96]` array.
fn concat_u8_48(left: [u8; 48], right: [u8; 48]) -> Result<[u8; 96], Error> {
    let mut iter = left.into_iter().chain(right);
    let result = [(); 96].map(|_| iter.next().unwrap());
    Ok(result)
}

/// Calculates data root of transaction in accordance with implementation in [arweave-js](https://github.com/ArweaveTeam/arweave-js/blob/master/src/common/lib/deepHash.ts).
/// [`DeepHashItem`] is a recursive Enum that allows the function to be applied to
/// nested [`Vec<u8>`] of arbitrary depth.
pub fn deep_hash(deep_hash_item: DeepHashItem) -> Result<[u8; 48], Error> {
    let hash = match deep_hash_item {
        DeepHashItem::Blob(blob) => {
            let blob_tag = format!("blob{}", blob.len());
            hash_all_sha384(vec![blob_tag.as_bytes(), &blob])?
        }
        DeepHashItem::List(list) => {
            let list_tag = format!("list{}", list.len());
            let mut hash = hash_sha384(list_tag.as_bytes())?;

            for child in list.into_iter() {
                let child_hash = deep_hash(child)?;
                hash = hash_sha384(&concat_u8_48(hash, child_hash)?)?;
            }
            hash
        }
    };
    Ok(hash)
}
