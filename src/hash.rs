//! Hash utilities for Greenfield object checksums
//! 
//! This module provides functions to compute integrity hashes for objects
//! following the Greenfield redundancy scheme.

use sha2::{Sha256, Digest};
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::io::Read;

/// Default redundancy parameters (from Greenfield chain)
pub const DEFAULT_DATA_SHARDS: usize = 4;
pub const DEFAULT_PARITY_SHARDS: usize = 2;
pub const DEFAULT_SEGMENT_SIZE: usize = 16 * 1024 * 1024; // 16MB

/// Generate SHA256 checksum of data
pub fn generate_checksum(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Generate integrity hash from a list of checksums
pub fn generate_integrity_hash(checksums: &[Vec<u8>]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    for checksum in checksums {
        hasher.update(checksum);
    }
    hasher.finalize().to_vec()
}

/// Encode a segment using Reed-Solomon erasure coding
/// Returns (data_shards + parity_shards) pieces
fn encode_segment(
    data: &[u8],
    data_shards: usize,
    parity_shards: usize,
) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    let total_shards = data_shards + parity_shards;
    
    // Calculate shard size (must be evenly divisible)
    let shard_size = (data.len() + data_shards - 1) / data_shards;
    
    // Prepare shards - pad data if necessary
    let mut shards: Vec<Vec<u8>> = Vec::with_capacity(total_shards);
    
    // Create data shards
    for i in 0..data_shards {
        let start = i * shard_size;
        let end = std::cmp::min(start + shard_size, data.len());
        let mut shard = vec![0u8; shard_size];
        if start < data.len() {
            let copy_len = end - start;
            shard[..copy_len].copy_from_slice(&data[start..end]);
        }
        shards.push(shard);
    }
    
    // Create empty parity shards
    for _ in 0..parity_shards {
        shards.push(vec![0u8; shard_size]);
    }
    
    // Create encoder and encode
    let encoder = ReedSolomon::new(data_shards, parity_shards)?;
    encoder.encode(&mut shards)?;
    
    Ok(shards)
}

/// Compute integrity hash roots for an object
/// Returns a list of hashes: [primary_sp_hash, secondary_sp_hash_1, ..., secondary_sp_hash_n]
/// where n = data_shards + parity_shards
pub fn compute_integrity_hash<R: Read>(
    mut reader: R,
    segment_size: usize,
    data_shards: usize,
    parity_shards: usize,
) -> Result<(Vec<Vec<u8>>, u64), Box<dyn std::error::Error>> {
    let total_shards = data_shards + parity_shards;
    
    // Collect checksums for each shard across all segments
    let mut segment_checksums: Vec<Vec<u8>> = Vec::new(); // Primary SP checksums
    let mut shard_checksums: Vec<Vec<Vec<u8>>> = vec![Vec::new(); total_shards]; // Secondary SP checksums
    
    let mut content_len: u64 = 0;
    let mut buffer = vec![0u8; segment_size];
    
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        
        content_len += n as u64;
        let segment = &buffer[..n];
        
        // Compute segment checksum for primary SP
        let seg_checksum = generate_checksum(segment);
        segment_checksums.push(seg_checksum);
        
        // Encode segment and compute checksums for secondary SPs
        let encoded_shards = encode_segment(segment, data_shards, parity_shards)?;
        for (i, shard) in encoded_shards.iter().enumerate() {
            let shard_checksum = generate_checksum(shard);
            shard_checksums[i].push(shard_checksum);
        }
    }
    
    // Build final hash list
    let mut hash_list: Vec<Vec<u8>> = Vec::with_capacity(total_shards + 1);
    
    // Primary SP integrity hash (hash of all segment checksums)
    hash_list.push(generate_integrity_hash(&segment_checksums));
    
    // Secondary SP integrity hashes (hash of all shard checksums for each shard index)
    for checksums in shard_checksums {
        hash_list.push(generate_integrity_hash(&checksums));
    }
    
    Ok((hash_list, content_len))
}

/// Compute integrity hashes from file path
pub fn compute_hash_from_file(
    file_path: &str,
    segment_size: usize,
    data_shards: usize,
    parity_shards: usize,
) -> Result<(Vec<Vec<u8>>, u64), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(file_path)?;
    compute_integrity_hash(file, segment_size, data_shards, parity_shards)
}

/// Compute integrity hashes with default parameters
pub fn compute_hash_from_file_default(
    file_path: &str,
) -> Result<(Vec<Vec<u8>>, u64), Box<dyn std::error::Error>> {
    compute_hash_from_file(
        file_path,
        DEFAULT_SEGMENT_SIZE,
        DEFAULT_DATA_SHARDS,
        DEFAULT_PARITY_SHARDS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_checksum() {
        let data = b"hello world";
        let checksum = generate_checksum(data);
        assert_eq!(checksum.len(), 32);
    }
    
    #[test]
    fn test_compute_integrity_hash() {
        let data = b"0123456789".repeat(100);
        let cursor = std::io::Cursor::new(data);
        
        let (hashes, size) = compute_integrity_hash(
            cursor,
            1024, // small segment for testing
            4,
            2,
        ).unwrap();
        
        // Should have 7 hashes (1 primary + 4 data + 2 parity)
        assert_eq!(hashes.len(), 7);
        assert_eq!(size, 1000);
        
        // Each hash should be 32 bytes (SHA256)
        for hash in &hashes {
            assert_eq!(hash.len(), 32);
        }
    }
}


