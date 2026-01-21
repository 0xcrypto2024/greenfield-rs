#!/bin/bash
# Greenfield SDK 调试对比脚本
# 用于对比 Go SDK 和 Rust SDK 的输出

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_header() {
    echo ""
    echo "=============================================="
    echo "$1"
    echo "=============================================="
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

# 帮助信息
if [ "$1" == "-h" ] || [ "$1" == "--help" ] || [ $# -lt 2 ]; then
    echo "Greenfield SDK Debug Compare Tool"
    echo ""
    echo "Usage:"
    echo "  $0 <go_output_file> <rust_output_file>"
    echo ""
    echo "Example:"
    echo "  $0 go_debug_output.txt rust_debug_output.txt"
    echo ""
    echo "Output files should contain debug output from:"
    echo "  - Go: go run main.go <command>"
    echo "  - Rust: RUST_LOG=debug cargo run -- <command>"
    exit 0
fi

GO_FILE=$1
RUST_FILE=$2

if [ ! -f "$GO_FILE" ]; then
    print_error "Go output file not found: $GO_FILE"
    exit 1
fi

if [ ! -f "$RUST_FILE" ]; then
    print_error "Rust output file not found: $RUST_FILE"
    exit 1
fi

print_header "Comparing Go SDK vs Rust SDK Output"

# 提取并对比 Domain Separator
print_header "Domain Separator"
GO_DS=$(grep -i "domain.separator" "$GO_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")
RUST_DS=$(grep -i "domain.separator" "$RUST_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")

echo "Go:   $GO_DS"
echo "Rust: $RUST_DS"
if [ "$GO_DS" == "$RUST_DS" ] && [ "$GO_DS" != "NOT_FOUND" ]; then
    print_success "Domain Separator matches!"
elif [ "$GO_DS" == "NOT_FOUND" ] || [ "$RUST_DS" == "NOT_FOUND" ]; then
    print_warning "Domain Separator not found in one or both outputs"
else
    print_error "Domain Separator MISMATCH!"
fi

# 提取并对比 Type Hash
print_header "Type Hash"
GO_TH=$(grep -i "type.hash" "$GO_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")
RUST_TH=$(grep -i "type.hash" "$RUST_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")

echo "Go:   $GO_TH"
echo "Rust: $RUST_TH"
if [ "$GO_TH" == "$RUST_TH" ] && [ "$GO_TH" != "NOT_FOUND" ]; then
    print_success "Type Hash matches!"
elif [ "$GO_TH" == "NOT_FOUND" ] || [ "$RUST_TH" == "NOT_FOUND" ]; then
    print_warning "Type Hash not found in one or both outputs"
else
    print_error "Type Hash MISMATCH!"
fi

# 提取并对比 Struct Hash
print_header "Struct Hash"
GO_SH=$(grep -i "struct.hash" "$GO_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")
RUST_SH=$(grep -i "struct.hash" "$RUST_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")

echo "Go:   $GO_SH"
echo "Rust: $RUST_SH"
if [ "$GO_SH" == "$RUST_SH" ] && [ "$GO_SH" != "NOT_FOUND" ]; then
    print_success "Struct Hash matches!"
elif [ "$GO_SH" == "NOT_FOUND" ] || [ "$RUST_SH" == "NOT_FOUND" ]; then
    print_warning "Struct Hash not found in one or both outputs"
else
    print_error "Struct Hash MISMATCH!"
fi

# 提取并对比 Final Hash
print_header "Final EIP-712 Hash"
GO_FH=$(grep -i "final.*hash\|eip712.*hash" "$GO_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")
RUST_FH=$(grep -i "final.*hash\|eip712.*hash" "$RUST_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")

echo "Go:   $GO_FH"
echo "Rust: $RUST_FH"
if [ "$GO_FH" == "$RUST_FH" ] && [ "$GO_FH" != "NOT_FOUND" ]; then
    print_success "Final Hash matches!"
elif [ "$GO_FH" == "NOT_FOUND" ] || [ "$RUST_FH" == "NOT_FOUND" ]; then
    print_warning "Final Hash not found in one or both outputs"
else
    print_error "Final Hash MISMATCH!"
fi

# 提取并对比 Canonical Request (for SP requests)
print_header "Canonical Request Message to Sign"
GO_MSG=$(grep -i "message.to.sign" "$GO_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")
RUST_MSG=$(grep -i "message.to.sign" "$RUST_FILE" | grep -oE "0x[a-fA-F0-9]{64}" | head -1 || echo "NOT_FOUND")

if [ "$GO_MSG" != "NOT_FOUND" ] || [ "$RUST_MSG" != "NOT_FOUND" ]; then
    echo "Go:   $GO_MSG"
    echo "Rust: $RUST_MSG"
    if [ "$GO_MSG" == "$RUST_MSG" ] && [ "$GO_MSG" != "NOT_FOUND" ]; then
        print_success "Message to Sign matches!"
    elif [ "$GO_MSG" == "NOT_FOUND" ] || [ "$RUST_MSG" == "NOT_FOUND" ]; then
        print_warning "Message to Sign not found in one or both outputs"
    else
        print_error "Message to Sign MISMATCH!"
    fi
else
    echo "(Not applicable - no Canonical Request found)"
fi

# 总结
print_header "Summary"
echo "If any values don't match, check the following:"
echo ""
echo "1. Domain Separator mismatch:"
echo "   - Check chainId encoding (should be uint256, left-padded to 32 bytes)"
echo "   - Verify domain parameters: name, version, salt, verifyingContract"
echo ""
echo "2. Type Hash mismatch:"
echo "   - Check field ordering (must be alphabetical)"
echo "   - Verify type string format (spaces, commas)"
echo "   - Check nested type names (e.g., TypeMsg1PrimarySpApproval)"
echo ""
echo "3. Struct Hash mismatch:"
echo "   - Check field encoding (strings -> keccak256, numbers -> left-padded)"
echo "   - Verify nested struct hashes"
echo "   - For bytes[]: Base64 encode first, then keccak256 the ASCII bytes"
echo ""
echo "4. Canonical Request mismatch (SP requests):"
echo "   - Check header ordering (alphabetical, lowercase)"
echo "   - Verify newline count between sections"
echo "   - Check host format (with/without port)"

