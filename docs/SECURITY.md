---
layout: default
title: Security Policy
category: documentation
---

## Security Policy

## Supported Versions

Orbit-RS follows semantic versioning. We provide security updates for the following versions:

| Version | Supported          | Status      |
| ------- | ------------------ | ----------- |
| 0.2.x   | :white_check_mark: | Current     |
| 0.1.x   | :white_check_mark: | Maintained  |
| < 0.1   | :x:                | Unsupported |

## Security Features

Orbit-RS includes several security features and practices:

### Automated Security Scanning

- **cargo-deny**: Dependency security auditing on every CI/CD run
- **Trivy**: Container image vulnerability scanning
- **SBOM Generation**: Software Bill of Materials for compliance
- **Automated Dependency Updates**: Regular security patch updates

### Runtime Security

- **Memory Safety**: Rust's ownership system prevents common vulnerabilities
- **RBAC Support**: Kubernetes Role-Based Access Control integration
- **Secure Defaults**: Production-ready secure configuration out of the box
- **Network Encryption**: gRPC with TLS support for inter-node communication

### Kubernetes Security

- **Pod Security Standards**: Compliance with Kubernetes security best practices
- **Secret Management**: Secure credential handling via Kubernetes Secrets
- **Network Policies**: Support for network isolation between components
- **Service Mesh Ready**: Compatible with Istio and Linkerd for enhanced security

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please follow these steps:

### Reporting Process

1. **DO NOT** create a public GitHub issue for security vulnerabilities
2. Email security concerns to: [security@turingworks.com](mailto:security@turingworks.com)
3. Include the following information:
   - Description of the vulnerability
   - Steps to reproduce the issue
   - Potential impact assessment
   - Suggested remediation (if any)

### What to Expect

- **Initial Response**: Within 48 hours of your report
- **Status Updates**: Every 5-7 days during investigation
- **Resolution Timeline**:
  - Critical vulnerabilities: 7-14 days
  - High severity: 14-30 days
  - Medium/Low severity: 30-90 days

### After Reporting

- **Accepted Vulnerabilities**:
  - We will work with you to understand and reproduce the issue
  - You will be credited in the security advisory (unless you prefer anonymity)
  - We will coordinate disclosure timeline with you
  - A CVE will be requested for significant vulnerabilities

- **Declined Reports**:
  - We will explain why the report was declined
  - We may provide alternative security recommendations
  - You are welcome to discuss the decision

## Security Best Practices

When deploying Orbit-RS:

1. **Keep Dependencies Updated**: Regularly update to latest stable versions
2. **Enable TLS**: Use encrypted communication for production deployments
3. **Restrict Access**: Use network policies and RBAC to limit access
4. **Monitor Logs**: Enable audit logging and monitor for suspicious activity
5. **Use Secrets Management**: Never hardcode credentials in configuration files
6. **Regular Security Audits**: Run security scans as part of your CI/CD pipeline

## Security Advisories

Security advisories will be published on:

- GitHub Security Advisories page
- Project release notes
- Security mailing list (if you want to be notified, contact us)

## Acknowledgments

We appreciate the security research community and will acknowledge all valid security reports in our security advisories.
