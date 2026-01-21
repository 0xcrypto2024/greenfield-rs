use reqwest::Client;
use serde::Deserialize;

/// Bucket information from chain
#[derive(Debug, Clone)]
pub struct BucketInfo {
    pub id: String,
    pub bucket_name: String,
    pub owner: String,
    pub visibility: i32,
    pub source_type: i32,
    pub create_at: i64,
    pub payment_address: String,
    pub global_virtual_group_family_id: u32,
    pub charged_read_quota: u64,
    pub bucket_status: i32,
    pub sp_as_delegated_agent_disabled: bool,
}

#[derive(Deserialize, Debug)]
struct BucketQueryResponse {
    bucket_info: Option<BucketInfoJson>,
}

#[derive(Deserialize, Debug)]
struct BucketInfoJson {
    id: Option<String>,
    bucket_name: Option<String>,
    owner: Option<String>,
    visibility: Option<String>,
    source_type: Option<String>,
    create_at: Option<String>,
    payment_address: Option<String>,
    global_virtual_group_family_id: Option<u32>,
    charged_read_quota: Option<String>,
    bucket_status: Option<String>,
    sp_as_delegated_agent_disabled: Option<bool>,
}

/// Get bucket information from the chain
pub async fn get_bucket_info(rpc_url: &str, bucket_name: &str) -> Result<BucketInfo, Box<dyn std::error::Error>> {
    let client = Client::new();
    
    // Use REST API to query bucket info
    let url = format!("{}/greenfield/storage/head_bucket/{}", rpc_url, bucket_name);
    
    let resp = client.get(&url).send().await?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Failed to get bucket info: {} - {}", status, body).into());
    }
    
    let data: BucketQueryResponse = resp.json().await?;
    
    let info = data.bucket_info.ok_or("No bucket_info in response")?;
    
    // Parse visibility string to i32
    let visibility = match info.visibility.as_deref() {
        Some("VISIBILITY_TYPE_PUBLIC_READ") => 1,
        Some("VISIBILITY_TYPE_PRIVATE") => 2,
        Some("VISIBILITY_TYPE_INHERIT") => 3,
        _ => 0,
    };
    
    // Parse source_type string to i32
    let source_type = match info.source_type.as_deref() {
        Some("SOURCE_TYPE_ORIGIN") => 0,
        Some("SOURCE_TYPE_BSC_CROSS_CHAIN") => 1,
        Some("SOURCE_TYPE_MIRROR_PENDING") => 2,
        _ => 0,
    };
    
    // Parse bucket_status string to i32
    let bucket_status = match info.bucket_status.as_deref() {
        Some("BUCKET_STATUS_CREATED") => 0,
        Some("BUCKET_STATUS_DISCONTINUED") => 1,
        Some("BUCKET_STATUS_MIGRATING") => 2,
        _ => 0,
    };
    
    Ok(BucketInfo {
        id: info.id.unwrap_or_default(),
        bucket_name: info.bucket_name.unwrap_or_default(),
        owner: info.owner.unwrap_or_default(),
        visibility,
        source_type,
        create_at: info.create_at.and_then(|s| s.parse().ok()).unwrap_or(0),
        payment_address: info.payment_address.unwrap_or_default(),
        global_virtual_group_family_id: info.global_virtual_group_family_id.unwrap_or(0),
        charged_read_quota: info.charged_read_quota.and_then(|s| s.parse().ok()).unwrap_or(0),
        bucket_status,
        sp_as_delegated_agent_disabled: info.sp_as_delegated_agent_disabled.unwrap_or(false),
    })
}

/// Get SP endpoint for a bucket (from primary SP)
pub async fn get_bucket_primary_sp(rpc_url: &str, bucket_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let bucket_info = get_bucket_info(rpc_url, bucket_name).await?;
    let vgf_id = bucket_info.global_virtual_group_family_id;
    
    println!("   Bucket VGF ID: {}", vgf_id);
    
    let client = Client::new();
    
    // Step 1: Get all VGF families and find the one matching our bucket's VGF ID
    let url = format!("{}/greenfield/virtualgroup/global_virtual_group_families?pagination.limit=100", rpc_url);
    let resp = client.get(&url).send().await?;
    
    if !resp.status().is_success() {
        return Err(format!("Failed to get VGF families: {}", resp.status()).into());
    }
    
    #[derive(Deserialize)]
    struct VgfFamiliesResponse {
        gvg_families: Vec<VgfJson>,
    }
    
    #[derive(Deserialize)]
    struct VgfJson {
        id: u32,
        primary_sp_id: u32,
    }
    
    let data: VgfFamiliesResponse = resp.json().await?;
    
    let vgf = data.gvg_families.iter()
        .find(|f| f.id == vgf_id)
        .ok_or_else(|| format!("VGF family {} not found", vgf_id))?;
    
    let sp_id = vgf.primary_sp_id;
    println!("   Primary SP ID: {}", sp_id);
    
    // Step 2: Get all SPs and find the one matching our primary SP ID
    let sps = crate::sp::list_storage_providers(rpc_url).await?;
    
    let sp = sps.iter()
        .find(|s| s.id == sp_id)
        .ok_or_else(|| format!("SP {} not found", sp_id))?;
    
    println!("   SP Endpoint: {}", sp.endpoint);
    
    Ok(sp.endpoint.clone())
}

