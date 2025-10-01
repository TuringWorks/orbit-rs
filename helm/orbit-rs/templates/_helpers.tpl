{{/*
Expand the name of the chart.
*/}}
{{- define "orbit-rs.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "orbit-rs.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "orbit-rs.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "orbit-rs.labels" -}}
helm.sh/chart: {{ include "orbit-rs.chart" . }}
{{ include "orbit-rs.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "orbit-rs.selectorLabels" -}}
app.kubernetes.io/name: {{ include "orbit-rs.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "orbit-rs.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "orbit-rs.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the image name
*/}}
{{- define "orbit-rs.image" -}}
{{- $registry := .Values.global.imageRegistry | default "" }}
{{- $repository := .Values.orbitServer.image.repository }}
{{- $tag := .Values.orbitServer.image.tag | default .Chart.AppVersion }}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Create the storage class name
*/}}
{{- define "orbit-rs.storageClassName" -}}
{{- if .Values.persistence.storageClass }}
{{- if eq "-" .Values.persistence.storageClass }}
{{- printf "storageClassName: \"\"" }}
{{- else }}
{{- printf "storageClassName: %s" .Values.persistence.storageClass }}
{{- end }}
{{- else if .Values.global.storageClass }}
{{- printf "storageClassName: %s" .Values.global.storageClass }}
{{- end }}
{{- end }}

{{/*
Generate configuration TOML
*/}}
{{- define "orbit-rs.config" -}}
# Orbit-RS Server Configuration

[server]
bind_address = "{{ .Values.config.server.bindAddress }}"
health_bind_address = "{{ .Values.config.server.healthBindAddress }}"
metrics_bind_address = "{{ .Values.config.server.metricsBindAddress }}"

[node]
id = "${POD_NAME}"
namespace = "${POD_NAMESPACE}"

[cluster]
discovery_mode = "{{ .Values.config.cluster.discoveryMode }}"
service_name = "{{ include "orbit-rs.fullname" . }}"
service_namespace = "{{ .Release.Namespace }}"
lease_duration_seconds = {{ .Values.config.cluster.leaseDurationSeconds }}
lease_renew_interval_seconds = {{ .Values.config.cluster.leaseRenewIntervalSeconds }}

[transactions]
database_path = "{{ .Values.config.transactions.databasePath }}"
max_connections = {{ .Values.config.transactions.maxConnections }}
enable_wal = {{ .Values.config.transactions.enableWal }}
recovery_timeout_seconds = {{ .Values.config.transactions.recoveryTimeoutSeconds }}
max_recovery_attempts = {{ .Values.config.transactions.maxRecoveryAttempts }}

[saga]
max_execution_time_seconds = {{ .Values.config.saga.maxExecutionTimeSeconds }}
default_step_timeout_seconds = {{ .Values.config.saga.defaultStepTimeoutSeconds }}
max_retry_attempts = {{ .Values.config.saga.maxRetryAttempts }}
enable_automatic_failover = {{ .Values.config.saga.enableAutomaticFailover }}

[logging]
level = "{{ .Values.config.logging.level }}"
format = "{{ .Values.config.logging.format }}"

[performance]
worker_threads = {{ .Values.config.performance.workerThreads }}
max_blocking_threads = {{ .Values.config.performance.maxBlockingThreads }}
thread_stack_size = {{ .Values.config.performance.threadStackSize }}

[tls]
enabled = {{ .Values.config.tls.enabled }}
{{- if .Values.config.tls.certFile }}
cert_file = "{{ .Values.config.tls.certFile }}"
{{- end }}
{{- if .Values.config.tls.keyFile }}
key_file = "{{ .Values.config.tls.keyFile }}"
{{- end }}
{{- if .Values.config.tls.caFile }}
ca_file = "{{ .Values.config.tls.caFile }}"
{{- end }}
{{- end }}

{{/*
Generate entrypoint script
*/}}
{{- define "orbit-rs.entrypoint" -}}
#!/bin/bash
set -e

# Set environment variables for configuration substitution
export POD_NAME=${HOSTNAME}
export POD_NAMESPACE=${POD_NAMESPACE:-{{ .Release.Namespace }}}
export NODE_ID="${POD_NAME}.${POD_NAMESPACE}"

# Create data directory if it doesn't exist
mkdir -p /app/data

# Substitute environment variables in config
envsubst < /app/config/orbit-server.toml > /tmp/orbit-server.toml

# Start the server
exec /app/orbit-server --config /tmp/orbit-server.toml "$@"
{{- end }}

{{/*
Generate health check script
*/}}
{{- define "orbit-rs.healthcheck" -}}
#!/bin/bash
# Health check script
curl -f http://localhost:{{ .Values.orbitServer.service.healthPort }}/health || exit 1
{{- end }}