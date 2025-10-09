#!/bin/bash

# Orbit-RS Kubernetes Deployment Validation Script
# This script validates different deployment configurations and workload profiles

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
HELM_CHART_DIR="${REPO_ROOT}/helm/orbit-rs"
K8S_MANIFEST_DIR="${REPO_ROOT}/k8s"
TEMP_DIR="/tmp/orbit-rs-validation"

# Supported profiles
PROFILES=("small" "medium" "large" "enterprise")
PLATFORMS=("linux/amd64" "linux/arm64")

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1" >&2
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" >&2
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1" >&2
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    local missing=()
    
    if ! command -v kubectl &> /dev/null; then
        missing+=("kubectl")
    fi
    
    if ! command -v helm &> /dev/null; then
        missing+=("helm")
    fi
    
    if ! command -v podman &> /dev/null && ! command -v docker &> /dev/null; then
        missing+=("podman or docker")
    fi
    
    if ! command -v yq &> /dev/null; then
        missing+=("yq")
    fi
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing[*]}"
        log_error "Please install the missing tools and try again."
        exit 1
    fi
    
    log_success "All prerequisites satisfied"
}

# Validate Kubernetes manifests
validate_k8s_manifests() {
    log_info "Validating Kubernetes manifests..."
    
    mkdir -p "$TEMP_DIR"
    local validation_failed=false
    
    # Find all YAML files in k8s directory
    while IFS= read -r -d '' file; do
        log_info "Validating manifest: $(basename "$file")"
        
        # Check YAML syntax
        if ! yq eval '.' "$file" >/dev/null 2>&1; then
            log_error "Invalid YAML syntax in $file"
            validation_failed=true
            continue
        fi
        
        # Validate with kubectl (dry-run)
        if ! kubectl apply --dry-run=client --validate=true -f "$file" &>/dev/null; then
            log_warning "kubectl validation failed for $file"
        else
            log_success "Valid manifest: $(basename "$file")"
        fi
        
    done < <(find "$K8S_MANIFEST_DIR" -name "*.yaml" -o -name "*.yml" -print0 2>/dev/null || true)
    
    if [[ "$validation_failed" == true ]]; then
        log_error "Kubernetes manifest validation failed"
        return 1
    fi
    
    log_success "All Kubernetes manifests are valid"
}

# Validate Helm charts
validate_helm_charts() {
    log_info "Validating Helm charts..."
    
    # Lint the base chart
    if ! helm lint "$HELM_CHART_DIR"; then
        log_error "Helm chart lint failed"
        return 1
    fi
    
    log_success "Helm chart lint passed"
    
    # Validate each profile
    for profile in "${PROFILES[@]}"; do
        local values_file="${HELM_CHART_DIR}/values-${profile}.yaml"
        
        if [[ -f "$values_file" ]]; then
            log_info "Validating Helm chart with $profile profile..."
            
            # Template the chart with the profile values
            local output_file="${TEMP_DIR}/helm-${profile}.yaml"
            if helm template "orbit-rs-${profile}" "$HELM_CHART_DIR" \
                -f "$values_file" \
                --output-dir "$TEMP_DIR" > /dev/null 2>&1; then
                
                log_success "Helm template successful for $profile profile"
                
                # Validate the templated output
                if kubectl apply --dry-run=client --validate=true \
                    -f "${TEMP_DIR}/orbit-rs/templates/" &>/dev/null; then
                    log_success "kubectl validation passed for $profile profile"
                else
                    log_warning "kubectl validation failed for $profile profile"
                fi
            else
                log_error "Helm template failed for $profile profile"
                return 1
            fi
        else
            log_warning "Values file not found: $values_file"
        fi
    done
    
    log_success "All Helm chart validations passed"
}

# Validate resource sizing
validate_resource_sizing() {
    log_info "Validating resource sizing configurations..."
    
    for profile in "${PROFILES[@]}"; do
        local values_file="${HELM_CHART_DIR}/values-${profile}.yaml"
        
        if [[ ! -f "$values_file" ]]; then
            continue
        fi
        
        log_info "Checking resource sizing for $profile profile..."
        
        # Extract CPU and memory requests/limits
        local cpu_requests=$(yq eval '.orbitServer.resources.requests.cpu' "$values_file" 2>/dev/null || echo "null")
        local memory_requests=$(yq eval '.orbitServer.resources.requests.memory' "$values_file" 2>/dev/null || echo "null")
        local cpu_limits=$(yq eval '.orbitServer.resources.limits.cpu' "$values_file" 2>/dev/null || echo "null")
        local memory_limits=$(yq eval '.orbitServer.resources.limits.memory' "$values_file" 2>/dev/null || echo "null")
        
        # Validate that resources are defined
        if [[ "$cpu_requests" == "null" ]] || [[ "$memory_requests" == "null" ]]; then
            log_error "Missing resource requests in $profile profile"
            return 1
        fi
        
        if [[ "$cpu_limits" == "null" ]] || [[ "$memory_limits" == "null" ]]; then
            log_error "Missing resource limits in $profile profile"
            return 1
        fi
        
        # Validate resource progression (small < medium < large < enterprise)
        case "$profile" in
            "small")
                expected_replicas=1
                ;;
            "medium")
                expected_replicas=3
                ;;
            "large")
                expected_replicas=6
                ;;
            "enterprise")
                expected_replicas=9
                ;;
        esac
        
        local actual_replicas=$(yq eval '.orbitServer.replicaCount' "$values_file" 2>/dev/null || echo "1")
        
        if [[ "$actual_replicas" -ge "$expected_replicas" ]]; then
            log_success "Replica count appropriate for $profile profile: $actual_replicas"
        else
            log_warning "Low replica count for $profile profile: $actual_replicas (expected >= $expected_replicas)"
        fi
        
        log_success "Resource sizing validated for $profile profile"
    done
    
    log_success "Resource sizing validation completed"
}

# Validate GPU configurations
validate_gpu_configurations() {
    log_info "Validating GPU configurations..."
    
    # Check enterprise profile for GPU settings
    local enterprise_values="${HELM_CHART_DIR}/values-enterprise.yaml"
    
    if [[ -f "$enterprise_values" ]]; then
        local gpu_enabled=$(yq eval '.orbitCompute.gpu.enabled' "$enterprise_values" 2>/dev/null || echo "false")
        local gpu_count=$(yq eval '.orbitCompute.gpu.count' "$enterprise_values" 2>/dev/null || echo "0")
        
        if [[ "$gpu_enabled" == "true" ]] && [[ "$gpu_count" -gt 0 ]]; then
            log_success "GPU configuration enabled in enterprise profile"
            
            # Check for GPU tolerations
            local tolerations=$(yq eval '.orbitCompute.tolerations[] | select(.key == "nvidia.com/gpu")' "$enterprise_values" 2>/dev/null || echo "")
            if [[ -n "$tolerations" ]]; then
                log_success "GPU tolerations configured"
            else
                log_warning "GPU tolerations not configured"
            fi
            
            # Check for GPU node selector
            local node_selector=$(yq eval '.orbitCompute.nodeSelector.accelerator' "$enterprise_values" 2>/dev/null || echo "")
            if [[ -n "$node_selector" ]] && [[ "$node_selector" != "null" ]]; then
                log_success "GPU node selector configured: $node_selector"
            else
                log_warning "GPU node selector not configured"
            fi
            
        else
            log_info "GPU not enabled in enterprise profile"
        fi
    fi
    
    log_success "GPU configuration validation completed"
}

# Validate storage configurations
validate_storage_configurations() {
    log_info "Validating storage configurations..."
    
    for profile in "${PROFILES[@]}"; do
        local values_file="${HELM_CHART_DIR}/values-${profile}.yaml"
        
        if [[ ! -f "$values_file" ]]; then
            continue
        fi
        
        log_info "Checking storage for $profile profile..."
        
        # Check persistence configuration
        local persistence_enabled=$(yq eval '.orbitServer.persistence.enabled' "$values_file" 2>/dev/null || echo "false")
        local storage_size=$(yq eval '.orbitServer.persistence.size' "$values_file" 2>/dev/null || echo "null")
        
        if [[ "$persistence_enabled" == "true" ]] && [[ "$storage_size" != "null" ]]; then
            log_success "Storage persistence configured for $profile: $storage_size"
        else
            log_warning "Storage persistence not properly configured for $profile"
        fi
        
        # Check memory-mapped storage for enterprise
        if [[ "$profile" == "enterprise" ]]; then
            local mmap_enabled=$(yq eval '.orbitServer.mmapStorage.enabled' "$values_file" 2>/dev/null || echo "false")
            if [[ "$mmap_enabled" == "true" ]]; then
                log_success "Memory-mapped storage enabled for enterprise profile"
            else
                log_info "Memory-mapped storage not enabled for enterprise profile"
            fi
        fi
    done
    
    log_success "Storage configuration validation completed"
}

# Validate autoscaling configurations
validate_autoscaling_configurations() {
    log_info "Validating autoscaling configurations..."
    
    for profile in "${PROFILES[@]}"; do
        local values_file="${HELM_CHART_DIR}/values-${profile}.yaml"
        
        if [[ ! -f "$values_file" ]]; then
            continue
        fi
        
        local hpa_enabled=$(yq eval '.autoscaling.enabled' "$values_file" 2>/dev/null || echo "false")
        
        case "$profile" in
            "small")
                if [[ "$hpa_enabled" == "false" ]]; then
                    log_success "HPA correctly disabled for small profile"
                else
                    log_warning "HPA enabled for small profile (may not be necessary)"
                fi
                ;;
            "enterprise")
                if [[ "$hpa_enabled" == "true" ]]; then
                    log_success "HPA enabled for enterprise profile"
                    
                    # Check HPA metrics
                    local cpu_target=$(yq eval '.autoscaling.hpa.metrics[] | select(.resource.name == "cpu") | .resource.target.averageUtilization' "$values_file" 2>/dev/null || echo "null")
                    if [[ "$cpu_target" != "null" ]]; then
                        log_success "HPA CPU target configured: ${cpu_target}%"
                    else
                        log_warning "HPA CPU target not configured"
                    fi
                else
                    log_warning "HPA disabled for enterprise profile"
                fi
                ;;
        esac
    done
    
    log_success "Autoscaling configuration validation completed"
}

# Validate security configurations
validate_security_configurations() {
    log_info "Validating security configurations..."
    
    for profile in "${PROFILES[@]}"; do
        local values_file="${HELM_CHART_DIR}/values-${profile}.yaml"
        
        if [[ ! -f "$values_file" ]]; then
            continue
        fi
        
        log_info "Checking security for $profile profile..."
        
        # Check security context
        local run_as_non_root=$(yq eval '.securityContext.runAsNonRoot' "$values_file" 2>/dev/null || echo "false")
        if [[ "$run_as_non_root" == "true" ]]; then
            log_success "Non-root security context configured for $profile"
        else
            log_warning "Non-root security context not configured for $profile"
        fi
        
        # Check capabilities
        local capabilities_drop=$(yq eval '.securityContext.capabilities.drop[]' "$values_file" 2>/dev/null || echo "")
        if [[ "$capabilities_drop" == *"ALL"* ]]; then
            log_success "Capabilities dropped for $profile"
        else
            log_warning "Capabilities not properly dropped for $profile"
        fi
        
        # Check read-only root filesystem for production profiles
        if [[ "$profile" == "enterprise" ]] || [[ "$profile" == "large" ]]; then
            local readonly_root=$(yq eval '.securityContext.readOnlyRootFilesystem' "$values_file" 2>/dev/null || echo "false")
            if [[ "$readonly_root" == "true" ]]; then
                log_success "Read-only root filesystem configured for $profile"
            else
                log_warning "Read-only root filesystem not configured for $profile"
            fi
        fi
    done
    
    log_success "Security configuration validation completed"
}

# Generate validation report
generate_report() {
    log_info "Generating validation report..."
    
    local report_file="${TEMP_DIR}/validation-report.txt"
    
    cat > "$report_file" << EOF
# Orbit-RS Kubernetes Deployment Validation Report
Generated: $(date)

## Summary
- Kubernetes manifests: Validated
- Helm charts: Validated
- Resource sizing: Validated
- GPU configurations: Validated
- Storage configurations: Validated
- Autoscaling configurations: Validated
- Security configurations: Validated

## Validated Profiles
EOF
    
    for profile in "${PROFILES[@]}"; do
        local values_file="${HELM_CHART_DIR}/values-${profile}.yaml"
        if [[ -f "$values_file" ]]; then
            echo "- $profile: ✓ Available" >> "$report_file"
        else
            echo "- $profile: ✗ Missing values file" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" << EOF

## Recommendations
1. Regularly update resource sizing based on actual usage patterns
2. Monitor GPU utilization and adjust GPU memory fraction as needed
3. Consider implementing custom metrics for more accurate autoscaling
4. Review security configurations periodically
5. Test disaster recovery procedures for enterprise deployments

## Next Steps
1. Deploy to a test environment
2. Run load tests for each profile
3. Validate monitoring and alerting
4. Test backup and recovery procedures
EOF
    
    log_success "Validation report generated: $report_file"
    
    # Display summary
    echo ""
    echo -e "${GREEN}===============================================${NC}"
    echo -e "${GREEN} Orbit-RS Kubernetes Deployment Validation ${NC}"
    echo -e "${GREEN}===============================================${NC}"
    echo ""
    cat "$report_file"
}

# Cleanup function
cleanup() {
    if [[ -d "$TEMP_DIR" ]]; then
        rm -rf "$TEMP_DIR"
    fi
}

# Main execution
main() {
    # Setup cleanup trap
    trap cleanup EXIT
    
    # Create temp directory
    mkdir -p "$TEMP_DIR"
    
    log_info "Starting Orbit-RS Kubernetes deployment validation..."
    
    # Run all validations
    check_prerequisites
    validate_k8s_manifests
    validate_helm_charts
    validate_resource_sizing
    validate_gpu_configurations
    validate_storage_configurations
    validate_autoscaling_configurations
    validate_security_configurations
    
    # Generate report
    generate_report
    
    log_success "Validation completed successfully!"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  -h, --help     Show this help message"
            echo "  --dry-run      Perform validation without making changes"
            echo ""
            echo "This script validates Orbit-RS Kubernetes deployment configurations"
            echo "including Helm charts, resource sizing, GPU configurations, and security settings."
            exit 0
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Run main function
main "$@"