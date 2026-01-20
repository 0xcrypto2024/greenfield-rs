use reqwest::Client;
use serde::Deserialize;

/// Status of a Storage Provider
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpStatus {
    InService,
    InJailed,
    GracefulExiting,
    InMaintenance,
    Unknown(i32),
}

impl From<i32> for SpStatus {
    fn from(v: i32) -> Self {
        match v {
            0 => SpStatus::InService,
            1 => SpStatus::InJailed,
            2 => SpStatus::GracefulExiting,
            3 => SpStatus::InMaintenance,
            _ => SpStatus::Unknown(v),
        }
    }
}

/// Description of a Storage Provider
#[derive(Debug, Clone)]
pub struct SpDescription {
    pub moniker: String,
    pub identity: String,
    pub website: String,
    pub security_contact: String,
    pub details: String,
}

/// Storage Provider information
#[derive(Debug, Clone)]
pub struct StorageProvider {
    pub id: u32,
    pub operator_address: String,
    pub funding_address: String,
    pub seal_address: String,
    pub approval_address: String,
    pub gc_address: String,
    pub maintenance_address: String,
    pub total_deposit: String,
    pub status: SpStatus,
    pub endpoint: String,
    pub description: Option<SpDescription>,
    pub bls_key: String,
}

// REST API response structures
#[derive(Deserialize, Debug)]
struct SpQueryResponse {
    sps: Option<Vec<SpJson>>,
}

#[derive(Deserialize, Debug)]
struct SpJson {
    id: Option<u32>,
    operator_address: Option<String>,
    funding_address: Option<String>,
    seal_address: Option<String>,
    approval_address: Option<String>,
    gc_address: Option<String>,
    maintenance_address: Option<String>,
    total_deposit: Option<String>,
    status: Option<i32>,
    endpoint: Option<String>,
    description: Option<SpDescriptionJson>,
    bls_key: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SpDescriptionJson {
    moniker: Option<String>,
    identity: Option<String>,
    website: Option<String>,
    security_contact: Option<String>,
    details: Option<String>,
}

// ABCI query response structure
#[derive(Deserialize, Debug)]
struct AbciQueryResponse {
    result: AbciQueryResult,
}

#[derive(Deserialize, Debug)]
struct AbciQueryResult {
    response: AbciResponse,
}

#[derive(Deserialize, Debug)]
struct AbciResponse {
    code: i32,
    value: Option<String>,
}

/// List all storage providers from the Greenfield network
/// 
/// This function queries the chain via ABCI query and decodes the protobuf response.
pub async fn list_storage_providers(rpc_url: &str) -> Result<Vec<StorageProvider>, Box<dyn std::error::Error>> {
    let client = Client::new();
    
    // Try REST API first
    let rest_url = format!("{}/greenfield/sp/storage_providers", rpc_url);
    let resp = client.get(&rest_url).send().await;
    
    if let Ok(resp) = resp {
        if resp.status().is_success() {
            if let Ok(data) = resp.json::<SpQueryResponse>().await {
                if let Some(sps) = data.sps {
                    return Ok(sps.into_iter().map(|sp| StorageProvider {
                        id: sp.id.unwrap_or(0),
                        operator_address: sp.operator_address.unwrap_or_default(),
                        funding_address: sp.funding_address.unwrap_or_default(),
                        seal_address: sp.seal_address.unwrap_or_default(),
                        approval_address: sp.approval_address.unwrap_or_default(),
                        gc_address: sp.gc_address.unwrap_or_default(),
                        maintenance_address: sp.maintenance_address.unwrap_or_default(),
                        total_deposit: sp.total_deposit.unwrap_or_default(),
                        status: SpStatus::from(sp.status.unwrap_or(0)),
                        endpoint: sp.endpoint.unwrap_or_default(),
                        description: sp.description.map(|d| SpDescription {
                            moniker: d.moniker.unwrap_or_default(),
                            identity: d.identity.unwrap_or_default(),
                            website: d.website.unwrap_or_default(),
                            security_contact: d.security_contact.unwrap_or_default(),
                            details: d.details.unwrap_or_default(),
                        }),
                        bls_key: sp.bls_key.unwrap_or_default(),
                    }).collect());
                }
            }
        }
    }
    
    // Fall back to ABCI query
    let abci_url = format!(
        "{}/abci_query?path=\"/greenfield.sp.Query/StorageProviders\"",
        rpc_url
    );
    
    let resp = client.get(&abci_url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("ABCI query failed: {}", resp.status()).into());
    }
    
    let abci_resp: AbciQueryResponse = resp.json().await?;
    
    if abci_resp.result.response.code != 0 {
        return Err(format!("ABCI query returned error code: {}", abci_resp.result.response.code).into());
    }
    
    let value = abci_resp.result.response.value
        .ok_or("No value in ABCI response")?;
    
    // Decode base64 value
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD.decode(&value)?;
    
    // Parse protobuf response
    // The response is QueryStorageProvidersResponse which contains repeated StorageProvider
    parse_storage_providers_proto(&decoded)
}

/// Parse protobuf-encoded StorageProvider list
fn parse_storage_providers_proto(data: &[u8]) -> Result<Vec<StorageProvider>, Box<dyn std::error::Error>> {
    use prost::Message;
    
    // Define minimal proto structures for parsing
    #[derive(prost::Message)]
    struct QueryStorageProvidersResponse {
        #[prost(message, repeated, tag = "1")]
        sps: Vec<StorageProviderProto>,
    }
    
    #[derive(prost::Message, Clone)]
    struct StorageProviderProto {
        #[prost(uint32, tag = "1")]
        id: u32,
        #[prost(string, tag = "2")]
        operator_address: String,
        #[prost(string, tag = "3")]
        funding_address: String,
        #[prost(string, tag = "4")]
        seal_address: String,
        #[prost(string, tag = "5")]
        approval_address: String,
        #[prost(string, tag = "6")]
        gc_address: String,
        #[prost(string, tag = "7")]
        maintenance_address: String,
        #[prost(string, tag = "8")]
        total_deposit: String,
        #[prost(int32, tag = "9")]
        status: i32,
        #[prost(string, tag = "10")]
        endpoint: String,
        #[prost(message, optional, tag = "11")]
        description: Option<DescriptionProto>,
        #[prost(bytes = "vec", tag = "12")]
        bls_key: Vec<u8>,
    }
    
    #[derive(prost::Message, Clone)]
    struct DescriptionProto {
        #[prost(string, tag = "1")]
        moniker: String,
        #[prost(string, tag = "2")]
        identity: String,
        #[prost(string, tag = "3")]
        website: String,
        #[prost(string, tag = "4")]
        security_contact: String,
        #[prost(string, tag = "5")]
        details: String,
    }
    
    let response = QueryStorageProvidersResponse::decode(data)?;
    
    Ok(response.sps.into_iter().map(|sp| StorageProvider {
        id: sp.id,
        operator_address: sp.operator_address,
        funding_address: sp.funding_address,
        seal_address: sp.seal_address,
        approval_address: sp.approval_address,
        gc_address: sp.gc_address,
        maintenance_address: sp.maintenance_address,
        total_deposit: sp.total_deposit,
        status: SpStatus::from(sp.status),
        endpoint: sp.endpoint,
        description: sp.description.map(|d| SpDescription {
            moniker: d.moniker,
            identity: d.identity,
            website: d.website,
            security_contact: d.security_contact,
            details: d.details,
        }),
        bls_key: hex::encode(&sp.bls_key),
    }).collect())
}

/// VGF Family from chain query
#[derive(Debug, Deserialize)]
struct GvgFamily {
    pub id: u32,
    pub primary_sp_id: u32,
}

#[derive(Debug, Deserialize)]
struct GvgFamiliesResponse {
    pub gvg_families: Vec<GvgFamily>,
}

/// Get a Virtual Group Family ID for a given SP ID from chain
pub async fn get_vgf_id_for_sp(
    rpc_url: &str,
    sp_id: u32,
) -> Result<u32, Box<dyn std::error::Error>> {
    let client = Client::new();
    
    // Query VGF families from chain
    let url = format!("{}/greenfield/virtualgroup/global_virtual_group_families?pagination.limit=100", rpc_url);
    
    println!("   Fetching VGF families from chain...");
    
    let resp = client.get(&url).send().await?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Chain query error ({}): {}", status, text).into());
    }
    
    let data: GvgFamiliesResponse = resp.json().await?;
    
    // Find a VGF family for this SP
    for family in data.gvg_families {
        if family.primary_sp_id == sp_id {
            println!("   Found VGF ID {} for SP ID {}", family.id, sp_id);
            return Ok(family.id);
        }
    }
    
    Err(format!("No VGF family found for SP ID {}", sp_id).into())
}

