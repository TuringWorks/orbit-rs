use anyhow::Result;
use futures::StreamExt;
use k8s_openapi::api::apps::v1::{StatefulSet, StatefulSetSpec};
use k8s_openapi::api::core::v1::{
    ConfigMap, PersistentVolumeClaimSpec, PodSpec, PodTemplateSpec, Service, ServiceAccount,
    ServiceSpec,
};
use kube::{
    api::{Api, ListParams, Patch, PatchParams, PostParams},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
        watcher::Config,
    },
    Resource, ResourceExt,
};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tracing::{error, info, warn};

use crate::crd::{ClusterCondition, ClusterPhase, OrbitCluster, OrbitClusterStatus};

pub const ORBIT_CLUSTER_FINALIZER: &str = "orbit-cluster.orbit.turingworks.com/finalizer";

#[derive(Clone)]
pub struct ClusterController {
    client: Client,
}

impl ClusterController {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn run(self) {
        let clusters: Api<OrbitCluster> = Api::all(self.client.clone());
        let context = Arc::new(ControllerContext::new(self.client.clone()));

        info!("Starting OrbitCluster controller");

        Controller::new(clusters, Config::default().any_semantic())
            .shutdown_on_signal()
            .run(reconcile_cluster, error_policy, context)
            .filter_map(|x| async move { std::result::Result::ok(x) })
            .for_each(|_| futures::future::ready(()))
            .await;
    }
}

#[derive(Clone)]
struct ControllerContext {
    client: Client,
    recorder: Recorder,
}

impl ControllerContext {
    fn new(client: Client) -> Self {
        let recorder = Recorder::new(
            client.clone(),
            Reporter {
                controller: "orbit-cluster-controller".into(),
                instance: std::env::var("POD_NAME").ok(),
            },
        );

        Self { client, recorder }
    }
}

async fn reconcile_cluster(
    cluster: Arc<OrbitCluster>,
    ctx: Arc<ControllerContext>,
) -> Result<Action> {
    let ns = cluster.namespace().unwrap_or_default();
    let name = cluster.name_any();

    info!("Reconciling OrbitCluster {} in namespace {}", name, ns);

    let clusters: Api<OrbitCluster> = Api::namespaced(ctx.client.clone(), &ns);

    finalizer(&clusters, ORBIT_CLUSTER_FINALIZER, cluster, |event| async {
        match event {
            Finalizer::Apply(cluster) => cluster_reconcile(&cluster, ctx.clone()).await,
            Finalizer::Cleanup(cluster) => cluster_cleanup(&cluster, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| anyhow::anyhow!("Finalizer error: {}", e))
}

async fn cluster_reconcile(cluster: &OrbitCluster, ctx: Arc<ControllerContext>) -> Result<Action> {
    let client = &ctx.client;
    let ns = cluster.namespace().unwrap_or_default();
    let name = cluster.name_any();

    info!("Applying OrbitCluster {}", name);

    // Update status to indicate we're processing
    update_cluster_status(
        cluster,
        &ctx,
        ClusterPhase::Creating,
        "Creating cluster resources",
    )
    .await?;

    // Create ServiceAccount
    create_service_account(client, cluster, &ns, &name).await?;

    // Create ConfigMap
    create_config_map(client, cluster, &ns, &name).await?;

    // Create headless Service for StatefulSet
    create_headless_service(client, cluster, &ns, &name).await?;

    // Create external Service (if needed)
    if cluster.spec.service.service_type != "ClusterIP" {
        create_external_service(client, cluster, &ns, &name).await?;
    }

    // Create StatefulSet
    create_stateful_set(client, cluster, &ns, &name).await?;

    // Update status to running
    update_cluster_status(
        cluster,
        &ctx,
        ClusterPhase::Running,
        "Cluster resources created successfully",
    )
    .await?;

    // Publish successful event
    ctx.recorder
        .publish(Event {
            type_: EventType::Normal,
            reason: "Created".into(),
            note: Some("OrbitCluster resources created successfully".into()),
            action: "Reconciling".into(),
            secondary: None,
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to publish event: {}", e))?;

    Ok(Action::requeue(Duration::from_secs(300)))
}

async fn cluster_cleanup(cluster: &OrbitCluster, ctx: Arc<ControllerContext>) -> Result<Action> {
    let client = &ctx.client;
    let ns = cluster.namespace().unwrap_or_default();
    let name = cluster.name_any();

    info!("Cleaning up OrbitCluster {}", name);

    // Update status to indicate termination
    update_cluster_status(
        cluster,
        &ctx,
        ClusterPhase::Terminating,
        "Terminating cluster resources",
    )
    .await?;

    // Delete StatefulSet
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), &ns);
    let _ = statefulsets.delete(&name, &Default::default()).await;

    // Delete Services
    let services: Api<Service> = Api::namespaced(client.clone(), &ns);
    let _ = services
        .delete(&format!("{}-headless", name), &Default::default())
        .await;
    let _ = services.delete(&name, &Default::default()).await;

    // Delete ConfigMap
    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let _ = configmaps
        .delete(&format!("{}-config", name), &Default::default())
        .await;

    // Delete ServiceAccount
    let service_accounts: Api<ServiceAccount> = Api::namespaced(client.clone(), &ns);
    let _ = service_accounts.delete(&name, &Default::default()).await;

    info!("OrbitCluster {} cleanup completed", name);

    Ok(Action::await_change())
}

async fn create_service_account(
    client: &Client,
    cluster: &OrbitCluster,
    ns: &str,
    name: &str,
) -> Result<()> {
    let service_accounts: Api<ServiceAccount> = Api::namespaced(client.clone(), ns);

    let sa = ServiceAccount {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(ns.to_string()),
            labels: Some(create_labels(cluster)),
            owner_references: Some(vec![cluster.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        ..Default::default()
    };

    let pp = PostParams::default();
    match service_accounts.create(&pp, &sa).await {
        Ok(_) => info!("Created ServiceAccount {}", name),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            info!("ServiceAccount {} already exists", name);
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to create ServiceAccount: {}", e)),
    }

    Ok(())
}

async fn create_config_map(
    client: &Client,
    cluster: &OrbitCluster,
    ns: &str,
    name: &str,
) -> Result<()> {
    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), ns);

    let mut data = BTreeMap::new();

    // Create orbit-server.yaml configuration
    let config = json!({
        "server": {
            "bind_address": format!("0.0.0.0:{}", cluster.spec.service.grpc_port),
            "health_bind_address": format!("0.0.0.0:{}", cluster.spec.service.health_port),
            "metrics_bind_address": format!("0.0.0.0:{}", cluster.spec.service.metrics_port)
        },
        "cluster": {
            "discovery_mode": cluster.spec.cluster.discovery_mode,
            "election_method": cluster.spec.cluster.election_method,
            "lease_duration_seconds": cluster.spec.cluster.lease_duration,
            "lease_renew_interval_seconds": cluster.spec.cluster.lease_renew_interval,
            "enable_raft_fallback": cluster.spec.cluster.enable_raft_fallback
        },
        "transactions": {
            "database_path": cluster.spec.transactions.database_path,
            "max_connections": cluster.spec.transactions.max_connections,
            "enable_wal": cluster.spec.transactions.enable_wal,
            "recovery_timeout_seconds": cluster.spec.transactions.recovery_timeout,
            "max_recovery_attempts": cluster.spec.transactions.max_recovery_attempts
        },
        "logging": {
            "level": cluster.spec.env.get("RUST_LOG").unwrap_or(&"info".to_string())
        }
    });

    data.insert(
        "orbit-server.yaml".to_string(),
        serde_yaml::to_string(&config)?,
    );

    let cm = ConfigMap {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            name: Some(format!("{}-config", name)),
            namespace: Some(ns.to_string()),
            labels: Some(create_labels(cluster)),
            owner_references: Some(vec![cluster.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    };

    let pp = PostParams::default();
    match configmaps.create(&pp, &cm).await {
        Ok(_) => info!("Created ConfigMap {}-config", name),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            // Update existing ConfigMap
            let patch = Patch::Merge(&cm);
            configmaps
                .patch(&format!("{}-config", name), &PatchParams::default(), &patch)
                .await?;
            info!("Updated ConfigMap {}-config", name);
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to create ConfigMap: {}", e)),
    }

    Ok(())
}

async fn create_headless_service(
    client: &Client,
    cluster: &OrbitCluster,
    ns: &str,
    name: &str,
) -> Result<()> {
    let services: Api<Service> = Api::namespaced(client.clone(), ns);

    let service = Service {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            name: Some(format!("{}-headless", name)),
            namespace: Some(ns.to_string()),
            labels: Some(create_labels(cluster)),
            owner_references: Some(vec![cluster.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            cluster_ip: Some("None".to_string()),
            selector: Some(create_selector_labels(cluster)),
            ports: Some(vec![
                k8s_openapi::api::core::v1::ServicePort {
                    name: Some("grpc".to_string()),
                    port: cluster.spec.service.grpc_port as i32,
                    target_port: Some(
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(
                            "grpc".to_string(),
                        ),
                    ),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                },
                k8s_openapi::api::core::v1::ServicePort {
                    name: Some("health".to_string()),
                    port: cluster.spec.service.health_port as i32,
                    target_port: Some(
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(
                            "health".to_string(),
                        ),
                    ),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let pp = PostParams::default();
    match services.create(&pp, &service).await {
        Ok(_) => info!("Created headless Service {}-headless", name),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            info!("Headless Service {}-headless already exists", name);
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to create headless Service: {}", e)),
    }

    Ok(())
}

async fn create_external_service(
    client: &Client,
    cluster: &OrbitCluster,
    ns: &str,
    name: &str,
) -> Result<()> {
    let services: Api<Service> = Api::namespaced(client.clone(), ns);

    let service = Service {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(ns.to_string()),
            labels: Some(create_labels(cluster)),
            annotations: Some(cluster.spec.service.annotations.clone()),
            owner_references: Some(vec![cluster.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            type_: Some(cluster.spec.service.service_type.clone()),
            selector: Some(create_selector_labels(cluster)),
            ports: Some(vec![
                k8s_openapi::api::core::v1::ServicePort {
                    name: Some("grpc".to_string()),
                    port: cluster.spec.service.grpc_port as i32,
                    target_port: Some(
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(
                            "grpc".to_string(),
                        ),
                    ),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                },
                k8s_openapi::api::core::v1::ServicePort {
                    name: Some("metrics".to_string()),
                    port: cluster.spec.service.metrics_port as i32,
                    target_port: Some(
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(
                            "metrics".to_string(),
                        ),
                    ),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let pp = PostParams::default();
    match services.create(&pp, &service).await {
        Ok(_) => info!("Created external Service {}", name),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            info!("External Service {} already exists", name);
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to create external Service: {}", e)),
    }

    Ok(())
}

async fn create_stateful_set(
    client: &Client,
    cluster: &OrbitCluster,
    ns: &str,
    name: &str,
) -> Result<()> {
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), ns);

    // Build environment variables
    let mut env_vars = vec![
        k8s_openapi::api::core::v1::EnvVar {
            name: "POD_NAME".to_string(),
            value_from: Some(k8s_openapi::api::core::v1::EnvVarSource {
                field_ref: Some(k8s_openapi::api::core::v1::ObjectFieldSelector {
                    field_path: "metadata.name".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        k8s_openapi::api::core::v1::EnvVar {
            name: "POD_NAMESPACE".to_string(),
            value_from: Some(k8s_openapi::api::core::v1::EnvVarSource {
                field_ref: Some(k8s_openapi::api::core::v1::ObjectFieldSelector {
                    field_path: "metadata.namespace".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        k8s_openapi::api::core::v1::EnvVar {
            name: "ORBIT_CLUSTER_NAME".to_string(),
            value: Some(name.to_string()),
            ..Default::default()
        },
        k8s_openapi::api::core::v1::EnvVar {
            name: "ORBIT_REPLICA_COUNT".to_string(),
            value: Some(cluster.spec.replicas.to_string()),
            ..Default::default()
        },
    ];

    // Add custom environment variables
    for (key, value) in &cluster.spec.env {
        env_vars.push(k8s_openapi::api::core::v1::EnvVar {
            name: key.clone(),
            value: Some(value.clone()),
            ..Default::default()
        });
    }

    let statefulset = StatefulSet {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(ns.to_string()),
            labels: Some(create_labels(cluster)),
            owner_references: Some(vec![cluster.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        spec: Some(StatefulSetSpec {
            service_name: format!("{}-headless", name),
            replicas: Some(cluster.spec.replicas),
            selector: k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector {
                match_labels: Some(create_selector_labels(cluster)),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                    labels: Some(create_labels(cluster)),
                    annotations: Some({
                        let mut annotations = BTreeMap::new();
                        if cluster.spec.monitoring.enabled {
                            annotations.insert("prometheus.io/scrape".to_string(), "true".to_string());
                            annotations.insert("prometheus.io/port".to_string(), cluster.spec.service.metrics_port.to_string());
                            annotations.insert("prometheus.io/path".to_string(), "/metrics".to_string());
                        }
                        annotations
                    }),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    service_account_name: Some(name.to_string()),
                    containers: vec![k8s_openapi::api::core::v1::Container {
                        name: "orbit-server".to_string(),
                        image: Some(format!("{}:{}", cluster.spec.image.repository, cluster.spec.image.tag)),
                        image_pull_policy: Some(cluster.spec.image.pull_policy.clone()),
                        ports: Some(vec![
                            k8s_openapi::api::core::v1::ContainerPort {
                                name: Some("grpc".to_string()),
                                container_port: cluster.spec.service.grpc_port as i32,
                                protocol: Some("TCP".to_string()),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::ContainerPort {
                                name: Some("health".to_string()),
                                container_port: cluster.spec.service.health_port as i32,
                                protocol: Some("TCP".to_string()),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::ContainerPort {
                                name: Some("metrics".to_string()),
                                container_port: cluster.spec.service.metrics_port as i32,
                                protocol: Some("TCP".to_string()),
                                ..Default::default()
                            },
                        ]),
                        env: Some(env_vars),
                        volume_mounts: Some(vec![
                            k8s_openapi::api::core::v1::VolumeMount {
                                name: "data".to_string(),
                                mount_path: "/app/data".to_string(),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::VolumeMount {
                                name: "config".to_string(),
                                mount_path: "/app/config".to_string(),
                                read_only: Some(true),
                                ..Default::default()
                            },
                        ]),
                        resources: Some(k8s_openapi::api::core::v1::ResourceRequirements {
                            requests: Some({
                                let mut requests = BTreeMap::new();
                                requests.insert("cpu".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(cluster.spec.resources.cpu_request.clone()));
                                requests.insert("memory".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(cluster.spec.resources.memory_request.clone()));
                                requests
                            }),
                            limits: Some({
                                let mut limits = BTreeMap::new();
                                limits.insert("cpu".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(cluster.spec.resources.cpu_limit.clone()));
                                limits.insert("memory".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(cluster.spec.resources.memory_limit.clone()));
                                limits
                            }),
                            ..Default::default()
                        }),
                        liveness_probe: Some(k8s_openapi::api::core::v1::Probe {
                            http_get: Some(k8s_openapi::api::core::v1::HTTPGetAction {
                                path: Some("/health/live".to_string()),
                                port: k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String("health".to_string()),
                                ..Default::default()
                            }),
                            initial_delay_seconds: Some(30),
                            period_seconds: Some(30),
                            timeout_seconds: Some(5),
                            failure_threshold: Some(3),
                            success_threshold: Some(1),
                            ..Default::default()
                        }),
                        readiness_probe: Some(k8s_openapi::api::core::v1::Probe {
                            http_get: Some(k8s_openapi::api::core::v1::HTTPGetAction {
                                path: Some("/health/ready".to_string()),
                                port: k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String("health".to_string()),
                                ..Default::default()
                            }),
                            initial_delay_seconds: Some(10),
                            period_seconds: Some(10),
                            timeout_seconds: Some(3),
                            failure_threshold: Some(3),
                            success_threshold: Some(1),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    volumes: Some(vec![
                        k8s_openapi::api::core::v1::Volume {
                            name: "config".to_string(),
                            config_map: Some(k8s_openapi::api::core::v1::ConfigMapVolumeSource {
                                name: Some(format!("{}-config", name)),
                                default_mode: Some(0o644),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ]),
                    ..Default::default()
                }),
            },
            volume_claim_templates: if cluster.spec.storage.size != "0" {
                Some(vec![k8s_openapi::api::core::v1::PersistentVolumeClaim {
                    metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                        name: Some("data".to_string()),
                        labels: Some(create_labels(cluster)),
                        ..Default::default()
                    },
                    spec: Some(PersistentVolumeClaimSpec {
                        access_modes: Some(vec![cluster.spec.storage.access_mode.clone()]),
                        storage_class_name: cluster.spec.storage.storage_class.clone(),
                        resources: Some(k8s_openapi::api::core::v1::VolumeResourceRequirements {
                            requests: Some({
                                let mut requests = BTreeMap::new();
                                requests.insert("storage".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(cluster.spec.storage.size.clone()));
                                requests
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }])
            } else {
                None
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    let pp = PostParams::default();
    match statefulsets.create(&pp, &statefulset).await {
        Ok(_) => info!("Created StatefulSet {}", name),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            // Update existing StatefulSet
            let patch = Patch::Merge(&statefulset);
            statefulsets
                .patch(&name, &PatchParams::default(), &patch)
                .await?;
            info!("Updated StatefulSet {}", name);
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to create StatefulSet: {}", e)),
    }

    Ok(())
}

async fn update_cluster_status(
    cluster: &OrbitCluster,
    ctx: &ControllerContext,
    phase: ClusterPhase,
    message: &str,
) -> Result<()> {
    let ns = cluster.namespace().unwrap_or_default();
    let name = cluster.name_any();
    let clusters: Api<OrbitCluster> = Api::namespaced(ctx.client.clone(), &ns);

    let condition = ClusterCondition {
        condition_type: "Ready".to_string(),
        status: if matches!(phase, ClusterPhase::Running) {
            "True"
        } else {
            "False"
        }
        .to_string(),
        last_transition_time: Some(chrono::Utc::now()),
        reason: Some(format!("{:?}", phase)),
        message: Some(message.to_string()),
    };

    let status = OrbitClusterStatus {
        phase: Some(phase),
        ready_replicas: None, // This would be populated by monitoring the StatefulSet
        replicas: Some(cluster.spec.replicas),
        conditions: vec![condition],
        leader: None, // This would be populated by leader election monitoring
        observed_generation: cluster.metadata.generation,
    };

    let status_patch = json!({
        "status": status
    });

    let patch = Patch::Merge(&status_patch);
    clusters
        .patch_status(&name, &PatchParams::default(), &patch)
        .await?;

    Ok(())
}

fn create_labels(cluster: &OrbitCluster) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), "orbit-rs".to_string());
    labels.insert(
        "app.kubernetes.io/component".to_string(),
        "server".to_string(),
    );
    labels.insert("app.kubernetes.io/instance".to_string(), cluster.name_any());
    labels.insert(
        "app.kubernetes.io/managed-by".to_string(),
        "orbit-operator".to_string(),
    );
    labels.insert(
        "orbit.turingworks.com/cluster".to_string(),
        cluster.name_any(),
    );
    labels
}

fn create_selector_labels(cluster: &OrbitCluster) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), "orbit-rs".to_string());
    labels.insert(
        "app.kubernetes.io/component".to_string(),
        "server".to_string(),
    );
    labels.insert("app.kubernetes.io/instance".to_string(), cluster.name_any());
    labels
}

fn error_policy(
    cluster: Arc<OrbitCluster>,
    error: &anyhow::Error,
    _ctx: Arc<ControllerContext>,
) -> Action {
    warn!(
        "Reconcile failed for OrbitCluster {}: {}",
        cluster.name_any(),
        error
    );
    Action::requeue(Duration::from_secs(60))
}
