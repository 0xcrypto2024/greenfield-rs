use anyhow::{anyhow, Result};

/// Extract EIP-155 chain ID from Greenfield chain_id string format.
///
/// Greenfield uses chain_id format like "greenfield_5600-1" where:
/// - "greenfield" is the prefix
/// - "5600" is the EIP-155 chain ID (used for wallet signing)
/// - "1" is the revision number
///
/// # Examples
/// ```rust
/// let chain_id = greenfield_rs::utils::extract_eip155_chain_id("greenfield_5600-1").unwrap();
/// assert_eq!(chain_id, 5600);
/// ```
pub fn extract_eip155_chain_id(chain_id: &str) -> Result<u64> {
    // Split by underscore to get ["greenfield", "5600-1"]
    let parts: Vec<&str> = chain_id.split('_').collect();

    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid chain_id format: expected 'greenfield_<id>-<revision>', got '{}'",
            chain_id
        ));
    }

    if parts[0] != "greenfield" {
        return Err(anyhow!(
            "Invalid chain_id prefix: expected 'greenfield', got '{}'",
            parts[0]
        ));
    }

    // Split the second part by hyphen to get ["5600", "1"]
    let id_part = parts[1]
        .split('-')
        .next()
        .ok_or_else(|| anyhow!("Invalid chain_id format: missing chain ID number"))?;

    // Parse the chain ID number
    id_part
        .parse::<u64>()
        .map_err(|_| anyhow!("Invalid chain_id: '{}' is not a valid number", id_part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_testnet_chain_id() {
        assert_eq!(extract_eip155_chain_id("greenfield_5600-1").unwrap(), 5600);
    }

    #[test]
    fn test_extract_mainnet_chain_id() {
        assert_eq!(extract_eip155_chain_id("greenfield_1017-1").unwrap(), 1017);
    }

    #[test]
    fn test_invalid_format() {
        assert!(extract_eip155_chain_id("invalid").is_err());
        assert!(extract_eip155_chain_id("greenfield").is_err());
        assert!(extract_eip155_chain_id("1017-1").is_err());
    }
}
