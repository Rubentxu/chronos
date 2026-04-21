//! Quadlet file generation for Podman.
//!
//! Generates Quadlet files (`.container`, `.pod`, `.network`, `.volume`) that
//! can be placed in `/etc/containers/containers.conf.d/` or `~/.config/containers/systemd/`
//! for automatic container management by Podman systemd units.

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::path::PathBuf;

/// Represents the type of Quadlet file to generate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QuadletType {
    #[default]
    Container,
    Pod,
    Network,
    Volume,
}

impl QuadletType {
    fn extension(&self) -> &'static str {
        match self {
            QuadletType::Container => "container",
            QuadletType::Pod => "pod",
            QuadletType::Network => "network",
            QuadletType::Volume => "volume",
        }
    }
}

/// A generated Quadlet file with its path and content.
#[derive(Debug, Clone)]
pub struct QuadletFile {
    pub quadlet_type: QuadletType,
    pub name: String,
    pub path: PathBuf,
    pub content: String,
}

impl QuadletFile {
    /// Returns the filename (e.g., `mycontainer.container`).
    pub fn filename(&self) -> String {
        format!("{}.{}", self.name, self.quadlet_type.extension())
    }
}

/// Options for configuring a container.
#[derive(Debug, Clone, Default)]
pub struct ContainerOptions {
    pub image: String,
    pub name: String,
    pub caps: Vec<String>,
    pub security_opt: Vec<String>,
    pub ports: Vec<PortMapping>,
    pub volumes: Vec<VolumeMapping>,
    pub env: HashMap<String, String>,
    pub network: Option<String>,
    pub pod: Option<String>,
    pub command: Vec<String>,
    pub user: Option<String>,
    pub workdir: Option<String>,
}

/// A port mapping in host:container format.
#[derive(Debug, Clone)]
pub struct PortMapping {
    pub host: u16,
    pub container: u16,
    pub protocol: String,
}

impl PortMapping {
    pub fn new(host: u16, container: u16) -> Self {
        Self {
            host,
            container,
            protocol: "tcp".to_string(),
        }
    }

    pub fn udp(host: u16, container: u16) -> Self {
        Self {
            host,
            container,
            protocol: "udp".to_string(),
        }
    }
}

/// A volume mapping in host_path:container_path:options format.
#[derive(Debug, Clone)]
pub struct VolumeMapping {
    pub host_path: PathBuf,
    pub container_path: PathBuf,
    pub options: String,
}

impl VolumeMapping {
    pub fn new(host_path: impl Into<PathBuf>, container_path: impl Into<PathBuf>) -> Self {
        Self {
            host_path: host_path.into(),
            container_path: container_path.into(),
            options: "rw".to_string(),
        }
    }

    pub fn readonly(host_path: impl Into<PathBuf>, container_path: impl Into<PathBuf>) -> Self {
        Self {
            host_path: host_path.into(),
            container_path: container_path.into(),
            options: "ro".to_string(),
        }
    }
}

/// Builder for generating Quadlet file content.
#[derive(Debug, Clone, Default)]
pub struct QuadletBuilder {
    quadlet_type: QuadletType,
    name: String,
    container_opts: ContainerOptions,
    network_opts: NetworkOptions,
    volume_opts: VolumeOptions,
}

#[derive(Debug, Clone, Default)]
struct NetworkOptions {
    name: String,
    driver: String,
    subnet: Option<String>,
    gateway: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct VolumeOptions {
    name: String,
    driver: String,
    opt: HashMap<String, String>,
}

impl QuadletBuilder {
    /// Creates a new builder for a Container Quadlet.
    pub fn new_container(name: impl Into<String>) -> Self {
        let name_str = name.into();
        Self {
            quadlet_type: QuadletType::Container,
            name: name_str.clone(),
            container_opts: ContainerOptions {
                name: name_str,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Creates a new builder for a Pod Quadlet.
    pub fn new_pod(name: impl Into<String>) -> Self {
        let name_str = name.into();
        Self {
            quadlet_type: QuadletType::Pod,
            name: name_str.clone(),
            container_opts: ContainerOptions {
                name: name_str,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Creates a new builder for a Network Quadlet.
    pub fn new_network(name: impl Into<String>) -> Self {
        let name_str = name.into();
        Self {
            quadlet_type: QuadletType::Network,
            name: name_str.clone(),
            network_opts: NetworkOptions {
                name: name_str,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Creates a new builder for a Volume Quadlet.
    pub fn new_volume(name: impl Into<String>) -> Self {
        let name_str = name.into();
        Self {
            quadlet_type: QuadletType::Volume,
            name: name_str.clone(),
            volume_opts: VolumeOptions {
                name: name_str,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Sets container options.
    pub fn container_options(mut self, opts: ContainerOptions) -> Self {
        self.container_opts = opts;
        self
    }

    /// Sets the container image.
    pub fn image(mut self, image: impl Into<String>) -> Self {
        self.container_opts.image = image.into();
        self
    }

    /// Adds a capability to the container.
    pub fn add_cap(mut self, cap: impl Into<String>) -> Self {
        self.container_opts.caps.push(cap.into());
        self
    }

    /// Adds a security option to the container.
    pub fn add_security_opt(mut self, opt: impl Into<String>) -> Self {
        self.container_opts.security_opt.push(opt.into());
        self
    }

    /// Adds a port mapping.
    pub fn add_port(mut self, mapping: PortMapping) -> Self {
        self.container_opts.ports.push(mapping);
        self
    }

    /// Adds a volume mapping.
    pub fn add_volume(mut self, mapping: VolumeMapping) -> Self {
        self.container_opts.volumes.push(mapping);
        self
    }

    /// Sets an environment variable.
    pub fn add_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.container_opts.env.insert(key.into(), value.into());
        self
    }

    /// Sets the network name.
    pub fn network(mut self, network: impl Into<String>) -> Self {
        self.container_opts.network = Some(network.into());
        self
    }

    /// Sets the pod name.
    pub fn pod(mut self, pod: impl Into<String>) -> Self {
        self.container_opts.pod = Some(pod.into());
        self
    }

    /// Sets the container command.
    pub fn command(mut self, cmd: Vec<String>) -> Self {
        self.container_opts.command = cmd;
        self
    }

    /// Sets the network driver.
    pub fn network_driver(mut self, driver: impl Into<String>) -> Self {
        self.network_opts.driver = driver.into();
        self
    }

    /// Sets the network subnet.
    pub fn network_subnet(mut self, subnet: impl Into<String>) -> Self {
        self.network_opts.subnet = Some(subnet.into());
        self
    }

    /// Sets the network gateway.
    pub fn network_gateway(mut self, gateway: impl Into<String>) -> Self {
        self.network_opts.gateway = Some(gateway.into());
        self
    }

    /// Sets the volume driver.
    pub fn volume_driver(mut self, driver: impl Into<String>) -> Self {
        self.volume_opts.driver = driver.into();
        self
    }

    /// Adds a volume option.
    pub fn add_volume_opt(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.volume_opts.opt.insert(key.into(), value.into());
        self
    }

    /// Builds and returns the generated Quadlet file.
    pub fn build(self) -> Result<QuadletFile, std::fmt::Error> {
        let content = match self.quadlet_type {
            QuadletType::Container => self.build_container()?,
            QuadletType::Pod => self.build_pod()?,
            QuadletType::Network => self.build_network()?,
            QuadletType::Volume => self.build_volume()?,
        };

        Ok(QuadletFile {
            quadlet_type: self.quadlet_type.clone(),
            name: self.name.clone(),
            path: PathBuf::from(&self.name).with_extension(self.quadlet_type.extension()),
            content,
        })
    }

    fn build_container(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();

        writeln!(&mut out, "[Unit]")?;
        writeln!(&mut out, "Description=Chronos sandbox container: {}", self.name)?;
        writeln!(&mut out)?;
        writeln!(&mut out, "[Container]")?;
        writeln!(&mut out, "Image={}", self.container_opts.image)?;
        writeln!(&mut out, "ContainerName={}", self.container_opts.name)?;

        // Capabilities
        if !self.container_opts.caps.is_empty() {
            writeln!(
                &mut out,
                "Capability={}",
                self.container_opts.caps.join(" ")
            )?;
        }

        // Security options
        for opt in &self.container_opts.security_opt {
            writeln!(&mut out, "SecurityOpt={}", opt)?;
        }

        // Port mappings
        for port in &self.container_opts.ports {
            writeln!(
                &mut out,
                "PublishPort={}:{}/{}",
                port.host, port.container, port.protocol
            )?;
        }

        // Volume mappings
        for vol in &self.container_opts.volumes {
            writeln!(
                &mut out,
                "Volume={}:{}:{}",
                vol.host_path.display(),
                vol.container_path.display(),
                vol.options
            )?;
        }

        // Environment variables
        for (key, value) in &self.container_opts.env {
            writeln!(&mut out, "Environment=\"{}={}\"", key, value)?;
        }

        // Network
        if let Some(ref network) = self.container_opts.network {
            writeln!(&mut out, "Network={}", network)?;
        }

        // Pod
        if let Some(ref pod) = self.container_opts.pod {
            writeln!(&mut out, "Pod={}", pod)?;
        }

        // Command
        if !self.container_opts.command.is_empty() {
            writeln!(
                &mut out,
                "Exec={}",
                self.container_opts.command.join(" ")
            )?;
        }

        // User
        if let Some(ref user) = self.container_opts.user {
            writeln!(&mut out, "User={}", user)?;
        }

        // Workdir
        if let Some(ref workdir) = self.container_opts.workdir {
            writeln!(&mut out, "WorkingDirectory={}", workdir)?;
        }

        writeln!(&mut out)?;
        writeln!(&mut out, "[Install]")?;
        writeln!(&mut out, "WantedBy=multi-user.target")?;

        Ok(out)
    }

    fn build_pod(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();

        writeln!(&mut out, "[Unit]")?;
        writeln!(&mut out, "Description=Chronos sandbox pod: {}", self.name)?;
        writeln!(&mut out)?;
        writeln!(&mut out, "[Pod]")?;
        writeln!(&mut out, "PodName={}", self.name)?;

        // Network - use a dedicated network for the pod
        if let Some(ref network) = self.container_opts.network {
            writeln!(&mut out, "Network={{{}", network)?;
            writeln!(&mut out, "}}")?;
        }

        writeln!(&mut out)?;
        writeln!(&mut out, "[Install]")?;
        writeln!(&mut out, "WantedBy=multi-user.target")?;

        Ok(out)
    }

    fn build_network(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();

        writeln!(&mut out, "[Unit]")?;
        writeln!(&mut out, "Description=Chronos sandbox network: {}", self.network_opts.name)?;
        writeln!(&mut out)?;
        writeln!(&mut out, "[Network]")?;
        writeln!(&mut out, "Driver={}", self.network_opts.driver)?;

        if let Some(ref subnet) = self.network_opts.subnet {
            writeln!(&mut out, "Subnet={}", subnet)?;
        }

        if let Some(ref gateway) = self.network_opts.gateway {
            writeln!(&mut out, "Gateway={}", gateway)?;
        }

        writeln!(&mut out)?;
        writeln!(&mut out, "[Install]")?;
        writeln!(&mut out, "WantedBy=multi-user.target")?;

        Ok(out)
    }

    fn build_volume(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();

        writeln!(&mut out, "[Unit]")?;
        writeln!(&mut out, "Description=Chronos sandbox volume: {}", self.volume_opts.name)?;
        writeln!(&mut out)?;
        writeln!(&mut out, "[Volume]")?;
        writeln!(&mut out, "Driver={}", self.volume_opts.driver)?;

        for (key, value) in &self.volume_opts.opt {
            writeln!(&mut out, "Opt={}={}", key, value)?;
        }

        writeln!(&mut out)?;
        writeln!(&mut out, "[Install]")?;
        writeln!(&mut out, "WantedBy=multi-user.target")?;

        Ok(out)
    }
}

/// Options for ptrace-capable debugging containers.
///
/// These options enable the necessary capabilities for system call tracing
/// and debugging operations.
pub fn ptrace_container_options(image: &str, name: &str, port: u16) -> ContainerOptions {
    ContainerOptions {
        image: image.to_string(),
        name: name.to_string(),
        caps: vec!["SYS_PTRACE".to_string()],
        security_opt: vec!["seccomp=unconfined".to_string()],
        ports: vec![PortMapping::new(port, port)],
        volumes: vec![],
        env: HashMap::new(),
        network: Some("chronos-network".to_string()),
        pod: None,
        command: vec![],
        user: None,
        workdir: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_quadlet_generation() {
        let quadlet = QuadletBuilder::new_container("test-container")
            .image("docker.io/alpine:latest")
            .add_cap("NET_ADMIN")
            .add_security_opt("no-new-privileges")
            .add_port(PortMapping::new(8080, 80))
            .add_volume(VolumeMapping::new("/data", "/mnt/data"))
            .add_env("DEBUG", "1")
            .network("chronos-network")
            .build()
            .unwrap();

        assert!(quadlet.content.contains("Image=docker.io/alpine:latest"));
        assert!(quadlet.content.contains("ContainerName=test-container"));
        assert!(quadlet.content.contains("Capability=NET_ADMIN"));
        assert!(quadlet.content.contains("PublishPort=8080:80/tcp"));
        assert!(quadlet.content.contains("Network=chronos-network"));
    }

    #[test]
    fn test_network_quadlet_generation() {
        let quadlet = QuadletBuilder::new_network("chronos-network")
            .network_driver("bridge")
            .network_subnet("10.89.0.0/16")
            .network_gateway("10.89.0.1")
            .build()
            .unwrap();

        assert!(quadlet.content.contains("Driver=bridge"));
        assert!(quadlet.content.contains("Subnet=10.89.0.0/16"));
        assert!(quadlet.content.contains("Gateway=10.89.0.1"));
    }

    #[test]
    fn test_volume_quadlet_generation() {
        let quadlet = QuadletBuilder::new_volume("chronos-data")
            .volume_driver("local")
            .add_volume_opt("type", "filesystem")
            .build()
            .unwrap();

        assert!(quadlet.content.contains("Driver=local"));
        assert!(quadlet.content.contains("Opt=type=filesystem"));
    }
}
