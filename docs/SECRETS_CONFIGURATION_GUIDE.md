---
layout: default
title: GitHub Secrets Configuration Guide
category: documentation
---

## GitHub Secrets Configuration Guide

**Repository:** orbit-rs  
**Date:** 2025-10-03  
**Purpose:** Configure secrets for CI/CD pipeline deployment

---

## Overview

The CI/CD pipeline (`.github/workflows/ci-cd.yml`) requires secrets to be configured in GitHub repository settings for Kubernetes deployments. This guide walks you through the setup process.

## Required Secrets

### 1. KUBE_CONFIG_STAGING

**Purpose:** Kubernetes configuration for staging environment  
**Used by:** `deploy-staging` job (runs on `develop` branch)  
**Format:** Base64-encoded kubeconfig file

### 2. KUBE_CONFIG_PRODUCTION  

**Purpose:** Kubernetes configuration for production environment  
**Used by:** `deploy-production` job (runs on `v*` tags)  
**Format:** Base64-encoded kubeconfig file

### 3. DOCKER_USERNAME (Optional)

**Purpose:** Docker Hub authentication for pushing images  
**Used by:** `docker-build` job  
**Format:** Docker Hub username string

### 4. DOCKER_PASSWORD (Optional)

**Purpose:** Docker Hub authentication token  
**Used by:** `docker-build` job  
**Format:** Docker Hub access token or password

---

## Step-by-Step Configuration

### Step 1: Access Repository Settings

1. Navigate to your GitHub repository: `https://github.com/TuringWorks/orbit-rs`
2. Click on **Settings** tab (requires admin access)
3. In the left sidebar, navigate to **Secrets and variables** → **Actions**

### Step 2: Prepare Kubernetes Configurations

#### For Staging Environment

1. **Get your kubeconfig file:**

   ```bash
   # If using kubectl, your config is typically at:
   cat ~/.kube/config
   
   # Or get config for specific cluster:
   kubectl config view --minify --flatten > staging-kubeconfig.yaml
   ```

2. **Encode the kubeconfig:**

   ```bash
   # macOS/Linux:
   base64 < staging-kubeconfig.yaml | tr -d '\n' > staging-kubeconfig-base64.txt
   
   # Or in one command:
   cat staging-kubeconfig.yaml | base64 | tr -d '\n'
   ```

3. **Copy the base64 output** (it will be a long string without line breaks)

#### For Production Environment

Repeat the same process for your production cluster:

```bash
kubectl config view --minify --flatten --context=production > production-kubeconfig.yaml
base64 < production-kubeconfig.yaml | tr -d '\n' > production-kubeconfig-base64.txt
```

### Step 3: Add Secrets to GitHub

#### Add KUBE_CONFIG_STAGING

1. In GitHub repository settings → Secrets and variables → Actions
2. Click **New repository secret**
3. Name: `KUBE_CONFIG_STAGING`
4. Secret: Paste the base64-encoded staging kubeconfig
5. Click **Add secret**

#### Add KUBE_CONFIG_PRODUCTION

1. Click **New repository secret** again
2. Name: `KUBE_CONFIG_PRODUCTION`
3. Secret: Paste the base64-encoded production kubeconfig
4. Click **Add secret**

#### Add Docker Hub Credentials (Optional)

If you want to push images to Docker Hub:

1. **Get Docker Hub Access Token:**
   - Go to <https://hub.docker.com/settings/security>
   - Click **New Access Token**
   - Name it `orbit-rs-ci-cd`
   - Copy the token (you won't be able to see it again)

2. **Add DOCKER_USERNAME:**
   - New repository secret
   - Name: `DOCKER_USERNAME`
   - Secret: Your Docker Hub username
   - Add secret

3. **Add DOCKER_PASSWORD:**
   - New repository secret
   - Name: `DOCKER_PASSWORD`
   - Secret: Your Docker Hub access token
   - Add secret

### Step 4: Configure Environment Protection Rules (Recommended)

For additional security, set up environment protection:

1. In repository Settings → Environments
2. Click **New environment** (or select existing)
3. Name: `staging` (matches workflow environment name)
4. Configure protection rules:
   -  **Required reviewers** (optional): Add reviewers who must approve deployments
   -  **Wait timer** (optional): Add delay before deployment
   -  **Deployment branches**: Limit to `develop` branch only

5. Repeat for `production` environment:
   - Name: `production`
   - Restrict to tags matching pattern: `v*`
   - Add required reviewers (highly recommended for production)

---

## Alternative: Using Kubernetes Service Account (Recommended for Production)

Instead of using your personal kubeconfig, create a dedicated service account:

### Create Service Account for Staging

```bash

# Create namespace
kubectl create namespace orbit-rs-staging

# Create service account
kubectl create serviceaccount orbit-deployer -n orbit-rs-staging

# Create role with deployment permissions
cat <<EOF | kubectl apply -f -
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: orbit-deployer-role
  namespace: orbit-rs-staging
rules:
- apiGroups: ["", "apps", "batch"]
  resources: ["deployments", "services", "pods", "configmaps", "secrets", "statefulsets"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
- apiGroups: ["networking.k8s.io"]
  resources: ["ingresses"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
EOF

# Bind role to service account
kubectl create rolebinding orbit-deployer-binding \
  --role=orbit-deployer-role \
  --serviceaccount=orbit-rs-staging:orbit-deployer \
  -n orbit-rs-staging

# Create service account token
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Secret
metadata:
  name: orbit-deployer-token
  namespace: orbit-rs-staging
  annotations:
    kubernetes.io/service-account.name: orbit-deployer
type: kubernetes.io/service-account-token
EOF

# Get the token
kubectl get secret orbit-deployer-token -n orbit-rs-staging -o jsonpath='{.data.token}' | base64 -d
```

### Generate Kubeconfig from Service Account

```bash

# Get cluster info
CLUSTER_NAME=$(kubectl config view -o jsonpath='{.contexts[?(@.name == "'$(kubectl config current-context)'")].context.cluster}')
CLUSTER_SERVER=$(kubectl config view -o jsonpath='{.clusters[?(@.name == "'${CLUSTER_NAME}'")].cluster.server}')
CLUSTER_CA=$(kubectl config view --flatten -o jsonpath='{.clusters[?(@.name == "'${CLUSTER_NAME}'")].cluster.certificate-authority-data}')

# Get service account token
SA_TOKEN=$(kubectl get secret orbit-deployer-token -n orbit-rs-staging -o jsonpath='{.data.token}' | base64 -d)

# Create kubeconfig
cat > staging-service-account-kubeconfig.yaml <<EOF
apiVersion: v1
kind: Config
clusters:
- name: ${CLUSTER_NAME}
  cluster:
    certificate-authority-data: ${CLUSTER_CA}
    server: ${CLUSTER_SERVER}
contexts:
- name: orbit-deployer@${CLUSTER_NAME}
  context:
    cluster: ${CLUSTER_NAME}
    namespace: orbit-rs-staging
    user: orbit-deployer
current-context: orbit-deployer@${CLUSTER_NAME}
users:
- name: orbit-deployer
  user:
    token: ${SA_TOKEN}
EOF

# Encode for GitHub secret
base64 < staging-service-account-kubeconfig.yaml | tr -d '\n'
```

Repeat the same process for production with namespace `orbit-rs` or `orbit-rs-production`.

---

## Verification

### Test the Secrets

After adding secrets, verify they work:

1. **Manual workflow run:**
   - Go to Actions tab
   - Select "CI/CD Pipeline"
   - Click "Run workflow"
   - Select branch/tag
   - Run workflow

2. **Check deployment job:**
   - The deployment job should now run successfully
   - Verify kubectl can connect to your cluster
   - Check if Helm deployment succeeds

### Common Issues

#### Issue: "error: You must be logged in to the server (Unauthorized)"

**Solution:**

- Token expired - regenerate service account token
- Wrong token format - ensure base64 encoding is correct
- Insufficient permissions - verify RBAC role has needed permissions

#### Issue: "Secret KUBE_CONFIG_STAGING is not set"

**Solution:**

- Ensure secret name matches exactly (case-sensitive)
- Secret must be in the repository where workflow runs
- Organization secrets need proper access configuration

#### Issue: Deployment fails with "connection refused"

**Solution:**

- Verify cluster server URL is accessible from GitHub Actions
- Check if cluster requires VPN or allowlisting GitHub IPs
- Consider using GitHub Actions self-hosted runners within your network

---

## Security Best Practices

### 1. Use Service Accounts

 Create dedicated service accounts for CI/CD  
 Grant minimal required permissions (principle of least privilege)  
 Rotate tokens regularly (every 90 days recommended)

### 2. Separate Environments

 Use different secrets for staging and production  
 Use different namespaces or clusters  
 Never use production credentials in staging

### 3. Audit Access

 Enable audit logging in Kubernetes  
 Monitor secret access in GitHub (Settings → Security log)  
 Review who has access to secrets regularly

### 4. Environment Protection

 Require manual approval for production deployments  
 Limit deployment branches/tags  
 Add wait timers for production changes

### 5. Secret Rotation

 Set expiration dates for service account tokens  
 Document rotation schedule  
 Automate rotation where possible

---

## Secret Management Alternatives

### Option 1: GitHub Environments with Environment Secrets

Instead of repository secrets, use environment-specific secrets:

1. Settings → Environments → Create environment
2. Add secrets to specific environment
3. Workflow automatically uses environment secrets when deploying to that environment

**Benefits:**

- Automatic secret scoping by environment
- Better access control
- Clear separation between staging/production

### Option 2: External Secret Management

For larger organizations, consider:

- **HashiCorp Vault**: Enterprise secret management
- **AWS Secrets Manager**: If using AWS
- **Azure Key Vault**: If using Azure
- **Google Secret Manager**: If using GCP

These can be integrated with GitHub Actions using official actions or custom scripts.

### Option 3: Self-Hosted Runners

Deploy runners in your private network:

**Benefits:**

- Direct cluster access without exposing credentials
- Better security for sensitive environments
- Compliance with corporate policies

**Setup:**

1. Settings → Actions → Runners → New self-hosted runner
2. Follow installation instructions for your OS
3. Runner can access Kubernetes cluster directly

---

## Quick Reference Commands

```bash

# View current Kubernetes context
kubectl config current-context

# List available contexts
kubectl config get-contexts

# Switch context
kubectl config use-context <context-name>

# Get kubeconfig for specific context
kubectl config view --minify --flatten --context=<context-name>

# Test connection with specific kubeconfig
KUBECONFIG=./staging-kubeconfig.yaml kubectl get nodes

# Verify service account permissions
kubectl auth can-i create deployments --as=system:serviceaccount:orbit-rs-staging:orbit-deployer -n orbit-rs-staging

# List all secrets in namespace
kubectl get secrets -n orbit-rs-staging

# Decode secret
kubectl get secret <secret-name> -n orbit-rs-staging -o jsonpath='{.data.token}' | base64 -d
```

---

## Support and Troubleshooting

### GitHub Documentation

- [Encrypted Secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
- [Using Environments](https://docs.github.com/en/actions/deployment/targeting-different-environments/using-environments-for-deployment)
- [Security Hardening](https://docs.github.com/en/actions/security-guides/security-hardening-for-github-actions)

### Kubernetes Documentation

- [Service Accounts](https://kubernetes.io/docs/tasks/configure-pod-container/configure-service-account/)
- [RBAC Authorization](https://kubernetes.io/docs/reference/access-authn-authz/rbac/)
- [Managing Service Accounts](https://kubernetes.io/docs/reference/access-authn-authz/service-accounts-admin/)

### Getting Help

1. **Check workflow logs** in Actions tab for detailed error messages
2. **Review audit logs** in repository settings
3. **Test locally** with same kubeconfig before adding to GitHub
4. **Verify cluster connectivity** from GitHub Actions IPs if needed

---

## Checklist

Before running deployments, ensure:

- [ ] Kubernetes clusters are accessible
- [ ] Service accounts created with proper RBAC
- [ ] Kubeconfig files encoded correctly
- [ ] Secrets added to GitHub repository
- [ ] Environment protection rules configured
- [ ] Namespaces exist in clusters
- [ ] Helm charts tested locally
- [ ] Monitoring and alerting set up
- [ ] Rollback procedure documented
- [ ] Team members have necessary access

---

**Last Updated:** 2025-10-03  
**Maintained By:** DevOps Team  
**Review Schedule:** Quarterly

For questions or issues, contact the DevOps team or create an issue in the repository.
