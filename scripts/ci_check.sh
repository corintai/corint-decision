#!/bin/bash

# CORINT CI Check Script
# 在本地运行与 GitHub Actions 相同的检查，提交前验证代码

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  CORINT CI Check Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Function to print status
print_status() {
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ $1 passed${NC}"
    else
        echo -e "${RED}✗ $1 failed${NC}"
        exit 1
    fi
}

# 1. Code formatting check
echo -e "${YELLOW}[1/5] Checking code formatting...${NC}"
cargo fmt -- --check
print_status "Code formatting"
echo ""

# 2. Clippy lints
echo -e "${YELLOW}[2/5] Running clippy...${NC}"
cargo clippy --all-targets --all-features -- -D warnings
print_status "Clippy"
echo ""

# 3. Run tests
echo -e "${YELLOW}[3/5] Running tests...${NC}"
cargo test --all-features --workspace
print_status "Tests"
echo ""

# 4. Run doc tests
echo -e "${YELLOW}[4/5] Running doc tests...${NC}"
cargo test --doc --all-features --workspace
print_status "Doc tests"
echo ""

# 5. Build release
echo -e "${YELLOW}[5/5] Building release version...${NC}"
cargo build --release --all-features --workspace
print_status "Build"
echo ""

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  All checks passed! ✓${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "You can now safely commit and push your code."
echo -e "GitHub Actions will run these same checks automatically."
