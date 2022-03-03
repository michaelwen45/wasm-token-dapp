//! Functionality for chunking file data and calculating and verifying root ids.

use crate::{crypto::Provider, error::Error};
use borsh::BorshDeserialize;

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
pub fn generate_leaves(data: Vec<u8>, crypto: &Provider) -> Result<Vec<Node>, Error> {
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

    let mut leaves = Vec::<Node>::new();
    let mut min_byte_range = 0;
    for chunk in data_chunks.into_iter() {
        let data_hash = crypto.hash_sha256(chunk)?;
        let max_byte_range = min_byte_range + &chunk.len();
        let offset = (max_byte_range as u32).to_note_vec();
        let id = crypto.hash_all_sha256(vec![&data_hash, &offset])?;

        leaves.push(Node {
            id,
            data_hash: Some(data_hash),
            min_byte_range,
            max_byte_range,
            left_child: None,
            right_child: None,
        });
        min_byte_range = min_byte_range + &chunk.len();
    }
    Ok(leaves)
}

/// Hashes together a single branch node from a pair of child nodes.
pub fn hash_branch(left: Node, right: Node, crypto: &Provider) -> Result<Node, Error> {
    let max_byte_range = (left.max_byte_range as u32).to_note_vec();
    let id = crypto.hash_all_sha256(vec![&left.id, &right.id, &max_byte_range])?;
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
pub fn build_layer<'a>(nodes: Vec<Node>, crypto: &Provider) -> Result<Vec<Node>, Error> {
    let mut layer = Vec::<Node>::with_capacity(nodes.len() / 2 + (nodes.len() % 2 != 0) as usize);
    let mut nodes_iter = nodes.into_iter();
    while let Some(left) = nodes_iter.next() {
        if let Some(right) = nodes_iter.next() {
            layer.push(hash_branch(left, right, &crypto).unwrap());
        } else {
            layer.push(left);
        }
    }
    Ok(layer)
}

/// Builds all layers from leaves up to single root node.
pub fn generate_data_root(mut nodes: Vec<Node>, crypto: &Provider) -> Result<Node, Error> {
    while nodes.len() > 1 {
        nodes = build_layer(nodes, &crypto)?;
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
    crypto: &Provider,
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
                let id = crypto.hash_all_sha256(vec![
                    &branch_proof.left_id,
                    &branch_proof.right_id,
                    &branch_proof.offset().to_note_vec(),
                ])?;

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
            let id =
                crypto.hash_all_sha256(vec![&data_hash, &(max_byte_range as u32).to_note_vec()])?;
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
