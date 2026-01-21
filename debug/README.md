# Greenfield SDK 调试工具

本目录包含用于调试 Greenfield 交易的工具，帮助对比 Go SDK 和 Rust SDK 的实现差异。

## 工具列表

| 文件 | 说明 |
|------|------|
| `go_debug/main.go` | Go SDK 调试程序，输出交易的中间值 |
| `compare_tx.sh` | 对比 Go 和 Rust 输出的脚本 |

## 使用方法

### 1. 设置 Go 调试环境

```bash
cd debug/go_debug
go mod init debug_greenfield
go mod tidy
```

### 2. 运行 Go 调试

```bash
# 调试 CreateBucket
go run main.go create-bucket \
    --bucket my-bucket \
    --sp-address 0x5ccF0F6b78a37Ef4e2CcBC10D155c28Fb8bE9BaF \
    --vgf-id 1

# 调试 CreateObject
go run main.go create-object \
    --bucket my-bucket \
    --object my-file.txt \
    --file ./testfile.txt

# 调试 PutObject Canonical Request
go run main.go put-object \
    --bucket my-bucket \
    --object my-file.txt \
    --sp-host gnfd-testnet-sp3.bnbchain.org
```

### 3. 运行 Rust 调试

```bash
# 在项目根目录
RUST_LOG=debug cargo run -- --keystore ./my-wallet create-bucket ...
```

### 4. 对比输出

```bash
./compare_tx.sh go_output.txt rust_output.txt
```

## 调试输出说明

### EIP-712 调试输出

```
=== EIP-712 Debug Info ===
Domain Separator: 0x...
Type Hash: 0x...
Struct Hash: 0x...
Final EIP-712 Hash: 0x...
Signature: 0x...
```

### Canonical Request 调试输出

```
=== GNFD1-ECDSA Debug Info ===
Canonical Request:
---
PUT
/bucket/object

content-type:application/octet-stream
x-gnfd-content-sha256:e3b0c...
x-gnfd-expiry-timestamp:2026-01-20T12:50:07Z
gnfd-testnet-sp3.bnbchain.org

content-type;x-gnfd-content-sha256;x-gnfd-expiry-timestamp
---
Message to Sign: 0x...
Signature: 0x...
```

## 常见问题对比点

### 1. EIP-712 签名问题

检查以下值是否一致：
- Domain Separator
- Type Hash（包括嵌套类型的 Type Hash）
- 各字段的编码值
- Struct Hash
- Final Hash

### 2. SP 请求签名问题

检查以下值是否一致：
- Canonical Headers 的顺序
- Host 的格式（有无 port）
- 换行符的数量
- Message to Sign

## 调试技巧

### 在 Go SDK 中添加调试输出

1. **EIP-712 签名** - 修改 `greenfield-cosmos-sdk/x/auth/tx/eip712.go`:

```go
func getSignBytes(...) ([]byte, error) {
    // ... 原有代码 ...
    
    // 添加调试输出
    fmt.Printf("=== EIP-712 Debug ===\n")
    fmt.Printf("Domain Separator: %x\n", domainSeparator)
    fmt.Printf("Type Hash: %x\n", typeHash)
    fmt.Printf("Struct Hash: %x\n", structHash)
    fmt.Printf("Final Hash: %x\n", finalHash)
    
    return finalHash, nil
}
```

2. **SP 签名** - 修改 `greenfield-common/go/http/gen_sign_str.go`:

```go
func GetCanonicalRequest(req *http.Request) string {
    // ... 原有代码 ...
    
    fmt.Printf("=== Canonical Request Debug ===\n")
    fmt.Printf("Canonical Request:\n---\n%s\n---\n", canonicalRequest)
    fmt.Printf("Bytes: %x\n", []byte(canonicalRequest))
    
    return canonicalRequest
}
```

### 在 Rust SDK 中添加调试输出

1. **EIP-712 签名** - 在 `eip712.rs` 或 `bucket_eip712.rs`:

```rust
pub fn get_eip712_hash(&self, chain_id: &str) -> Result<H256, String> {
    let domain_separator = get_domain_separator(chain_id)?;
    let struct_hash = self.get_struct_hash()?;
    
    println!("=== EIP-712 Debug ===");
    println!("Domain Separator: 0x{}", hex::encode(domain_separator.as_bytes()));
    println!("Type Hash: 0x{}", hex::encode(Self::get_type_hash()));
    println!("Struct Hash: 0x{}", hex::encode(struct_hash.as_bytes()));
    
    // ... 计算 final hash ...
    
    println!("Final Hash: 0x{}", hex::encode(final_hash.as_bytes()));
    
    Ok(final_hash)
}
```

2. **SP 签名** - 在 `client.rs` 的 `put_object`:

```rust
println!("=== Canonical Request Debug ===");
println!("Canonical Request:\n---\n{}\n---", canonical_request);
println!("Bytes: {:02x?}", canonical_request.as_bytes());
println!("Message to Sign: 0x{}", hex::encode(&msg_to_sign));
```

