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
        encoded.extend_from_slice(&keccak256(self.granter.as_bytes()));
        encoded.extend_from_slice(&keccak256(self.payer.as_bytes()));

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
            "      [DEBUG] Fee final_hash: 0x{}",
            hex::encode(final_hash.as_bytes())
        );
        final_hash
    }
}

impl TypeMsg1PrimarySpApproval {
    pub fn get_type_hash() -> H256 {
        let type_str = "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        H256::from(keccak256(type_str.as_bytes()))
    }
    pub fn get_struct_hash(&self) -> H256 {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(Self::get_type_hash().as_bytes());
        let eh: u64 = self.expired_height.parse().unwrap_or(u64::MAX);
        let mut eh_bytes = [0u8; 32];
        eh_bytes[24..32].copy_from_slice(&eh.to_be_bytes());
        encoded.extend_from_slice(&eh_bytes);

        let mut gvg_bytes = [0u8; 32];
        // u32 is 4 bytes.
        let val: u32 = self.global_virtual_group_family_id.parse().unwrap_or(0);
        let val_bytes = val.to_be_bytes();
        gvg_bytes[28..32].copy_from_slice(&val_bytes);
        encoded.extend_from_slice(&gvg_bytes);
        H256::from(keccak256(&encoded))
    }
}

impl TypeCreateObject {
    pub fn get_type_hash() -> H256 {
        // Corrected struct names and types based on official SDK's WalkFields & SanitizeTypedef logic.
        // Msg1's primary_sp_approval field maps to TypeMsg1PrimarySpApproval.
        // Nested field types match Go's reflect-based types (uint64, uint32).
        let msg_val_str = "Msg1(string bucket_name,string content_type,string creator,string[] expect_checksums,string object_name,uint64 payload_size,TypeMsg1PrimarySpApproval primary_sp_approval,string redundancy_type,string type,string visibility)";
        let psa_str =
            "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)";
        H256::from(keccak256(format!("{}{}", msg_val_str, psa_str).as_bytes()))
    }
    pub fn get_struct_hash(&self) -> H256 {
        let type_hash = Self::get_type_hash();
        let mut encoded = Vec::new();
        encoded.extend_from_slice(type_hash.as_bytes());

        let bucket_hash = keccak256(self.bucket_name.as_bytes());
        encoded.extend_from_slice(&bucket_hash);
        println!(
            "      [DEBUG] Msg1 bucket_name_hash: 0x{}",
            hex::encode(bucket_hash)
        );

        let ct_hash = keccak256(self.content_type.as_bytes());
        encoded.extend_from_slice(&ct_hash);
        println!(
            "      [DEBUG] Msg1 content_type_hash: 0x{}",
            hex::encode(ct_hash)
        );

        let creator_hash = keccak256(self.creator.as_bytes());
        encoded.extend_from_slice(&creator_hash);
        println!(
            "      [DEBUG] Msg1 creator_hash: 0x{}",
            hex::encode(creator_hash)
        );

        let checksums_hash = if self.expect_checksums.is_empty() {
            keccak256(b"")
        } else {
            let mut inner = Vec::new();
            for cs in &self.expect_checksums {
                inner.extend_from_slice(&keccak256(cs.as_bytes()));
            }
            keccak256(&inner)
        };
        encoded.extend_from_slice(&checksums_hash);
        println!(
            "      [DEBUG] Msg1 expect_checksums_hash: 0x{}",
            hex::encode(checksums_hash)
        );

        let obj_hash = keccak256(self.object_name.as_bytes());
        encoded.extend_from_slice(&obj_hash);
        println!(
            "      [DEBUG] Msg1 object_name_hash: 0x{}",
            hex::encode(obj_hash)
        );

        let mut ps_bytes = [0u8; 32];
        ps_bytes[24..32]
            .copy_from_slice(&self.payload_size.parse::<u64>().unwrap_or(0).to_be_bytes());
        encoded.extend_from_slice(&ps_bytes);
        println!(
            "      [DEBUG] Msg1 payload_size_hash: 0x{}",
            hex::encode(ps_bytes)
        );

        let psa_hash = self.primary_sp_approval.get_struct_hash();
        encoded.extend_from_slice(psa_hash.as_bytes());
        println!(
            "      [DEBUG] Msg1 psa_hash: 0x{}",
            hex::encode(psa_hash.as_bytes())
        );

        let rt_hash = keccak256(self.redundancy_type.as_str().as_bytes());
        encoded.extend_from_slice(&rt_hash);
        println!(
            "      [DEBUG] Msg1 redundancy_type_hash: 0x{}",
            hex::encode(rt_hash)
        );

        let type_url_hash = keccak256(self.type_url.as_bytes());
        encoded.extend_from_slice(&type_url_hash);
        println!(
            "      [DEBUG] Msg1 type_url_hash: 0x{}",
            hex::encode(type_url_hash)
        );

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
        let tx = "Tx(uint256 account_number,uint256 chain_id,Fee fee,string memo,Msg1 msg1,uint256 sequence,uint256 timeout_height)";
        let coin = "Coin(uint256 amount,string denom)";
        let fee = "Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)";
        let msg_val = "Msg1(string bucket_name,string content_type,string creator,string[] expect_checksums,string object_name,uint256 payload_size,TypePrimarySpApproval primary_sp_approval,string redundancy_type,string type,string visibility)";
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

        // Fix: parse chain_id from self.chain_id instead of hardcoded 5600
        let cid_num: u64 = self.chain_id.parse()?;
        let mut cid_bytes = [0u8; 32];
        cid_bytes[24..32].copy_from_slice(&cid_num.to_be_bytes());
        encoded.extend_from_slice(&cid_bytes);

        encoded.extend_from_slice(self.fee.get_struct_hash().as_bytes());
        encoded.extend_from_slice(&keccak256(self.memo.as_bytes()));
        encoded.extend_from_slice(self.msg1.get_struct_hash().as_bytes());

        let seq: u64 = self.sequence.parse()?;
        let mut seq_bytes = [0u8; 32];
        seq_bytes[24..32].copy_from_slice(&seq.to_be_bytes());
        encoded.extend_from_slice(&seq_bytes);

        let timeout: u64 = self.timeout_height.parse().unwrap_or(0);
        let mut timeout_bytes = [0u8; 32];
        timeout_bytes[24..32].copy_from_slice(&timeout.to_be_bytes());
        encoded.extend_from_slice(&timeout_bytes);

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
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);

        // 1. chainId
        let mut cid_bytes = [0u8; 32];
        cid_bytes[24..32].copy_from_slice(&chain_id_num.to_be_bytes());
        encoded.extend_from_slice(&cid_bytes);

        // 2. name
        encoded.extend_from_slice(&keccak256(b"Greenfield Tx"));

        // 3. salt
        encoded.extend_from_slice(&keccak256(b"0"));

        // 4. verifyingContract
        // CRITICAL: Official SDK uses the string "greenfield", NOT an address!
        // This is confirmed from greenfield-cosmos-sdk@v1.10.1/x/auth/tx/eip712.go
        encoded.extend_from_slice(&keccak256(b"greenfield"));

        // 5. version
        encoded.extend_from_slice(&keccak256(b"1.0.0"));

        let final_domain_hash = H256::from(keccak256(&encoded));
        println!("      [DEBUG] Domain Type String: {}", type_str);
        println!(
            "      [DEBUG] Domain final_hash: 0x{}",
            hex::encode(final_domain_hash.as_bytes())
        );
        Ok(final_domain_hash)
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
