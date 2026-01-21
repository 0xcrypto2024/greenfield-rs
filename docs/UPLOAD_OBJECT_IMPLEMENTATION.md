# Greenfield Rust SDK - Upload Object 实现指南

本文档详细记录了如何在 Rust 中实现 Greenfield 的 Upload Object 功能，包括 CreateObject + PutObject 完整流程、SP 认证机制和常见问题排查。

## 目录

1. [背景知识](#背景知识)
2. [问题定位与解决过程](#问题定位与解决过程)
3. [Go SDK 代码追踪方法](#go-sdk-代码追踪方法)
4. [从零开始实现 Upload Object](#从零开始实现-upload-object)
5. [Rust 代码详解](#rust-代码详解)
6. [关键经验总结](#关键经验总结)

---

## 背景知识

### Upload Object 完整流程

Upload Object 是一个**两步操作**：

```
┌─────────────────────────────────────────────────────────────┐
│                    Upload Object 完整流程                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Step 1: CreateObject (链上交易)                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ User ──MsgCreateObject──> Chain                     │    │
│  │ Chain: 创建 object 元数据，状态=OBJECT_STATUS_CREATED │    │
│  │ Chain ──TxHash──> User                              │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                 │
│                           ▼                                 │
│  Step 2: 等待 SP 同步                                        │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ User ──HEAD ?upload-progress──> SP                  │    │
│  │ SP: 检查 object 元数据是否已从链同步                   │    │
│  │ SP ──200 OK / 404──> User                           │    │
│  │ (如果 404，等待重试)                                  │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                 │
│                           ▼                                 │
│  Step 3: PutObject (HTTP 请求到 SP)                          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ User ──PUT /{bucket}/{object}──> SP                 │    │
│  │ SP: 验证签名，检查权限，接收数据                       │    │
│  │ SP ──200 OK / Error──> User                         │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                 │
│                           ▼                                 │
│  Step 4: Object Sealed (SP 异步处理)                         │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ SP: 存储数据，计算证明，提交 seal 交易                 │    │
│  │ Chain: 更新 object 状态=OBJECT_STATUS_SEALED         │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### SP 认证机制：GNFD1-ECDSA

PutObject 使用 **GNFD1-ECDSA** 认证方式，不同于链上交易的 EIP-712：

| 认证类型 | 使用场景 | 签名内容 |
|---------|---------|---------|
| EIP-712 | 链上交易 (CreateBucket, CreateObject) | TypedData 结构化数据 |
| GNFD1-ECDSA | SP HTTP 请求 (PutObject, GetObject) | Canonical Request 字符串 |

### 核心仓库

| 仓库 | 作用 | 关键文件 |
|------|------|----------|
| `greenfield-go-sdk` | Go SDK 客户端 | `client/api_object.go` |
| `greenfield-common` | 公共库，签名逻辑 | `go/http/gen_sign_str.go` |
| `greenfield-storage-provider` | SP 代码，验证逻辑 | `modular/gater/request_context.go` |

---

## 问题定位与解决过程

### 问题 1: `mismatched primary sp` (400)

**错误信息**：
```
❌ Upload failed: PUT failed with status 400 Bad Request: 
<Error><Code>20002</Code><Message>mismatched primary sp</Message></Error>
```

**排查过程**：

1. 用户手动指定了 `--sp-url` 参数
2. 指定的 SP 不是 bucket 的 primary SP

**解决方案**：从 bucket 信息自动获取 primary SP endpoint。

```rust
// 1. 查询 bucket 信息，获取 primary_sp_id
let bucket_info = get_bucket_info(bucket_name).await?;
let primary_sp_id = bucket_info.global_virtual_group_family_id;

// 2. 查询 VGF 信息，获取 primary_sp_id
let vgf_info = get_vgf_info(bucket_info.global_virtual_group_family_id).await?;

// 3. 查询 SP 信息，获取 endpoint
let sp_info = get_sp_info(vgf_info.primary_sp_id).await?;
let sp_endpoint = sp_info.endpoint;
```

---

### 问题 2: `Failed to get virtual group family: 501 Not Implemented`

**错误信息**：
```
❌ Upload failed: Failed to get virtual group family: 501 Not Implemented
```

**排查过程**：

1. 原本使用 `/greenfield/virtualgroup/global_virtual_group_family/{id}` API
2. 该 API 返回 501，未实现

**解决方案**：使用列表 API 并过滤：

```rust
// 使用列表 API
let url = format!("{}/greenfield/virtualgroup/global_virtual_group_families", rpc_url);
let response: VGFListResponse = client.get(&url).send().await?.json().await?;

// 找到匹配的 VGF
let vgf = response.global_virtual_group_families
    .iter()
    .find(|f| f.id == target_vgf_id)
    .ok_or("VGF not found")?;
```

---

### 问题 3: `no permission` (401) - 缺少 Header

**错误信息**：
```
❌ Upload failed: PUT failed with status 401 Unauthorized: 
<Error><Code>50004</Code><Message>no permission</Message></Error>
```

**排查过程**：

1. 在 SP 代码中搜索错误码 50004
2. 发现是权限验证失败
3. 追踪 `VerifyAuthentication` 函数

```go
// greenfield-storage-provider/modular/gater/object_handler.go
authenticated, err = g.baseApp.GfSpClient().VerifyAuthentication(
    reqCtx.Context(), coremodule.AuthOpTypePutObject,
    reqCtx.Account(), reqCtx.bucketName, reqCtx.objectName)

if !authenticated {
    log.CtxErrorw(reqCtx.Context(), "no permission to operate")
    err = ErrNoPermission  // 错误码 50004
    return
}
```

**分析**：
- `VerifyAuthentication` 先验证签名
- 签名验证失败会返回 `ErrRequestConsistent`
- 返回 `ErrNoPermission` 说明签名验证**看似通过**，但恢复的地址与 object creator 不匹配

**关键发现**：缺少 `X-Gnfd-Content-Sha256` header！

查看 Go SDK 代码：

```go
// greenfield-go-sdk/types/const.go
const EmptyStringSHA256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

// greenfield-go-sdk/client/api_object.go
reqMeta := requestMeta{
    contentSHA256: types.EmptyStringSHA256,  // 重要！
    // ...
}
```

**解决方案**：添加 `X-Gnfd-Content-Sha256` header：

```rust
const EMPTY_STRING_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

// 在请求头和签名中都要包含
request
    .header("X-Gnfd-Content-Sha256", EMPTY_STRING_SHA256)
    // ...
```

---

### 问题 4: `no permission` (401) - Canonical Request 格式错误

**错误信息**：同上

**排查过程**：

添加 `X-Gnfd-Content-Sha256` 后仍然报错。深入对比 Go SDK 和 Rust SDK 的 canonical request 构建。

**Go SDK 代码** (`greenfield-common/go/http/gen_sign_str.go`):

```go
func GetCanonicalRequest(req *http.Request) string {
    canonicalRequest := strings.Join([]string{
        req.Method,
        EncodePath(req.URL.Path),
        req.URL.RawQuery,
        getCanonicalHeaders(req, supportHeaders),
        getSignedHeaders(req, supportHeaders),
    }, "\n")
    return canonicalRequest
}
```

**关键发现**：

`strings.Join` 在每个元素之间插入 `\n`。而 `getCanonicalHeaders` 返回的字符串**末尾已有 `\n`**（host 后面），所以实际结果是：

```
Method\n
Path\n
Query\n
header1:value1\n
header2:value2\n
host.example.com\n
\n                    <- 两个换行！
signedHeaders
```

**Rust 代码 (错误版本)**：

```rust
format!("{}\n{}\n{}\n{}{}", method, path, query, canonical_headers, signed_headers)
// 结果：...host\nsignedHeaders  <- 只有一个换行！
```

**解决方案**：

```rust
// 修复前
format!("{}\n{}\n{}\n{}{}", ...)

// 修复后
format!("{}\n{}\n{}\n{}\n{}", ...)
//                      ^^ 添加 \n
```

---

### 问题 5: `no such object` - SP 未同步

**现象**：CreateObject 成功后立即 PutObject，返回错误或 `no permission`。

**排查过程**：

1. CreateObject 是链上交易
2. SP 需要从链上同步 object 元数据
3. 同步有延迟（几秒到十几秒）

**Go SDK 的处理** (`api_object.go`):

```go
func (c *Client) PutObject(...) error {
    // 等待 SP 同步 object 元数据
    if err := c.headSPObjectInfo(ctx, bucketName, objectName); err != nil {
        return err
    }
    // ... 继续上传
}

func (c *Client) headSPObjectInfo(ctx context.Context, bucketName, objectName string) error {
    backOff := 1.0
    for i := 0; i < 4; i++ {
        // 查询 SP
        objectInfo, err := c.HeadObjectFromSP(ctx, bucketName, objectName, types.HeadByNameOption{})
        if err == nil {
            return nil  // Object 已同步
        }
        time.Sleep(time.Duration(backOff) * time.Second)
        backOff *= 2  // 指数退避
    }
    return errors.New("object not synced")
}
```

**解决方案**：在 PutObject 前添加重试逻辑：

```rust
async fn wait_for_sp_object_sync(
    &self, sp_url: &str, bucket: &str, object: &str
) -> Result<(), Box<dyn std::error::Error>> {
    let mut backoff = 1.0;
    for _ in 0..4 {
        match self.head_object_from_sp(sp_url, bucket, object).await {
            Ok(_) => return Ok(()),
            Err(e) if e.to_string().contains("no such object") => {
                tokio::time::sleep(Duration::from_secs_f64(backoff)).await;
                backoff *= 2.0;
            }
            Err(_) => break,  // 其他错误说明 object 存在
        }
    }
    Ok(())
}
```

---

## Go SDK 代码追踪方法

### 方法论

```
┌─────────────────────────────────────────────────────────────┐
│                    Go SDK 代码追踪路径                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 找入口：greenfield-go-sdk/client/api_object.go          │
│     └─> PutObject() 函数                                    │
│                                                             │
│  2. 找请求构建：                                             │
│     └─> c.sendReq(ctx, reqMeta, &sendOpt, endpoint)         │
│     └─> 跳转到 api_client.go                                │
│                                                             │
│  3. 找签名逻辑：                                             │
│     └─> greenfield-common/go/http/gen_sign_str.go           │
│     └─> GetCanonicalRequest(), GetMsgToSignInGNFD1Auth()    │
│                                                             │
│  4. 找 SP 验证逻辑：                                         │
│     └─> greenfield-storage-provider/modular/gater/          │
│     └─> request_context.go - verifySignatureForGNFD1Ecdsa() │
│                                                             │
│  5. 找支持的 Headers 列表：                                   │
│     └─> greenfield-common/go/http/const.go                  │
│     └─> supportHeads 变量                                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 关键文件路径

```
greenfield-go-sdk/
├── client/
│   ├── api_object.go          # PutObject 入口
│   └── api_client.go          # sendReq, 请求签名
├── types/
│   └── const.go               # EmptyStringSHA256

greenfield-common/
├── go/http/
│   ├── gen_sign_str.go        # GetCanonicalRequest, GetMsgToSignInGNFD1Auth
│   └── const.go               # supportHeads

greenfield-storage-provider/
├── modular/gater/
│   ├── request_context.go     # verifySignatureForGNFD1Ecdsa
│   └── object_handler.go      # PutObject 处理
```

### 关键代码片段

**1. PutObject 入口** (`api_object.go`)

```go
func (c *Client) PutObject(ctx context.Context, bucketName, objectName string,
    objectSize int64, reader io.Reader, opts types.PutObjectOptions) error {
    
    // 1. 等待 SP 同步
    if err := c.headSPObjectInfo(ctx, bucketName, objectName); err != nil {
        return err
    }
    
    // 2. 获取 SP endpoint
    bucketInfo, _, err := c.GetBucketMeta(ctx, bucketName, types.GetBucketMetaOptions{})
    spEndpoint := c.getSPEndpointFromBucketInfo(bucketInfo)
    
    // 3. 构建请求元数据
    reqMeta := requestMeta{
        bucketName:    bucketName,
        objectName:    objectName,
        contentSHA256: types.EmptyStringSHA256,  // 重要！
        contentLength: objectSize,
        contentType:   contentType,
    }
    
    // 4. 发送请求
    sendOpt := sendOptions{
        method:    http.MethodPut,
        body:      reader,
        isAdminAPI: false,
    }
    _, err = c.sendReq(ctx, reqMeta, &sendOpt, spEndpoint)
    return err
}
```

**2. Canonical Request 构建** (`gen_sign_str.go`)

```go
var supportHeads = []string{
    HTTPHeaderContentSHA256,    // x-gnfd-content-sha256
    HTTPHeaderTransactionHash,
    HTTPHeaderObjectID,
    HTTPHeaderRedundancyIndex,
    HTTPHeaderResource,
    HTTPHeaderDate,
    HTTPHeaderRange,
    HTTPHeaderPieceIndex,
    HTTPHeaderContentType,      // content-type
    HTTPHeaderContentMD5,
    HTTPHeaderUnsignedMsg,
    HTTPHeaderUserAddress,
    HTTPHeaderExpiryTimestamp,  // x-gnfd-expiry-timestamp
}

func GetCanonicalRequest(req *http.Request) string {
    supportHeaders := initSupportHeaders()
    canonicalRequest := strings.Join([]string{
        req.Method,
        EncodePath(req.URL.Path),
        req.URL.RawQuery,
        getCanonicalHeaders(req, supportHeaders),
        getSignedHeaders(req, supportHeaders),
    }, "\n")
    return canonicalRequest
}

func getCanonicalHeaders(req *http.Request, supportHeaders map[string]bool) string {
    var content strings.Builder
    headers := getCanonicalHeaderList(req, supportHeaders)
    for _, header := range headers {
        content.WriteString(header)
        content.WriteByte(':')
        content.WriteString(req.Header.Get(header))
        content.WriteByte('\n')
    }
    // 添加 host（不带 "host:" 前缀）
    if !containHostHeader {
        content.WriteString(GetHostInfo(req))
        content.WriteByte('\n')
    }
    return content.String()  // 末尾有 \n
}
```

**3. SP 签名验证** (`request_context.go`)

```go
func (r *RequestContext) verifySignatureForGNFD1Ecdsa(requestSignature string) (sdk.AccAddress, error) {
    // 重建 canonical request
    realMsgToSign := commonhttp.GetMsgToSignInGNFD1Auth(r.request)
    
    // 解码签名
    signature, err := hex.DecodeString(requestSignature)
    
    // 恢复地址
    addr, _, err := commonhash.RecoverAddr(realMsgToSign, signature)
    
    return addr, nil
}
```

---

## 从零开始实现 Upload Object

### Step 1: 理解 Canonical Request 格式

```
PUT\n
/bucket-name/object-name\n
\n
content-type:application/octet-stream\n
x-gnfd-content-sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n
x-gnfd-expiry-timestamp:2026-01-20T12:50:07Z\n
gnfd-testnet-sp3.bnbchain.org\n
\n
content-type;x-gnfd-content-sha256;x-gnfd-expiry-timestamp
```

### Step 2: Headers 规则

**必需的 Headers**：

| Header | 值 | 说明 |
|--------|-----|------|
| `Content-Type` | `application/octet-stream` | 内容类型 |
| `X-Gnfd-Content-Sha256` | `e3b0c44298fc...` | 空字符串的 SHA256 |
| `X-Gnfd-Expiry-Timestamp` | `2026-01-20T12:50:07Z` | ISO 8601 格式，UTC |
| `Authorization` | `GNFD1-ECDSA, Signature=...` | 签名 |

**Header 排序**：按字母顺序（小写）

```
content-type
x-gnfd-content-sha256
x-gnfd-expiry-timestamp
```

**Host 特殊处理**：
- 不在 `supportHeads` 列表中
- 作为最后一行添加到 canonical headers
- **不带** `host:` 前缀，只有值

### Step 3: 签名格式

```
Authorization: GNFD1-ECDSA, Signature=<65字节签名的hex编码>
```

**签名计算**：
```
message = keccak256(canonical_request)
signature = secp256k1_sign(message, private_key)  // 65 bytes: R || S || V
V = V - 27  // 转换 V 值：27/28 -> 0/1
```

### Step 4: 时间戳格式

```
2026-01-20T12:50:07Z
```

- ISO 8601 格式
- UTC 时区（Z 后缀）
- 建议设置为当前时间 + 1000 秒

---

## Rust 代码详解

### 完整 Upload 流程 (`client.rs`)

```rust
pub async fn upload(
    &self,
    bucket_name: &str,
    object_name: &str,
    file_path: &str,
    visibility: i32,
    sp_url: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    
    // Step 1: 计算 checksums
    println!("📊 Computing file checksums...");
    let (checksums, file_size) = compute_hash_from_file(file_path)?;
    
    // Step 2: CreateObject (链上交易)
    println!("📝 Creating object metadata on chain...");
    let tx_hash = self.create_object_internal(
        bucket_name, object_name, checksums, file_size, visibility, "application/octet-stream"
    ).await?;
    println!("✅ Object created: {}", tx_hash);
    
    // Step 3: 获取 SP endpoint
    let sp_endpoint = match sp_url {
        Some(url) => url.to_string(),
        None => {
            println!("🔍 Looking up primary SP for bucket...");
            self.get_bucket_primary_sp(bucket_name).await?
        }
    };
    
    // Step 4: 等待 SP 同步
    println!("⏳ Waiting for SP to sync object metadata...");
    self.wait_for_sp_object_sync(&sp_endpoint, bucket_name, object_name).await?;
    
    // Step 5: PutObject (HTTP 请求)
    println!("📤 Uploading file to SP...");
    self.put_object(&sp_endpoint, bucket_name, object_name, file_path).await?;
    
    println!("✅ Upload completed!");
    Ok(tx_hash)
}
```

### PutObject 签名和请求 (`client.rs`)

```rust
pub async fn put_object(
    &self,
    sp_url: &str,
    bucket_name: &str,
    object_name: &str,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    
    // 1. 读取文件
    let file_content = std::fs::read(file_path)?;
    let content_type = "application/octet-stream";
    
    // 2. 构建 URL
    let url_path = format!("/{}/{}", bucket_name, object_name);
    let full_url = format!("{}{}", sp_url.trim_end_matches('/'), url_path);
    
    // 3. 提取 host
    let parsed_url = reqwest::Url::parse(&full_url)?;
    let sp_host = parsed_url.host_str().ok_or("Invalid SP URL")?;
    
    // 4. 构建时间戳
    let expiry = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(1000))
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    
    // 5. 构建 Canonical Headers（按字母排序）
    const EMPTY_STRING_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    
    let canonical_headers = format!(
        "content-type:{}\nx-gnfd-content-sha256:{}\nx-gnfd-expiry-timestamp:{}\n{}\n",
        content_type,
        EMPTY_STRING_SHA256,
        expiry,
        sp_host  // Host 不带 "host:" 前缀
    );
    
    let signed_headers = "content-type;x-gnfd-content-sha256;x-gnfd-expiry-timestamp";
    
    // 6. 构建 Canonical Request
    // 注意：canonical_headers 末尾已有 \n，这里再加一个 \n 确保两个换行
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}",
        "PUT",
        url_path,
        "",  // 空 query string
        canonical_headers,
        signed_headers
    );
    
    // 7. 计算签名
    let msg_to_sign = ethers::utils::keccak256(canonical_request.as_bytes());
    let signature = self.wallet.sign_hash(H256::from(msg_to_sign))?;
    
    // 8. 转换 V 值 (27/28 -> 0/1)
    let mut sig_bytes = signature.to_vec();
    if sig_bytes[64] >= 27 {
        sig_bytes[64] -= 27;
    }
    
    // 9. 构建 Authorization header
    let auth_header = format!("GNFD1-ECDSA, Signature={}", hex::encode(&sig_bytes));
    
    // 10. 发送请求
    let client = reqwest::Client::new();
    let response = client
        .put(&full_url)
        .header("Authorization", auth_header)
        .header("Content-Type", content_type)
        .header("Content-Length", file_content.len().to_string())
        .header("X-Gnfd-Content-Sha256", EMPTY_STRING_SHA256)
        .header("X-Gnfd-Expiry-Timestamp", &expiry)
        .body(file_content)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(format!("PUT failed with status {}: {}", status, body).into());
    }
    
    Ok(())
}
```

### 等待 SP 同步 (`client.rs`)

```rust
async fn wait_for_sp_object_sync(
    &self,
    sp_url: &str,
    bucket_name: &str,
    object_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut backoff = Duration::from_secs(1);
    
    for retry in 0..4 {
        println!("  Checking SP sync status (attempt {}/4)...", retry + 1);
        
        match self.head_object_from_sp(sp_url, bucket_name, object_name).await {
            Ok(_) => {
                println!("  ✓ Object metadata synced to SP");
                return Ok(());
            }
            Err(e) if e.to_string().contains("no such object") => {
                println!("  ⏳ Not yet synced, waiting {:?}...", backoff);
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
            Err(_) => {
                // 其他错误说明 object 存在（只是可能有其他问题）
                println!("  ✓ Object exists on SP");
                return Ok(());
            }
        }
    }
    
    println!("  ⚠ Proceeding without confirmation (SP may still be syncing)");
    Ok(())
}

async fn head_object_from_sp(
    &self,
    sp_url: &str,
    bucket_name: &str,
    object_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 使用 ?upload-progress 查询参数
    let url = format!("{}/{}/{}?upload-progress", sp_url, bucket_name, object_name);
    
    // 需要签名认证
    let expiry = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(1000))
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    
    // ... 构建签名，发送 GET 请求 ...
    
    let response = client.get(&url)
        .header("Authorization", auth_header)
        .header("X-Gnfd-Expiry-Timestamp", &expiry)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        let body = response.text().await?;
        Err(body.into())
    }
}
```

---

## 关键经验总结

### 1. Canonical Request 换行符

**最常见的错误！**

```
正确：...header\nhost\n\nsignedHeaders  (两个换行)
错误：...header\nhost\nsignedHeaders   (一个换行)
```

### 2. Header 排序

必须按**字母顺序**（小写）：
```
content-type
x-gnfd-content-sha256
x-gnfd-expiry-timestamp
```

### 3. Host 处理

- Host 不在 `supportHeads` 列表中
- 单独添加到 canonical headers 末尾
- **只有值，没有 `host:` 前缀**

### 4. V 值转换

```rust
// ECDSA 签名的 V 值需要转换
if sig_bytes[64] >= 27 {
    sig_bytes[64] -= 27;  // 27/28 -> 0/1
}
```

### 5. SP 同步等待

CreateObject 成功后，SP 需要时间同步元数据。使用指数退避重试：

```rust
backoff = 1s -> 2s -> 4s -> 8s
```

### 6. 必需的 Headers

| Header | 必须在请求中 | 必须在签名中 |
|--------|------------|------------|
| Content-Type | ✅ | ✅ |
| X-Gnfd-Content-Sha256 | ✅ | ✅ |
| X-Gnfd-Expiry-Timestamp | ✅ | ✅ |
| Authorization | ✅ | ❌ |
| Content-Length | ✅ | ❌ |

### 7. Primary SP 获取流程

```
Bucket -> global_virtual_group_family_id -> VGF -> primary_sp_id -> SP endpoint
```

---

## 常见错误速查表

| 错误码 | 错误信息 | 原因 | 解决方案 |
|--------|----------|------|----------|
| 400 20002 | mismatched primary sp | 上传到错误的 SP | 自动获取 bucket 的 primary SP |
| 401 50004 | no permission | 签名验证后地址不匹配 | 检查 canonical request 格式 |
| 401 50001 | unsupported sign type | Authorization 格式错误 | 使用 `GNFD1-ECDSA, Signature=` |
| 401 50023 | incorrect expiry timestamp | 时间戳格式错误 | 使用 ISO 8601 格式 `Z` 后缀 |
| 404 | no such object | SP 未同步 object | 添加重试等待逻辑 |

---

## 调试工具

### 打印 Canonical Request

```rust
println!("📋 Canonical Request:");
println!("---");
println!("{}", canonical_request);
println!("---");
println!("🔑 Canonical Request bytes: {:?}", canonical_request.as_bytes());
println!("🔑 Message to sign: 0x{}", hex::encode(&msg_to_sign));
```

### 验证签名恢复

```rust
let recovered = signature.recover(H256::from(msg_to_sign))?;
println!("Recovered address: {:?}", recovered);
println!("Expected address: {:?}", self.wallet.address());
assert_eq!(recovered, self.wallet.address());
```

### 对比 Go SDK

在 Go SDK 的 `greenfield-common/go/http/gen_sign_str.go` 添加调试输出：

```go
func GetCanonicalRequest(req *http.Request) string {
    // ... 原有代码 ...
    fmt.Printf("DEBUG Canonical Request:\n%s\n", canonicalRequest)
    fmt.Printf("DEBUG Bytes: %x\n", []byte(canonicalRequest))
    return canonicalRequest
}
```

---

## 参考资料

- [Greenfield 文档](https://docs.bnbchain.org/greenfield/)
- [Go SDK 源码](https://github.com/bnb-chain/greenfield-go-sdk)
- [greenfield-common 源码](https://github.com/bnb-chain/greenfield-common)
- [SP 源码](https://github.com/bnb-chain/greenfield-storage-provider)

