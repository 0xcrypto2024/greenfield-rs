// Greenfield SDK 调试工具
// 用于输出交易的中间值，方便与 Rust SDK 对比
//
// 使用前需要设置环境：
// 1. 克隆 greenfield-go-sdk, greenfield-cosmos-sdk, greenfield-common
// 2. 在 go.mod 中添加 replace 指令指向本地仓库
// 3. 运行 go mod tidy

package main

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math/big"
	"os"
	"sort"
	"strconv"
	"strings"

	"golang.org/x/crypto/sha3"
)

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	switch os.Args[1] {
	case "create-bucket":
		debugCreateBucket()
	case "create-object":
		debugCreateObject()
	case "put-object":
		debugPutObject()
	case "eip712-types":
		printEIP712Types()
	case "domain-separator":
		debugDomainSeparator()
	default:
		printUsage()
	}
}

func printUsage() {
	fmt.Println(`Greenfield SDK Debug Tool

Usage:
    go run main.go <command> [options]

Commands:
    create-bucket      Debug CreateBucket EIP-712 signing
    create-object      Debug CreateObject EIP-712 signing  
    put-object         Debug PutObject Canonical Request
    eip712-types       Print EIP-712 type strings
    domain-separator   Debug Domain Separator calculation

Examples:
    go run main.go create-bucket --bucket my-bucket --sp 0x5ccF0F6b78a37Ef4e2CcBC10D155c28Fb8bE9BaF --vgf-id 1
    go run main.go create-object --bucket my-bucket --object file.txt
    go run main.go put-object --bucket my-bucket --object file.txt --host sp.example.com
    go run main.go eip712-types
    go run main.go domain-separator --chain-id 5600`)
}

// ============== Domain Separator ==============

func debugDomainSeparator() {
	chainID := "5600" // 默认测试网

	for i, arg := range os.Args {
		if arg == "--chain-id" && i+1 < len(os.Args) {
			chainID = os.Args[i+1]
		}
	}

	fmt.Println("=== Domain Separator Debug ===")
	fmt.Println()

	// EIP712Domain type string
	domainTypeStr := "EIP712Domain(uint256 chainId,string name,string salt,string verifyingContract,string version)"
	domainTypeHash := keccak256([]byte(domainTypeStr))

	fmt.Printf("Domain Type String: %s\n", domainTypeStr)
	fmt.Printf("Domain Type Hash: 0x%s\n", hex.EncodeToString(domainTypeHash))
	fmt.Println()

	// 计算各字段的编码
	chainIDInt, _ := strconv.ParseInt(chainID, 10, 64)
	chainIDBytes := leftPad32(big.NewInt(chainIDInt).Bytes())

	nameHash := keccak256([]byte("Greenfield Tx"))
	saltHash := keccak256([]byte("0"))
	vcHash := keccak256([]byte("greenfield"))
	versionHash := keccak256([]byte("1.0.0"))

	fmt.Println("Field Encodings:")
	fmt.Printf("  chainId (%s): 0x%s\n", chainID, hex.EncodeToString(chainIDBytes))
	fmt.Printf("  name hash: 0x%s\n", hex.EncodeToString(nameHash))
	fmt.Printf("  salt hash: 0x%s\n", hex.EncodeToString(saltHash))
	fmt.Printf("  verifyingContract hash: 0x%s\n", hex.EncodeToString(vcHash))
	fmt.Printf("  version hash: 0x%s\n", hex.EncodeToString(versionHash))
	fmt.Println()

	// 组合计算 Domain Separator
	var encoded []byte
	encoded = append(encoded, domainTypeHash...)
	encoded = append(encoded, chainIDBytes...)
	encoded = append(encoded, nameHash...)
	encoded = append(encoded, saltHash...)
	encoded = append(encoded, vcHash...)
	encoded = append(encoded, versionHash...)

	domainSeparator := keccak256(encoded)

	fmt.Printf("Encoded bytes length: %d\n", len(encoded))
	fmt.Printf("Domain Separator: 0x%s\n", hex.EncodeToString(domainSeparator))
}

// ============== EIP-712 Types ==============

func printEIP712Types() {
	fmt.Println("=== EIP-712 Type Strings ===")
	fmt.Println()

	// CreateBucket types
	fmt.Println("--- CreateBucket ---")
	txType := "Tx(uint256 account_number,uint256 chain_id,Fee fee,string memo,Msg1 msg1,uint256 sequence,uint256 timeout_height)"
	coinType := "Coin(uint256 amount,string denom)"
	feeType := "Fee(Coin[] amount,uint256 gas_limit,string granter,string payer)"
	msg1BucketType := "Msg1(string bucket_name,uint64 charged_read_quota,string creator,string payment_address,string primary_sp_address,TypeMsg1PrimarySpApproval primary_sp_approval,string type,string visibility)"
	approvalBucketType := "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)"

	fullBucketType := txType + coinType + feeType + msg1BucketType + approvalBucketType
	fmt.Printf("Full Type String:\n%s\n\n", fullBucketType)
	fmt.Printf("Type Hash: 0x%s\n", hex.EncodeToString(keccak256([]byte(fullBucketType))))
	fmt.Println()

	// CreateObject types
	fmt.Println("--- CreateObject ---")
	msg1ObjectType := "Msg1(string bucket_name,string content_type,string creator,bytes[] expect_checksums,string object_name,uint64 payload_size,TypeMsg1PrimarySpApproval primary_sp_approval,string redundancy_type,string type,string visibility)"
	approvalObjectType := "TypeMsg1PrimarySpApproval(uint64 expired_height,uint32 global_virtual_group_family_id)"

	fullObjectType := txType + coinType + feeType + msg1ObjectType + approvalObjectType
	fmt.Printf("Full Type String:\n%s\n\n", fullObjectType)
	fmt.Printf("Type Hash: 0x%s\n", hex.EncodeToString(keccak256([]byte(fullObjectType))))
	fmt.Println()

	// 各嵌套类型的 Type Hash
	fmt.Println("--- Nested Type Hashes ---")
	fmt.Printf("Coin Type Hash: 0x%s\n", hex.EncodeToString(keccak256([]byte(coinType))))
	fmt.Printf("Fee Type Hash: 0x%s\n", hex.EncodeToString(keccak256([]byte(feeType))))
	fmt.Printf("Msg1 (Bucket) Type Hash: 0x%s\n", hex.EncodeToString(keccak256([]byte(msg1BucketType))))
	fmt.Printf("Msg1 (Object) Type Hash: 0x%s\n", hex.EncodeToString(keccak256([]byte(msg1ObjectType))))
	fmt.Printf("Approval Type Hash: 0x%s\n", hex.EncodeToString(keccak256([]byte(approvalBucketType))))
}

// ============== CreateBucket Debug ==============

func debugCreateBucket() {
	bucketName := "test-bucket"
	spAddress := "0x5ccF0F6b78a37Ef4e2CcBC10D155c28Fb8bE9BaF"
	vgfID := uint32(1)
	creator := "0xEa39644C04b40316f7270EDf7bB4249c6F47caFE"
	visibility := "VISIBILITY_TYPE_PRIVATE"
	accountNumber := "123"
	sequence := "0"
	chainID := "5600"

	// 解析命令行参数
	for i, arg := range os.Args {
		switch arg {
		case "--bucket":
			if i+1 < len(os.Args) {
				bucketName = os.Args[i+1]
			}
		case "--sp":
			if i+1 < len(os.Args) {
				spAddress = os.Args[i+1]
			}
		case "--vgf-id":
			if i+1 < len(os.Args) {
				id, _ := strconv.ParseUint(os.Args[i+1], 10, 32)
				vgfID = uint32(id)
			}
		case "--creator":
			if i+1 < len(os.Args) {
				creator = os.Args[i+1]
			}
		case "--account-number":
			if i+1 < len(os.Args) {
				accountNumber = os.Args[i+1]
			}
		case "--sequence":
			if i+1 < len(os.Args) {
				sequence = os.Args[i+1]
			}
		}
	}

	fmt.Println("=== CreateBucket EIP-712 Debug ===")
	fmt.Println()

	// 构建 EIP-712 消息
	msg := map[string]interface{}{
		"type":               "/greenfield.storage.MsgCreateBucket",
		"bucket_name":        bucketName,
		"charged_read_quota": "0",
		"creator":            creator,
		"payment_address":    "",
		"primary_sp_address": spAddress,
		"primary_sp_approval": map[string]interface{}{
			"expired_height":                 "0",
			"global_virtual_group_family_id": vgfID,
		},
		"visibility": visibility,
	}

	tx := map[string]interface{}{
		"account_number": accountNumber,
		"chain_id":       chainID,
		"fee": map[string]interface{}{
			"amount": []map[string]interface{}{
				{"amount": "5000000000000", "denom": "BNB"},
			},
			"gas_limit": "2400",
			"granter":   "",
			"payer":     creator,
		},
		"memo":           "",
		"msg1":           msg,
		"sequence":       sequence,
		"timeout_height": "0",
	}

	// 打印 JSON
	jsonBytes, _ := json.MarshalIndent(tx, "", "  ")
	fmt.Println("EIP-712 Message JSON:")
	fmt.Println(string(jsonBytes))
	fmt.Println()

	// 打印各字段的哈希
	fmt.Println("--- Field Hashes ---")
	printFieldHashes(tx)
}

// ============== CreateObject Debug ==============

func debugCreateObject() {
	bucketName := "test-bucket"
	objectName := "test-file.txt"
	creator := "0xEa39644C04b40316f7270EDf7bB4249c6F47caFE"
	visibility := "VISIBILITY_TYPE_PRIVATE"
	accountNumber := "123"
	sequence := "1"
	chainID := "5600"

	// 生成示例 checksums (7 个)
	var checksums [][]byte
	for i := 0; i < 7; i++ {
		hash := sha256.Sum256([]byte(fmt.Sprintf("segment_%d", i)))
		checksums = append(checksums, hash[:])
	}

	// 解析命令行参数
	for i, arg := range os.Args {
		switch arg {
		case "--bucket":
			if i+1 < len(os.Args) {
				bucketName = os.Args[i+1]
			}
		case "--object":
			if i+1 < len(os.Args) {
				objectName = os.Args[i+1]
			}
		case "--creator":
			if i+1 < len(os.Args) {
				creator = os.Args[i+1]
			}
		}
	}

	fmt.Println("=== CreateObject EIP-712 Debug ===")
	fmt.Println()

	// 打印 checksums (Base64 格式，这是 Go SDK 的行为)
	fmt.Println("--- Checksums (Base64) ---")
	var checksumStrings []string
	for i, cs := range checksums {
		b64 := base64.StdEncoding.EncodeToString(cs)
		checksumStrings = append(checksumStrings, b64)
		fmt.Printf("  [%d] Hex: %s\n", i, hex.EncodeToString(cs))
		fmt.Printf("      Base64: %s\n", b64)
		fmt.Printf("      Base64 bytes: %s\n", hex.EncodeToString([]byte(b64)))
		fmt.Printf("      keccak256(Base64 bytes): %s\n", hex.EncodeToString(keccak256([]byte(b64))))
	}
	fmt.Println()

	// 计算 checksums 数组的哈希
	var checksumArrayHash []byte
	for _, cs := range checksums {
		b64 := base64.StdEncoding.EncodeToString(cs)
		checksumArrayHash = append(checksumArrayHash, keccak256([]byte(b64))...)
	}
	fmt.Printf("Checksums Array Hash: 0x%s\n", hex.EncodeToString(keccak256(checksumArrayHash)))
	fmt.Println()

	// 构建 EIP-712 消息
	msg := map[string]interface{}{
		"type":              "/greenfield.storage.MsgCreateObject",
		"bucket_name":       bucketName,
		"content_type":      "application/octet-stream",
		"creator":           creator,
		"expect_checksums":  checksumStrings, // Base64 strings
		"object_name":       objectName,
		"payload_size":      "1024",
		"primary_sp_approval": map[string]interface{}{
			"expired_height":                 "18446744073709551615", // u64::MAX
			"global_virtual_group_family_id": "0",                   // CreateObject 必须为 0
		},
		"redundancy_type": "REDUNDANCY_EC_TYPE",
		"visibility":      visibility,
	}

	tx := map[string]interface{}{
		"account_number": accountNumber,
		"chain_id":       chainID,
		"fee": map[string]interface{}{
			"amount": []map[string]interface{}{
				{"amount": "6000000000000", "denom": "BNB"},
			},
			"gas_limit": "1200",
			"granter":   "",
			"payer":     creator,
		},
		"memo":           "",
		"msg1":           msg,
		"sequence":       sequence,
		"timeout_height": "0",
	}

	// 打印 JSON
	jsonBytes, _ := json.MarshalIndent(tx, "", "  ")
	fmt.Println("EIP-712 Message JSON:")
	fmt.Println(string(jsonBytes))
	fmt.Println()

	// 打印各字段的哈希
	fmt.Println("--- Field Hashes ---")
	printFieldHashes(tx)
}

// ============== PutObject Debug ==============

func debugPutObject() {
	bucketName := "test-bucket"
	objectName := "test-file.txt"
	spHost := "gnfd-testnet-sp3.bnbchain.org"
	expiry := "2026-01-20T12:50:07Z"
	contentType := "application/octet-stream"

	// 解析命令行参数
	for i, arg := range os.Args {
		switch arg {
		case "--bucket":
			if i+1 < len(os.Args) {
				bucketName = os.Args[i+1]
			}
		case "--object":
			if i+1 < len(os.Args) {
				objectName = os.Args[i+1]
			}
		case "--host":
			if i+1 < len(os.Args) {
				spHost = os.Args[i+1]
			}
		case "--expiry":
			if i+1 < len(os.Args) {
				expiry = os.Args[i+1]
			}
		}
	}

	fmt.Println("=== PutObject Canonical Request Debug ===")
	fmt.Println()

	// 空字符串的 SHA256
	emptyStringSHA256 := "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

	// 构建 Headers (按字母排序)
	headers := map[string]string{
		"content-type":            contentType,
		"x-gnfd-content-sha256":   emptyStringSHA256,
		"x-gnfd-expiry-timestamp": expiry,
	}

	// 打印 Headers
	fmt.Println("--- Headers (sorted) ---")
	var sortedKeys []string
	for k := range headers {
		sortedKeys = append(sortedKeys, k)
	}
	sort.Strings(sortedKeys)
	for _, k := range sortedKeys {
		fmt.Printf("  %s: %s\n", k, headers[k])
	}
	fmt.Println()

	// 构建 Canonical Headers
	var canonicalHeaders strings.Builder
	for _, k := range sortedKeys {
		canonicalHeaders.WriteString(k)
		canonicalHeaders.WriteByte(':')
		canonicalHeaders.WriteString(headers[k])
		canonicalHeaders.WriteByte('\n')
	}
	// 添加 Host（不带 "host:" 前缀）
	canonicalHeaders.WriteString(spHost)
	canonicalHeaders.WriteByte('\n')

	// Signed Headers
	signedHeaders := strings.Join(sortedKeys, ";")

	// 构建 Canonical Request
	// 使用 strings.Join 模拟 Go SDK 的行为
	urlPath := "/" + bucketName + "/" + objectName
	canonicalRequest := strings.Join([]string{
		"PUT",
		urlPath,
		"", // empty query
		canonicalHeaders.String(),
		signedHeaders,
	}, "\n")

	fmt.Println("--- Canonical Request ---")
	fmt.Println("```")
	fmt.Print(canonicalRequest)
	fmt.Println("```")
	fmt.Println()

	// 打印字节表示
	fmt.Println("--- Canonical Request Bytes ---")
	fmt.Printf("Length: %d\n", len(canonicalRequest))
	fmt.Printf("Hex: %s\n", hex.EncodeToString([]byte(canonicalRequest)))
	fmt.Println()

	// 显示关键的换行符位置
	fmt.Println("--- Newline Analysis ---")
	for i, b := range []byte(canonicalRequest) {
		if b == '\n' {
			fmt.Printf("  Newline at position %d\n", i)
		}
	}
	fmt.Println()

	// 计算 Message to Sign
	msgToSign := keccak256([]byte(canonicalRequest))
	fmt.Printf("Message to Sign: 0x%s\n", hex.EncodeToString(msgToSign))
}

// ============== Helper Functions ==============

func keccak256(data []byte) []byte {
	h := sha3.NewLegacyKeccak256()
	h.Write(data)
	return h.Sum(nil)
}

func leftPad32(data []byte) []byte {
	if len(data) >= 32 {
		return data[:32]
	}
	padded := make([]byte, 32)
	copy(padded[32-len(data):], data)
	return padded
}

func printFieldHashes(data map[string]interface{}) {
	for k, v := range data {
		switch val := v.(type) {
		case string:
			hash := keccak256([]byte(val))
			fmt.Printf("  %s: \"%s\" -> 0x%s\n", k, val, hex.EncodeToString(hash))
		case map[string]interface{}:
			fmt.Printf("  %s: (nested struct)\n", k)
			printFieldHashes(val)
		case []map[string]interface{}:
			fmt.Printf("  %s: (array of %d items)\n", k, len(val))
		case []string:
			fmt.Printf("  %s: (array of %d strings)\n", k, len(val))
			for i, s := range val {
				fmt.Printf("    [%d]: \"%s\" -> 0x%s\n", i, s, hex.EncodeToString(keccak256([]byte(s))))
			}
		}
	}
}

