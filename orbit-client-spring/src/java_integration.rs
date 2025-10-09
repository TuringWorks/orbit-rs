use crate::{
    grpc_server::{GrpcServerConfig, SpringGrpcServer},
    http_server::{HttpServerConfig, SpringHttpServer},
    ApplicationContext, SpringResult,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Comprehensive Java Spring Boot integration that provides HTTP, gRPC, and JNI interfaces
pub struct JavaSpringBootIntegration {
    context: Arc<RwLock<ApplicationContext>>,
    http_server: Option<SpringHttpServer>,
    grpc_server: Option<SpringGrpcServer>,
    config: JavaIntegrationConfig,
}

/// Configuration for the Java Spring Boot integration
#[derive(Debug, Clone)]
pub struct JavaIntegrationConfig {
    pub http_config: Option<HttpServerConfig>,
    pub grpc_config: Option<GrpcServerConfig>,
    pub enable_http: bool,
    pub enable_grpc: bool,
    pub enable_jni: bool,
}

impl Default for JavaIntegrationConfig {
    fn default() -> Self {
        Self {
            http_config: Some(HttpServerConfig::default()),
            grpc_config: Some(GrpcServerConfig::default()),
            enable_http: true,
            enable_grpc: true,
            enable_jni: true,
        }
    }
}

impl JavaSpringBootIntegration {
    /// Create a new Java Spring Boot integration
    pub fn new(integration_config: JavaIntegrationConfig) -> Self {
        let context = ApplicationContext::new();
        let context_arc = Arc::new(RwLock::new(context));

        let http_server = if integration_config.enable_http {
            integration_config
                .http_config
                .as_ref()
                .map(|config| SpringHttpServer::new(context_arc.clone(), config.clone()))
        } else {
            None
        };

        let grpc_server = if integration_config.enable_grpc {
            integration_config
                .grpc_config
                .as_ref()
                .map(|config| SpringGrpcServer::new(context_arc.clone(), config.clone()))
        } else {
            None
        };

        Self {
            context: context_arc,
            http_server,
            grpc_server,
            config: integration_config,
        }
    }

    /// Start all enabled servers and services
    pub async fn start(&self) -> SpringResult<()> {
        info!("🚀 Starting Java Spring Boot Integration for Orbit services");

        // Start the application context
        {
            let context = self.context.read().await;
            context.start().await?;
        }

        // Start HTTP server if enabled
        if let Some(http_server) = &self.http_server {
            http_server.start().await?;
        }

        // Start gRPC server if enabled
        if let Some(grpc_server) = &self.grpc_server {
            grpc_server.start().await?;
        }

        // JNI is passive - no startup needed

        info!("✅ Java Spring Boot Integration started successfully");
        self.print_startup_info().await;

        Ok(())
    }

    /// Stop all servers and services
    pub async fn stop(&self) -> SpringResult<()> {
        info!("🛑 Stopping Java Spring Boot Integration");

        // Stop HTTP server
        if let Some(http_server) = &self.http_server {
            http_server.stop().await?;
        }

        // Stop gRPC server
        if let Some(grpc_server) = &self.grpc_server {
            grpc_server.stop().await?;
        }

        // Stop the application context
        {
            let context = self.context.read().await;
            context.stop().await?;
        }

        info!("✅ Java Spring Boot Integration stopped successfully");
        Ok(())
    }

    /// Get the application context
    pub fn context(&self) -> Arc<RwLock<ApplicationContext>> {
        self.context.clone()
    }

    /// Print startup information
    async fn print_startup_info(&self) {
        let context = self.context.read().await;
        let config = context.config();

        println!("\n🌟 ===== Orbit Java Spring Boot Integration Started =====");
        println!(
            "📋 Application: {} v{}",
            config.application.name, config.application.version
        );

        if self.config.enable_http {
            if let Some(http_config) = &self.config.http_config {
                println!(
                    "🌐 HTTP API: http://{}:{}{}",
                    http_config.host, http_config.port, http_config.base_path
                );
                println!(
                    "   📖 Endpoints: GET {}/services, GET {}/config, GET {}/health",
                    http_config.base_path, http_config.base_path, http_config.base_path
                );
            }
        }

        if self.config.enable_grpc {
            if let Some(grpc_config) = &self.config.grpc_config {
                println!("⚡ gRPC API: {}:{}", grpc_config.host, grpc_config.port);
                println!("   📦 Service: OrbitSpringService (mock implementation)");
            }
        }

        if self.config.enable_jni {
            println!("☕ JNI Library: com.orbit.spring.OrbitClient");
            println!("   📚 Methods: initializeContext, startContext, stopContext, etc.");
        }

        println!(
            "🔍 Health Check: GET {}/health",
            if let Some(http_config) = &self.config.http_config {
                format!(
                    "http://{}:{}{}",
                    http_config.host, http_config.port, http_config.base_path
                )
            } else {
                "N/A".to_string()
            }
        );

        let stats = context.container_stats();
        println!("📊 Services: {} registered", stats.total_beans);
        println!("🔧 Server Port: {}", config.server.port);
        println!("======================================================\n");
    }

    /// Wait for shutdown signals
    pub async fn wait_for_shutdown(&self) -> SpringResult<()> {
        info!("⏳ Waiting for shutdown signal...");

        // Create shutdown signal handlers
        let mut shutdown_signals = vec![];

        if let Some(http_server) = &self.http_server {
            shutdown_signals.push(http_server.shutdown_token());
        }

        if let Some(grpc_server) = &self.grpc_server {
            shutdown_signals.push(grpc_server.shutdown_token());
        }

        // Wait for Ctrl+C
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("📡 Received Ctrl+C, initiating shutdown");
            }
            _ = async {
                // Wait for any shutdown signal
                for token in shutdown_signals {
                    let token_clone = token.clone();
                    tokio::spawn(async move {
                        token_clone.cancelled().await;
                    });
                }
            } => {
                info!("📡 Received shutdown signal");
            }
        }

        self.stop().await
    }

    /// Run the integration (start and wait for shutdown)
    pub async fn run(&self) -> SpringResult<()> {
        self.start().await?;
        self.wait_for_shutdown().await
    }

    /// Get integration status
    pub async fn status(&self) -> JavaIntegrationStatus {
        let context = self.context.read().await;
        let is_running = context.is_running().await;
        let stats = context.container_stats();
        let health = context.health_check().await;

        JavaIntegrationStatus {
            running: is_running,
            services_registered: stats.total_beans,
            healthy: health.healthy,
            healthy_services: health.healthy_services,
            unhealthy_services: health.unhealthy_services,
            http_enabled: self.config.enable_http,
            grpc_enabled: self.config.enable_grpc,
            jni_enabled: self.config.enable_jni,
        }
    }
}

/// Status information for the Java integration
#[derive(Debug, Clone)]
pub struct JavaIntegrationStatus {
    pub running: bool,
    pub services_registered: usize,
    pub healthy: bool,
    pub healthy_services: Vec<String>,
    pub unhealthy_services: Vec<String>,
    pub http_enabled: bool,
    pub grpc_enabled: bool,
    pub jni_enabled: bool,
}

// ============================================================================
// Builder Pattern for Easy Configuration
// ============================================================================

/// Builder for JavaSpringBootIntegration
pub struct JavaSpringBootIntegrationBuilder {
    integration_config: JavaIntegrationConfig,
}

impl JavaSpringBootIntegrationBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            integration_config: JavaIntegrationConfig::default(),
        }
    }

    /// Enable/disable HTTP server
    pub fn enable_http(mut self, enabled: bool) -> Self {
        self.integration_config.enable_http = enabled;
        self
    }

    /// Set HTTP server configuration
    pub fn http_config(mut self, config: HttpServerConfig) -> Self {
        self.integration_config.http_config = Some(config);
        self
    }

    /// Enable/disable gRPC server
    pub fn enable_grpc(mut self, enabled: bool) -> Self {
        self.integration_config.enable_grpc = enabled;
        self
    }

    /// Set gRPC server configuration
    pub fn grpc_config(mut self, config: GrpcServerConfig) -> Self {
        self.integration_config.grpc_config = Some(config);
        self
    }

    /// Enable/disable JNI bindings
    pub fn enable_jni(mut self, enabled: bool) -> Self {
        self.integration_config.enable_jni = enabled;
        self
    }

    /// Build the integration
    pub fn build(self) -> JavaSpringBootIntegration {
        JavaSpringBootIntegration::new(self.integration_config)
    }
}

impl Default for JavaSpringBootIntegrationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Create a Java Spring Boot integration with default settings
pub fn create_default_java_integration() -> JavaSpringBootIntegration {
    JavaSpringBootIntegrationBuilder::new().build()
}

/// Create a Java Spring Boot integration with custom configuration
pub fn create_java_integration_with_config(
    integration_config: JavaIntegrationConfig,
) -> JavaSpringBootIntegration {
    JavaSpringBootIntegration::new(integration_config)
}

/// Create an HTTP-only Java integration
pub fn create_http_only_java_integration(
    http_config: HttpServerConfig,
) -> JavaSpringBootIntegration {
    JavaSpringBootIntegrationBuilder::new()
        .enable_grpc(false)
        .enable_jni(false)
        .http_config(http_config)
        .build()
}

/// Create a gRPC-only Java integration
pub fn create_grpc_only_java_integration(
    grpc_config: GrpcServerConfig,
) -> JavaSpringBootIntegration {
    JavaSpringBootIntegrationBuilder::new()
        .enable_http(false)
        .enable_jni(false)
        .grpc_config(grpc_config)
        .build()
}

/// Create a JNI-only Java integration
pub fn create_jni_only_java_integration() -> JavaSpringBootIntegration {
    JavaSpringBootIntegrationBuilder::new()
        .enable_http(false)
        .enable_grpc(false)
        .build()
}

// ============================================================================
// Java Integration Helper Functions
// ============================================================================

/// Java class names for different integration modes
pub struct JavaClassNames;

impl JavaClassNames {
    pub const ORBIT_CLIENT: &'static str = "com.orbit.spring.OrbitClient";
    pub const HTTP_CLIENT: &'static str = "com.orbit.spring.OrbitHttpClient";
    pub const GRPC_CLIENT: &'static str = "com.orbit.spring.OrbitGrpcClient";
    pub const CONFIGURATION: &'static str = "com.orbit.spring.OrbitConfiguration";
}

/// Generate Java client library information
pub fn generate_java_library_info() -> JavaLibraryInfo {
    JavaLibraryInfo {
        group_id: "com.orbit".to_string(),
        artifact_id: "orbit-spring-boot-starter".to_string(),
        version: "1.0.0".to_string(),
        classes: vec![
            JavaClassNames::ORBIT_CLIENT.to_string(),
            JavaClassNames::HTTP_CLIENT.to_string(),
            JavaClassNames::GRPC_CLIENT.to_string(),
            JavaClassNames::CONFIGURATION.to_string(),
        ],
        native_library_name: "liborbit_client_spring".to_string(),
        description: "Orbit Spring Boot Integration Library".to_string(),
    }
}

/// Information about the generated Java library
#[derive(Debug, Clone)]
pub struct JavaLibraryInfo {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    pub classes: Vec<String>,
    pub native_library_name: String,
    pub description: String,
}

// ============================================================================
// Example Usage Functions
// ============================================================================

/// Example: Create and run a full-featured Java integration
pub async fn run_full_java_integration_example() -> SpringResult<()> {
    // Create custom configurations
    let http_config = HttpServerConfig {
        host: "localhost".to_string(),
        port: 8080,
        base_path: "/api/v1".to_string(),
    };

    let grpc_config = GrpcServerConfig {
        host: "localhost".to_string(),
        port: 9090,
        tls_enabled: false,
        max_message_size: 8 * 1024 * 1024, // 8MB
    };

    // Build and run the integration
    let integration = JavaSpringBootIntegrationBuilder::new()
        .http_config(http_config)
        .grpc_config(grpc_config)
        .build();

    integration.run().await
}

/// Example: Create a lightweight HTTP-only Java integration
pub async fn run_http_only_java_example() -> SpringResult<()> {
    let http_config = HttpServerConfig {
        host: "0.0.0.0".to_string(),
        port: 3000,
        base_path: "/orbit".to_string(),
    };

    let integration = create_http_only_java_integration(http_config);
    integration.run().await
}

/// Example: Print Java library usage instructions
pub fn print_java_usage_instructions() {
    let lib_info = generate_java_library_info();

    println!("\n📚 ===== Java Spring Boot Usage Instructions =====");
    println!("Maven Dependency:");
    println!("<dependency>");
    println!("  <groupId>{}</groupId>", lib_info.group_id);
    println!("  <artifactId>{}</artifactId>", lib_info.artifact_id);
    println!("  <version>{}</version>", lib_info.version);
    println!("</dependency>\n");

    println!("Gradle Dependency:");
    println!(
        "implementation '{}:{}:{}'",
        lib_info.group_id, lib_info.artifact_id, lib_info.version
    );

    println!("\nJava Usage Examples:");
    println!("// HTTP Client");
    println!("OrbitHttpClient httpClient = new OrbitHttpClient(\"http://localhost:8080/api/v1\");");
    println!("String config = httpClient.getConfig();");

    println!("\n// gRPC Client");
    println!("OrbitGrpcClient grpcClient = new OrbitGrpcClient(\"localhost\", 9090);");
    println!("ServiceListResponse services = grpcClient.listServices();");

    println!("\n// JNI Direct Calls");
    println!("long contextId = OrbitClient.initializeContext(\"/path/to/config.yaml\");");
    println!("boolean started = OrbitClient.startContext(contextId);");
    println!("String[] services = OrbitClient.getServiceNames(contextId);");

    println!("\nSpring Boot Configuration (application.yml):");
    println!("orbit:");
    println!("  http:");
    println!("    url: http://localhost:8080/api/v1");
    println!("  grpc:");
    println!("    host: localhost");
    println!("    port: 9090");
    println!("  jni:");
    println!("    enabled: true");
    println!("    config-path: /etc/orbit/config.yaml");
    println!("=================================================\n");
}
