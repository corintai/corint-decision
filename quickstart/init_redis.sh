#!/bin/bash
# CORINT Redis Data Initialization Script
# ============================================================================
# This script loads pre-computed feature values into Redis for E2E testing
# Database configuration is read from config/server.yaml (datasources.redis_features)
# For backward compatibility, also supports reading from repository/configs/datasources/redis_features.yaml
#
# Usage:
#   ./init_redis.sh                    # Use config from server.yaml or datasources yaml
#   ./init_redis.sh -c /path/to/config.yaml  # Use custom config file
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DATA_FILE="${SCRIPT_DIR}/data/redis_features.txt"
CONFIG_FILE="${PROJECT_ROOT}/repository/configs/datasources/redis_features.yaml"

# Parse command line arguments
while getopts "c:" opt; do
    case $opt in
        c) CONFIG_FILE="$OPTARG" ;;
        *) echo "Usage: $0 [-c config_file]" && exit 1 ;;
    esac
done

# Check if redis-cli is available
if ! command -v redis-cli &> /dev/null; then
    echo "Error: redis-cli is not installed or not in PATH"
    exit 1
fi

# Check if data file exists
if [ ! -f "${DATA_FILE}" ]; then
    echo "Error: Data file not found: ${DATA_FILE}"
    exit 1
fi

# Check if config file exists
if [ ! -f "${CONFIG_FILE}" ]; then
    echo "Error: Config file not found: ${CONFIG_FILE}"
    exit 1
fi

echo "=============================================="
echo "CORINT Redis Data Initialization"
echo "=============================================="
echo "Config file: ${CONFIG_FILE}"

# Parse configuration from YAML
if command -v yq &> /dev/null; then
    CONNECTION_STRING=$(yq -r '.connection_string // empty' "${CONFIG_FILE}")
else
    # Fallback to grep/sed for simple YAML parsing
    CONNECTION_STRING=$(grep '^connection_string:' "${CONFIG_FILE}" | sed 's/^connection_string:[[:space:]]*"\?\([^"]*\)"\?/\1/')
fi

if [ -z "${CONNECTION_STRING}" ]; then
    echo "Error: connection_string not found in config file"
    exit 1
fi

# Parse connection string (redis://host:port/db or redis://:password@host:port/db)
# Remove redis:// prefix
CONN="${CONNECTION_STRING#redis://}"

# Extract password if present (format: :password@host:port/db)
if [[ "${CONN}" == *"@"* ]]; then
    AUTH_PART="${CONN%%@*}"
    REDIS_PASSWORD="${AUTH_PART#:}"
    CONN="${CONN#*@}"
else
    REDIS_PASSWORD=""
fi

# Extract host and port
HOST_PORT="${CONN%%/*}"
REDIS_HOST="${HOST_PORT%%:*}"
REDIS_PORT="${HOST_PORT#*:}"

# Extract database number if present
if [[ "${CONN}" == *"/"* ]]; then
    REDIS_DB="${CONN#*/}"
else
    REDIS_DB="0"
fi

# Default port if not specified
REDIS_PORT="${REDIS_PORT:-6379}"

echo "Host: ${REDIS_HOST}"
echo "Port: ${REDIS_PORT}"
echo "Database: ${REDIS_DB}"
echo "Data file: ${DATA_FILE}"
echo ""

# Build redis-cli command
REDIS_CLI="redis-cli -h ${REDIS_HOST} -p ${REDIS_PORT} -n ${REDIS_DB}"
if [ -n "${REDIS_PASSWORD}" ]; then
    REDIS_CLI="${REDIS_CLI} -a ${REDIS_PASSWORD}"
fi

# Test Redis connection
echo "Testing Redis connection..."
if ! ${REDIS_CLI} PING &> /dev/null; then
    echo "Error: Cannot connect to Redis at ${REDIS_HOST}:${REDIS_PORT}"
    exit 1
fi
echo "Connection successful!"
echo ""

# Count commands to execute
CMD_COUNT=$(grep -v '^#' "${DATA_FILE}" | grep -v '^$' | wc -l | tr -d ' ')
echo "Loading ${CMD_COUNT} Redis commands..."
echo ""

# Load data
grep -v '^#' "${DATA_FILE}" | grep -v '^$' | while read -r line; do
    ${REDIS_CLI} ${line} > /dev/null
done

echo "=============================================="
echo "Data loaded successfully!"
echo ""

# Verify some keys
echo "Verification (sample keys):"
echo "  user_features:normal_user_001:risk_score = $(${REDIS_CLI} GET user_features:normal_user_001:risk_score)"
echo "  user_features:vip_user_001:risk_score = $(${REDIS_CLI} GET user_features:vip_user_001:risk_score)"
echo "  user_features:suspicious_001:risk_score = $(${REDIS_CLI} GET user_features:suspicious_001:risk_score)"
echo "  user_profiles:vip_user_001:kyc_status = $(${REDIS_CLI} GET user_profiles:vip_user_001:kyc_status)"
echo "  user_profiles:vip_user_001:is_verified = $(${REDIS_CLI} GET user_profiles:vip_user_001:is_verified)"
echo ""
echo "=============================================="
