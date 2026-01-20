use ethers::types::H256;
use ethers::utils::keccak256;
use serde::{Deserialize, Serialize};

// ============================================================================
// Data Structs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tx {
    pub account_number: String,
    pub chain_id: String,
    pub fee: Fee,
    pub memo: String,
    // Must be msg1, 1-indexed
    pub msg1: TypeCreateObject,
    pub sequence: String,
    pub timeout_height: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fee {
    pub amount: Vec<Coin>,
    #[serde(rename = "gas_limit")]
    pub gas_limit: String,
    pub granter: String,
    pub payer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coin {
    pub denom: String,
    pub amount: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Visibility {
    #[serde(rename = "VISIBILITY_TYPE_PUBLIC_READ")]
    Public,
    #[serde(rename = "VISIBILITY_TYPE_PRIVATE")]
    Private,
    #[serde(rename = "VISIBILITY_TYPE_INHERIT")]
    Inherit,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "VISIBILITY_TYPE_PUBLIC_READ",
            Visibility::Private => "VISIBILITY_TYPE_PRIVATE",
            Visibility::Inherit => "VISIBILITY_TYPE_INHERIT",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RedundancyType {
    #[serde(rename = "REDUNDANCY_EC_TYPE")]
    EcType,
    #[serde(rename = "REDUNDANCY_REPLICA_TYPE")]
    ReplicaType,
}

impl RedundancyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RedundancyType::EcType => "REDUNDANCY_EC_TYPE",
            RedundancyType::ReplicaType => "REDUNDANCY_REPLICA_TYPE",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCreateObject {
    #[serde(rename = "type")]
    pub type_url: String,
    pub bucket_name: String,
    pub content_type: String,
    pub creator: String,
    pub expect_checksums: Vec<String>,
    pub object_name: String,
    pub payload_size: String,
    pub primary_sp_approval: TypeMsg1PrimarySpApproval,
    pub redundancy_type: RedundancyType,
    pub visibility: Visibility,
}

// Alias MsgValue to TypeCreateObject for compatibility with existing code if needed,
// OR just change the code to use TypeCreateObject.
// The user code in client.rs uses MsgValue alias. Let's keep MsgValue as an alias or rename struct.
// User provided: struct TypeCreateObject.
// Existing usage: msg0: MsgValue.
// I should add `pub type MsgValue = TypeCreateObject;` to maintain compatibility with client.rs which might be importing MsgValue.
pub type MsgValue = TypeCreateObject;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMsg1PrimarySpApproval {
    pub expired_height: String,
    pub global_virtual_group_family_id: String, // Must be String for EIP-712 JSON
                                                // No sig field
}

// Alias PrimarySpApproval to TypeMsg1PrimarySpApproval for compatibility
pub type PrimarySpApproval = TypeMsg1PrimarySpApproval;
pub type TypePrimarySpApproval = TypeMsg1PrimarySpApproval;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMsg {
    #[serde(rename = "type")]
    pub type_url: String,
    pub value: serde_json::Value,
}

// ============================================================================
// Hashing Implementation
// ============================================================================

impl Coin {
    pub fn get_type_hash() -> H256 {
        let type_str = "Coin(uint256 amount,string denom)";
        H256::from(keccak256(type_str.as_bytes()))
    }
    pub fn get_struct_hash(&self) -> H256 {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(Self::get_type_hash().as_bytes());
        let amount_num: u128 = self.amount.parse().unwrap_or(0);
        let mut amount_bytes = [0u8; 32];
        let val_bytes = amount_num.to_be_bytes(); // 16 bytes. wait, u128 is 16 bytes.
                                                  // Rust u128 to_be_bytes returns [u8; 16].
                                                  // We need 32 bytes padded.
        amount_bytes[16..32].copy_from_slice(&val_bytes);
        encoded.extend_from_slice(&amount_bytes);
        encoded.extend_from_slice(&keccak256(self.denom.as_bytes()));
        H256::from(keccak256(&encoded))
    }
}

impl Fee {
    pub fn get_type_hash() -> H256 {
        // Must include gas_limit, granter, payer
        let type_str = "Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)Coin(uint256 amount,string denom)";
        H256::from(keccak256(type_str.as_bytes()))
    }
    pub fn get_struct_hash(&self) -> H256 {
        let type_hash = Self::get_type_hash();
        let mut encoded = Vec::new();
        encoded.extend_from_slice(type_hash.as_bytes());
        let mut coins_hashes = Vec::new();
        for coin in &self.amount {
            coins_hashes.extend_from_slice(coin.get_struct_hash().as_bytes());
        }
        let coins_array_hash = keccak256(&coins_hashes);
        encoded.extend_from_slice(&coins_array_hash);
        let gas_num: u64 = self.gas_limit.parse().unwrap_or(0);
        let mut gas_bytes = [0u8; 32];
        gas_bytes[24..32].copy_from_slice(&gas_num.to_be_bytes());
        encoded.extend_from_slice(&gas_bytes);
        let granter_hash = keccak256(self.granter.as_bytes());
        encoded.extend_from_slice(&granter_hash);
        let payer_hash = keccak256(self.payer.as_bytes());
        encoded.extend_from_slice(&payer_hash);

        let final_hash = H256::from(keccak256(&encoded));
        println!(
            "      [DEBUG] Fee TypeHash: 0x{}",
            hex::encode(type_hash.as_bytes())
        );
        println!(
            "      [DEBUG] Fee coins_array_hash: 0x{}",
            hex::encode(coins_array_hash)
        );
        println!(
            "      [DEBUG] Fee gas_limit hash: 0x{}",
            hex::encode(gas_bytes)
        );
        println!(
            "      [DEBUG] Fee granter: '{}' -> 0x{}",
            self.granter,
            hex::encode(granter_hash)
        );
        println!(
            "      [DEBUG] Fee payer: '{}' -> 0x{}",
            self.payer,
            hex::encode(payer_hash)
        );
        println!(
            "      [DEBUG] Fee final_hash: 0x{}",
            hex::encode(final_hash.as_bytes())
        );
        final_hash
    }
}

impl TypeMsg1PrimarySpApproval {
    pub fn get_type_hash() -> H256 {
        let type_str = "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        println!("      [DEBUG] PSA Type String: {}", type_str);
        H256::from(keccak256(type_str.as_bytes()))
    }
    pub fn get_struct_hash(&self) -> H256 {
        let type_hash = Self::get_type_hash();
        let mut encoded = Vec::new();
        encoded.extend_from_slice(type_hash.as_bytes());
        println!("      [DEBUG] PSA TypeHash: 0x{}", hex::encode(type_hash.as_bytes()));
        
        let eh: u64 = self.expired_height.parse().unwrap_or(u64::MAX);
        let mut eh_bytes = [0u8; 32];
        eh_bytes[24..32].copy_from_slice(&eh.to_be_bytes());
        encoded.extend_from_slice(&eh_bytes);
        println!("      [DEBUG] PSA expired_height: {} -> 0x{}", eh, hex::encode(&eh_bytes));

        let mut gvg_bytes = [0u8; 32];
        // u32 is 4 bytes.
        let val: u32 = self.global_virtual_group_family_id.parse().unwrap_or(0);
        let val_bytes = val.to_be_bytes();
        gvg_bytes[28..32].copy_from_slice(&val_bytes);
        encoded.extend_from_slice(&gvg_bytes);
        println!("      [DEBUG] PSA gvg_family_id: {} -> 0x{}", val, hex::encode(&gvg_bytes));
        
        let final_hash = H256::from(keccak256(&encoded));
        println!("      [DEBUG] PSA final_hash: 0x{}", hex::encode(final_hash.as_bytes()));
        final_hash
    }
}

impl TypeCreateObject {
    pub fn get_type_hash() -> H256 {
        // Corrected struct names and types based on official SDK's WalkFields & SanitizeTypedef logic.
        // Msg1's primary_sp_approval field maps to TypeMsg1PrimarySpApproval.
        // IMPORTANT: Go SDK sorts all type fields ALPHABETICALLY (see eip712.go lines 253-257)
        // So field order must be alphabetical: bucket_name, content_type, creator, expect_checksums,
        // object_name, payload_size, primary_sp_approval, redundancy_type, type, visibility
        let msg_val_str = "Msg1(string bucket_name,string content_type,string creator,bytes[] expect_checksums,string object_name,uint64 payload_size,TypeMsg1PrimarySpApproval primary_sp_approval,string redundancy_type,string type,string visibility)";
        let psa_str =
            "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        let full_type_str = format!("{}{}", msg_val_str, psa_str);
        println!("      [DEBUG] Msg1 Full Type String:\n{}", full_type_str);
        H256::from(keccak256(full_type_str.as_bytes()))
    }
    pub fn get_struct_hash(&self) -> H256 {
        let type_hash = Self::get_type_hash();
        let mut encoded = Vec::new();
        encoded.extend_from_slice(type_hash.as_bytes());

        // IMPORTANT: Encoding order MUST be ALPHABETICAL (Go SDK sorts fields)
        // Order: bucket_name, content_type, creator, expect_checksums, object_name,
        // payload_size, primary_sp_approval, redundancy_type, type, visibility

        // 1. bucket_name
        let bucket_hash = keccak256(self.bucket_name.as_bytes());
        encoded.extend_from_slice(&bucket_hash);
        println!(
            "      [DEBUG] Msg1 bucket_name_hash: 0x{}",
            hex::encode(bucket_hash)
        );

        // 2. content_type
        let ct_hash = keccak256(self.content_type.as_bytes());
        encoded.extend_from_slice(&ct_hash);
        println!(
            "      [DEBUG] Msg1 content_type_hash: 0x{}",
            hex::encode(ct_hash)
        );

        // 3. creator
        let creator_hash = keccak256(self.creator.as_bytes());
        encoded.extend_from_slice(&creator_hash);
        println!(
            "      [DEBUG] Msg1 creator_hash: 0x{}",
            hex::encode(creator_hash)
        );

        // 4. expect_checksums
        // CRITICAL: Go SDK uses jsonpb to serialize bytes as base64 strings,
        // then cleanTypesAndMsgValue converts base64 string to []byte(string) (ASCII bytes, NOT decoding base64).
        // So we must hash the base64 string's ASCII bytes, not the original checksum bytes!
        let checksums_hash = if self.expect_checksums.is_empty() {
            keccak256(b"")
        } else {
            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
            let mut inner = Vec::new();
            for cs in &self.expect_checksums {
                // First decode hex to get original bytes
                let original_bytes = if cs.starts_with("0x") || cs.starts_with("0X") {
                    hex::decode(&cs[2..]).unwrap_or_else(|_| cs.as_bytes().to_vec())
                } else {
                    hex::decode(cs).unwrap_or_else(|_| cs.as_bytes().to_vec())
                };
                // Encode to base64 string (like jsonpb does)
                let base64_str = BASE64_STANDARD.encode(&original_bytes);
                // Hash the base64 string's ASCII bytes (like Go SDK's cleanTypesAndMsgValue does)
                let ascii_bytes = base64_str.as_bytes();
                println!("      [DEBUG] Checksum hex: 0x{}", hex::encode(&original_bytes));
                println!("      [DEBUG] Checksum base64: {}", base64_str);
                println!("      [DEBUG] Checksum base64 ASCII hash: 0x{}", hex::encode(keccak256(ascii_bytes)));
                inner.extend_from_slice(&keccak256(ascii_bytes));
            }
            keccak256(&inner)
        };
        encoded.extend_from_slice(&checksums_hash);
        println!(
            "      [DEBUG] Msg1 expect_checksums_hash: 0x{}",
            hex::encode(checksums_hash)
        );

        // 5. object_name
        let obj_hash = keccak256(self.object_name.as_bytes());
        encoded.extend_from_slice(&obj_hash);
        println!(
            "      [DEBUG] Msg1 object_name_hash: 0x{}",
            hex::encode(obj_hash)
        );

        // 6. payload_size
        let mut ps_bytes = [0u8; 32];
        ps_bytes[24..32]
            .copy_from_slice(&self.payload_size.parse::<u64>().unwrap_or(0).to_be_bytes());
        encoded.extend_from_slice(&ps_bytes);
        println!(
            "      [DEBUG] Msg1 payload_size_hash: 0x{}",
            hex::encode(ps_bytes)
        );

        // 7. primary_sp_approval
        let psa_hash = self.primary_sp_approval.get_struct_hash();
        encoded.extend_from_slice(psa_hash.as_bytes());
        println!(
            "      [DEBUG] Msg1 psa_hash: 0x{}",
            hex::encode(psa_hash.as_bytes())
        );

        // 8. redundancy_type
        let rt_hash = keccak256(self.redundancy_type.as_str().as_bytes());
        encoded.extend_from_slice(&rt_hash);
        println!(
            "      [DEBUG] Msg1 redundancy_type_hash: 0x{}",
            hex::encode(rt_hash)
        );

        // 9. type (type_url)
        let type_url_hash = keccak256(self.type_url.as_bytes());
        encoded.extend_from_slice(&type_url_hash);
        println!(
            "      [DEBUG] Msg1 type_url_hash: 0x{}",
            hex::encode(type_url_hash)
        );

        // 10. visibility
        let vis_hash = keccak256(self.visibility.as_str().as_bytes());
        encoded.extend_from_slice(&vis_hash);
        println!(
            "      [DEBUG] Msg1 visibility_hash: 0x{}",
            hex::encode(vis_hash)
        );

        let final_hash = H256::from(keccak256(&encoded));
        println!(
            "      [DEBUG] Msg1 TypeHash: 0x{}",
            hex::encode(type_hash.as_bytes())
        );
        println!(
            "      [DEBUG] Msg1 final_hash: 0x{}",
            hex::encode(final_hash.as_bytes())
        );
        final_hash
    }
}

impl Tx {
    fn get_component_type_strs() -> (String, String, String, String, String, String) {
        // IMPORTANT: Go SDK sorts ALL fields alphabetically (eip712.go lines 260-263)
        // After sorting: account_number, chain_id, fee, memo, msg1, sequence, timeout_height
        // "msg1" < "sequence" in alphabetical order!
        let tx = "Tx(uint256 account_number,uint256 chain_id,Fee fee,string memo,Msg1 msg1,uint256 sequence,uint256 timeout_height)";
        let coin = "Coin(uint256 amount,string denom)";
        let fee = "Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)";
        // IMPORTANT: Go SDK sorts all type fields ALPHABETICALLY (see eip712.go lines 253-257)
        // So field order must be alphabetical
        let msg_val = "Msg1(string bucket_name,string content_type,string creator,bytes[] expect_checksums,string object_name,uint64 payload_size,TypeMsg1PrimarySpApproval primary_sp_approval,string redundancy_type,string type,string visibility)";
        let psa =
            "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        (
            tx.into(),
            coin.into(),
            fee.into(),
            msg_val.into(),
            psa.into(),
            "".into(),
        )
    }
    pub fn get_type_hash() -> H256 {
        let (tx, coin, fee, msg_val, psa, _) = Self::get_component_type_strs();
        // Must concatenate in strict alphabetical order for TypeHash (Coin, Fee, Msg1, TypePrimarySpApproval, Tx)
        // Note: Order must match extractMsgTypes recursive generation in Greenfield source
        let type_str = format!("{}{}{}{}{}", tx, coin, fee, msg_val, psa);
        println!("      [DEBUG] Tx Full Type String:\n{}", type_str);
        H256::from(keccak256(type_str.as_bytes()))
    }
    pub fn get_struct_hash(&self) -> Result<H256, Box<dyn std::error::Error>> {
        let type_hash = Self::get_type_hash();
        let mut encoded = Vec::new();
        encoded.extend_from_slice(type_hash.as_bytes());
        
        let acc_num: u64 = self.account_number.parse()?;
        let mut acc_bytes = [0u8; 32];
        acc_bytes[24..32].copy_from_slice(&acc_num.to_be_bytes());
        encoded.extend_from_slice(&acc_bytes);
        println!("      [DEBUG] Tx account_number: {} -> 0x{}", acc_num, hex::encode(&acc_bytes));

        // Fix: parse chain_id from self.chain_id instead of hardcoded 5600
        let cid_num: u64 = self.chain_id.parse()?;
        let mut cid_bytes = [0u8; 32];
        cid_bytes[24..32].copy_from_slice(&cid_num.to_be_bytes());
        encoded.extend_from_slice(&cid_bytes);
        println!("      [DEBUG] Tx chain_id: {} -> 0x{}", cid_num, hex::encode(&cid_bytes));

        let fee_hash = self.fee.get_struct_hash();
        encoded.extend_from_slice(fee_hash.as_bytes());
        println!("      [DEBUG] Tx fee_hash: 0x{}", hex::encode(fee_hash.as_bytes()));
        
        let memo_hash = keccak256(self.memo.as_bytes());
        encoded.extend_from_slice(&memo_hash);
        println!("      [DEBUG] Tx memo: '{}' -> 0x{}", self.memo, hex::encode(&memo_hash));

        // IMPORTANT: Go SDK sorts ALL fields alphabetically (eip712.go lines 260-263)
        // After sorting: account_number, chain_id, fee, memo, msg1, sequence, timeout_height
        // So msg1 comes BEFORE sequence!
        let msg1_hash = self.msg1.get_struct_hash();
        encoded.extend_from_slice(msg1_hash.as_bytes());
        println!("      [DEBUG] Tx msg1_hash: 0x{}", hex::encode(msg1_hash.as_bytes()));

        let seq: u64 = self.sequence.parse()?;
        let mut seq_bytes = [0u8; 32];
        seq_bytes[24..32].copy_from_slice(&seq.to_be_bytes());
        encoded.extend_from_slice(&seq_bytes);
        println!("      [DEBUG] Tx sequence: {} -> 0x{}", seq, hex::encode(&seq_bytes));

        let timeout: u64 = self.timeout_height.parse().unwrap_or(0);
        let mut timeout_bytes = [0u8; 32];
        timeout_bytes[24..32].copy_from_slice(&timeout.to_be_bytes());
        encoded.extend_from_slice(&timeout_bytes);
        println!("      [DEBUG] Tx timeout_height: {} -> 0x{}", timeout, hex::encode(&timeout_bytes));

        let final_hash = H256::from(keccak256(&encoded));
        println!(
            "      [DEBUG] Tx TypeHash: 0x{}",
            hex::encode(type_hash.as_bytes())
        );
        println!(
            "      [DEBUG] Tx final_hash: 0x{}",
            hex::encode(final_hash.as_bytes())
        );
        Ok(final_hash)
    }
    pub fn get_domain_separator(chain_id_str: &str) -> Result<H256, Box<dyn std::error::Error>> {
        let chain_id_num = if let Some(start) = chain_id_str.find('_') {
            if let Some(end) = chain_id_str.find('-') {
                chain_id_str[start + 1..end].parse::<u64>()?
            } else {
                5600
            }
        } else {
            5600
        };
        // Official Greenfield SDK sorts ALL types A-Z, including EIP712Domain.
        // Sorted: chainId, name, salt, verifyingContract, version
        let type_str = "EIP712Domain(uint256 chainId,string name,string salt,string verifyingContract,string version)";
        let type_hash = keccak256(type_str.as_bytes());
        
        println!("      [DEBUG] Domain Type String: {}", type_str);
        println!("      [DEBUG] Domain TypeHash: 0x{}", hex::encode(&type_hash));
        
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);

        // 1. chainId
        let mut cid_bytes = [0u8; 32];
        cid_bytes[24..32].copy_from_slice(&chain_id_num.to_be_bytes());
        encoded.extend_from_slice(&cid_bytes);
        println!("      [DEBUG] Domain chainId: {} -> 0x{}", chain_id_num, hex::encode(&cid_bytes));

        // 2. name
        let name_hash = keccak256(b"Greenfield Tx");
        encoded.extend_from_slice(&name_hash);
        println!("      [DEBUG] Domain name: 'Greenfield Tx' -> 0x{}", hex::encode(&name_hash));

        // 3. salt
        let salt_hash = keccak256(b"0");
        encoded.extend_from_slice(&salt_hash);
        println!("      [DEBUG] Domain salt: '0' -> 0x{}", hex::encode(&salt_hash));

        // 4. verifyingContract - use "greenfield" (matches Go SDK client behavior)
        Self::append_verifying_contract(&mut encoded, "greenfield");

        // 5. version
        let version_hash = keccak256(b"1.0.0");
        encoded.extend_from_slice(&version_hash);
        println!("      [DEBUG] Domain version: '1.0.0' -> 0x{}", hex::encode(&version_hash));

        let final_domain_hash = H256::from(keccak256(&encoded));
        println!(
            "      [DEBUG] Domain final_hash: 0x{}",
            hex::encode(final_domain_hash.as_bytes())
        );
        Ok(final_domain_hash)
    }
    
    fn append_verifying_contract(encoded: &mut Vec<u8>, verifying_contract: &str) {
        let vc_hash = keccak256(verifying_contract.as_bytes());
        encoded.extend_from_slice(&vc_hash);
        println!("      [DEBUG] Domain verifyingContract: '{}' -> 0x{}", verifying_contract, hex::encode(&vc_hash));
    }
    
    /// Get domain separator with specified verifyingContract
    pub fn get_domain_separator_with_vc(chain_id_str: &str, verifying_contract: &str) -> Result<H256, Box<dyn std::error::Error>> {
        let chain_id_num: u64 = if chain_id_str.contains('_') {
            if let (Some(start), Some(end)) = (
                chain_id_str.find('_'),
                chain_id_str.find('-'),
            ) {
                chain_id_str[start + 1..end].parse::<u64>()?
            } else {
                5600
            }
        } else {
            5600
        };
        let type_str = "EIP712Domain(uint256 chainId,string name,string salt,string verifyingContract,string version)";
        let type_hash = keccak256(type_str.as_bytes());
        
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);

        // 1. chainId
        let mut cid_bytes = [0u8; 32];
        cid_bytes[24..32].copy_from_slice(&chain_id_num.to_be_bytes());
        encoded.extend_from_slice(&cid_bytes);

        // 2. name
        let name_hash = keccak256(b"Greenfield Tx");
        encoded.extend_from_slice(&name_hash);

        // 3. salt
        let salt_hash = keccak256(b"0");
        encoded.extend_from_slice(&salt_hash);

        // 4. verifyingContract
        let vc_hash = keccak256(verifying_contract.as_bytes());
        encoded.extend_from_slice(&vc_hash);

        // 5. version
        let version_hash = keccak256(b"1.0.0");
        encoded.extend_from_slice(&version_hash);

        Ok(H256::from(keccak256(&encoded)))
    }
    
    pub fn get_eip712_hash(&self, chain_id: &str) -> Result<H256, Box<dyn std::error::Error>> {
        let domain_separator = Self::get_domain_separator(chain_id)?;
        let struct_hash = self.get_struct_hash()?;
        let mut digest_input = Vec::new();
        digest_input.push(0x19);
        digest_input.push(0x01);
        digest_input.extend_from_slice(domain_separator.as_bytes());
        digest_input.extend_from_slice(struct_hash.as_bytes());
        println!(
            "   Domain Separator: 0x{}",
            hex::encode(domain_separator.as_bytes())
        );
        println!("   Struct Hash: 0x{}", hex::encode(struct_hash.as_bytes()));
        Ok(H256::from(keccak256(&digest_input)))
    }
}
