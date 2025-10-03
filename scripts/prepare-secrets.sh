#!/bin/bash

# GitHub Secrets Preparation Script for orbit-rs
# This script helps prepare kubeconfig secrets for GitHub Actions
# 
# Usage:
#   ./prepare-secrets.sh staging
#   ./prepare-secrets.sh production
#
# Requirements:
#   - kubectl configured with access to your cluster
#   - base64 command available
#   - Valid kubeconfig context

set -e

ENVIRONMENT="${1:-staging}"
OUTPUT_FILE="${ENVIRONMENT}-kubeconfig-base64.txt"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Orbit-RS GitHub Secrets Preparation ===${NC}"
echo ""
echo "Environment: ${ENVIRONMENT}"
echo ""

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    echo -e "${RED}Error: kubectl is not installed or not in PATH${NC}"
    echo "Please install kubectl first: https://kubernetes.io/docs/tasks/tools/"
    exit 1
fi

# Check if base64 is available
if ! command -v base64 &> /dev/null; then
    echo -e "${RED}Error: base64 command not found${NC}"
    exit 1
fi

# Get current context
CURRENT_CONTEXT=$(kubectl config current-context)
echo -e "${YELLOW}Current kubectl context:${NC} ${CURRENT_CONTEXT}"
echo ""

# Ask user to confirm or select context
echo "Available contexts:"
kubectl config get-contexts
echo ""

read -p "Use current context '${CURRENT_CONTEXT}'? (y/n): " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    read -p "Enter the context name to use: " SELECTED_CONTEXT
    kubectl config use-context "${SELECTED_CONTEXT}"
    CURRENT_CONTEXT="${SELECTED_CONTEXT}"
fi

# Test connection
echo -e "${YELLOW}Testing connection to cluster...${NC}"
if kubectl cluster-info &> /dev/null; then
    echo -e "${GREEN}✓ Successfully connected to cluster${NC}"
else
    echo -e "${RED}✗ Failed to connect to cluster${NC}"
    echo "Please check your kubeconfig and cluster access"
    exit 1
fi

# Ask for namespace
if [ "${ENVIRONMENT}" = "staging" ]; then
    DEFAULT_NAMESPACE="orbit-rs-staging"
else
    DEFAULT_NAMESPACE="orbit-rs"
fi

read -p "Enter namespace for ${ENVIRONMENT} (default: ${DEFAULT_NAMESPACE}): " NAMESPACE
NAMESPACE="${NAMESPACE:-${DEFAULT_NAMESPACE}}"

echo ""
echo -e "${YELLOW}Creating kubeconfig for namespace: ${NAMESPACE}${NC}"

# Option 1: Use current kubeconfig
echo ""
echo "Choose kubeconfig method:"
echo "1. Use current user kubeconfig (simpler, less secure)"
echo "2. Create service account with limited permissions (recommended)"
read -p "Enter choice (1 or 2): " -n 1 -r CHOICE
echo ""

if [ "${CHOICE}" = "2" ]; then
    # Create service account method
    echo -e "${YELLOW}Creating service account in namespace ${NAMESPACE}...${NC}"
    
    # Create namespace if it doesn't exist
    if ! kubectl get namespace "${NAMESPACE}" &> /dev/null; then
        echo "Creating namespace ${NAMESPACE}..."
        kubectl create namespace "${NAMESPACE}"
    fi
    
    SA_NAME="orbit-deployer"
    
    # Create service account
    echo "Creating service account ${SA_NAME}..."
    kubectl create serviceaccount "${SA_NAME}" -n "${NAMESPACE}" --dry-run=client -o yaml | kubectl apply -f -
    
    # Create role
    echo "Creating deployment role..."
    cat <<EOF | kubectl apply -f -
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: orbit-deployer-role
  namespace: ${NAMESPACE}
rules:
- apiGroups: ["", "apps", "batch"]
  resources: ["deployments", "services", "pods", "configmaps", "secrets", "statefulsets", "jobs"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
- apiGroups: ["networking.k8s.io"]
  resources: ["ingresses"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
EOF
    
    # Create role binding
    echo "Creating role binding..."
    kubectl create rolebinding orbit-deployer-binding \
      --role=orbit-deployer-role \
      --serviceaccount="${NAMESPACE}:${SA_NAME}" \
      -n "${NAMESPACE}" \
      --dry-run=client -o yaml | kubectl apply -f -
    
    # Create service account token secret
    echo "Creating service account token..."
    cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Secret
metadata:
  name: orbit-deployer-token
  namespace: ${NAMESPACE}
  annotations:
    kubernetes.io/service-account.name: ${SA_NAME}
type: kubernetes.io/service-account-token
EOF
    
    # Wait for token to be created
    echo "Waiting for token to be populated..."
    for i in {1..10}; do
        TOKEN=$(kubectl get secret orbit-deployer-token -n "${NAMESPACE}" -o jsonpath='{.data.token}' 2>/dev/null || echo "")
        if [ -n "${TOKEN}" ]; then
            break
        fi
        sleep 1
    done
    
    if [ -z "${TOKEN}" ]; then
        echo -e "${RED}Error: Failed to get service account token${NC}"
        exit 1
    fi
    
    # Get cluster info
    CLUSTER_NAME=$(kubectl config view -o jsonpath="{.contexts[?(@.name==\"${CURRENT_CONTEXT}\")].context.cluster}")
    CLUSTER_SERVER=$(kubectl config view -o jsonpath="{.clusters[?(@.name==\"${CLUSTER_NAME}\")].cluster.server}")
    CLUSTER_CA=$(kubectl config view --flatten -o jsonpath="{.clusters[?(@.name==\"${CLUSTER_NAME}\")].cluster.certificate-authority-data}")
    
    # Decode token
    SA_TOKEN=$(echo "${TOKEN}" | base64 -d)
    
    # Create kubeconfig
    TEMP_KUBECONFIG=$(mktemp)
    cat > "${TEMP_KUBECONFIG}" <<EOF
apiVersion: v1
kind: Config
clusters:
- name: ${CLUSTER_NAME}
  cluster:
    certificate-authority-data: ${CLUSTER_CA}
    server: ${CLUSTER_SERVER}
contexts:
- name: ${SA_NAME}@${CLUSTER_NAME}
  context:
    cluster: ${CLUSTER_NAME}
    namespace: ${NAMESPACE}
    user: ${SA_NAME}
current-context: ${SA_NAME}@${CLUSTER_NAME}
users:
- name: ${SA_NAME}
  user:
    token: ${SA_TOKEN}
EOF
    
    echo -e "${GREEN}✓ Service account created successfully${NC}"
    
    # Test the service account
    echo -e "${YELLOW}Testing service account permissions...${NC}"
    if KUBECONFIG="${TEMP_KUBECONFIG}" kubectl get pods -n "${NAMESPACE}" &> /dev/null; then
        echo -e "${GREEN}✓ Service account has valid permissions${NC}"
    else
        echo -e "${YELLOW}⚠ Warning: Could not test permissions (namespace may be empty)${NC}"
    fi
    
    KUBECONFIG_FILE="${TEMP_KUBECONFIG}"
else
    # Use current kubeconfig method
    echo -e "${YELLOW}Extracting current kubeconfig...${NC}"
    TEMP_KUBECONFIG=$(mktemp)
    kubectl config view --minify --flatten > "${TEMP_KUBECONFIG}"
    KUBECONFIG_FILE="${TEMP_KUBECONFIG}"
fi

# Encode kubeconfig to base64
echo ""
echo -e "${YELLOW}Encoding kubeconfig to base64...${NC}"
base64 < "${KUBECONFIG_FILE}" | tr -d '\n' > "${OUTPUT_FILE}"

# Verify encoding
ENCODED_SIZE=$(wc -c < "${OUTPUT_FILE}")
echo -e "${GREEN}✓ Base64 encoding complete${NC}"
echo "  Output file: ${OUTPUT_FILE}"
echo "  Size: ${ENCODED_SIZE} bytes"

# Clean up
rm -f "${TEMP_KUBECONFIG}"

echo ""
echo -e "${GREEN}=== Success! ===${NC}"
echo ""
echo "Next steps:"
echo "1. Copy the contents of ${OUTPUT_FILE}"
echo "   ${YELLOW}cat ${OUTPUT_FILE} | pbcopy${NC}  # macOS"
echo "   ${YELLOW}cat ${OUTPUT_FILE} | xclip -selection clipboard${NC}  # Linux"
echo ""
echo "2. Go to GitHub repository settings:"
echo "   https://github.com/TuringWorks/orbit-rs/settings/secrets/actions"
echo ""
echo "3. Click 'New repository secret'"
echo ""
if [ "${ENVIRONMENT}" = "staging" ]; then
    echo "4. Name: ${YELLOW}KUBE_CONFIG_STAGING${NC}"
else
    echo "4. Name: ${YELLOW}KUBE_CONFIG_PRODUCTION${NC}"
fi
echo ""
echo "5. Paste the base64 string from ${OUTPUT_FILE}"
echo ""
echo "6. Click 'Add secret'"
echo ""
echo -e "${YELLOW}⚠ IMPORTANT:${NC}"
echo "  - Keep the ${OUTPUT_FILE} file secure (it contains cluster credentials)"
echo "  - Delete it after adding to GitHub: ${YELLOW}rm ${OUTPUT_FILE}${NC}"
echo "  - Consider rotating credentials regularly"
echo ""

# Show the first and last 20 characters for verification
CONTENT=$(cat "${OUTPUT_FILE}")
PREVIEW="${CONTENT:0:20}...${CONTENT: -20}"
echo "Preview (first/last 20 chars): ${PREVIEW}"
echo ""
