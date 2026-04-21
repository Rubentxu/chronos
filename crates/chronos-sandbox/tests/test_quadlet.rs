//! Tests for Quadlet file generation.
//!
//! These tests verify Quadlet container/network/volume builders
//! and container options generation.

use chronos_sandbox::container::quadlet::{
    ContainerOptions, PortMapping, QuadletBuilder, QuadletFile, QuadletType, VolumeMapping,
    ptrace_container_options,
};

/// Tests building a basic container quadlet.
#[test]
fn test_quadlet_container_builder() {
    let quadlet = QuadletBuilder::new_container("myapp")
        .image("docker.io/alpine:latest")
        .add_cap("NET_ADMIN")
        .add_security_opt("no-new-privileges")
        .add_port(PortMapping::new(8080, 80))
        .network("chronos-network")
        .build()
        .unwrap();

    assert_eq!(quadlet.name, "myapp");
    assert_eq!(quadlet.quadlet_type, QuadletType::Container);
    assert!(quadlet.content.contains("[Container]"), "Should have [Container] section");
    assert!(quadlet.content.contains("Image=docker.io/alpine:latest"));
    assert!(quadlet.content.contains("ContainerName=myapp"));
    assert!(quadlet.content.contains("Capability=NET_ADMIN"));
    assert!(quadlet.content.contains("PublishPort=8080:80/tcp"));
    assert!(quadlet.content.contains("Network=chronos-network"));
}

/// Tests building a network quadlet.
#[test]
fn test_quadlet_network_builder() {
    let quadlet = QuadletBuilder::new_network("chronos-net")
        .network_driver("bridge")
        .network_subnet("10.89.0.0/16")
        .network_gateway("10.89.0.1")
        .build()
        .unwrap();

    assert_eq!(quadlet.name, "chronos-net");
    assert_eq!(quadlet.quadlet_type, QuadletType::Network);
    assert!(
        quadlet.content.contains("[Network]"),
        "Should have [Network] section"
    );
    assert!(quadlet.content.contains("Driver=bridge"));
    assert!(quadlet.content.contains("Subnet=10.89.0.0/16"));
    assert!(quadlet.content.contains("Gateway=10.89.0.1"));
}

/// Tests building a volume quadlet.
#[test]
fn test_quadlet_volume_builder() {
    let quadlet = QuadletBuilder::new_volume("chronos-data")
        .volume_driver("local")
        .add_volume_opt("type", "filesystem")
        .build()
        .unwrap();

    assert_eq!(quadlet.name, "chronos-data");
    assert_eq!(quadlet.quadlet_type, QuadletType::Volume);
    assert!(
        quadlet.content.contains("[Volume]"),
        "Should have [Volume] section"
    );
    assert!(quadlet.content.contains("Driver=local"));
    assert!(quadlet.content.contains("Opt=type=filesystem"));
}

/// Tests container quadlet with ptrace capabilities has correct security options.
#[test]
fn test_quadlet_ptrace_options() {
    let quadlet = QuadletBuilder::new_container("rust-debug")
        .image("rust-debug:latest")
        .add_cap("SYS_PTRACE")
        .add_security_opt("seccomp=unconfined")
        .build()
        .unwrap();

    assert!(quadlet.content.contains("Capability=SYS_PTRACE"));
    assert!(quadlet
        .content
        .contains("SecurityOpt=seccomp=unconfined"));
}

/// Tests ContainerOptions default values.
#[test]
fn test_container_options_default() {
    let opts = ContainerOptions::default();

    assert!(opts.image.is_empty());
    assert!(opts.name.is_empty());
    assert!(opts.caps.is_empty());
    assert!(opts.security_opt.is_empty());
    assert!(opts.ports.is_empty());
    assert!(opts.volumes.is_empty());
    assert!(opts.env.is_empty());
    assert!(opts.network.is_none());
    assert!(opts.pod.is_none());
    assert!(opts.command.is_empty());
    assert!(opts.user.is_none());
    assert!(opts.workdir.is_none());
}

/// Tests ptrace_container_options helper function.
#[test]
fn test_ptrace_container_options_helper() {
    let opts = ptrace_container_options("myimage:latest", "myname", 2345);

    assert_eq!(opts.image, "myimage:latest");
    assert_eq!(opts.name, "myname");
    assert!(opts.caps.contains(&"SYS_PTRACE".to_string()));
    assert!(opts
        .security_opt
        .contains(&"seccomp=unconfined".to_string()));
    assert!(opts.ports.iter().any(|p| p.host == 2345 && p.container == 2345));
    assert_eq!(opts.network, Some("chronos-network".to_string()));
}

/// Tests PortMapping creation and UDP variant.
#[test]
fn test_port_mapping() {
    let tcp = PortMapping::new(8080, 80);
    assert_eq!(tcp.host, 8080);
    assert_eq!(tcp.container, 80);
    assert_eq!(tcp.protocol, "tcp");

    let udp = PortMapping::udp(53, 53);
    assert_eq!(udp.protocol, "udp");
}

/// Tests VolumeMapping creation and readonly variant.
#[test]
fn test_volume_mapping() {
    let vol = VolumeMapping::new("/host/data", "/container/data");
    assert_eq!(vol.host_path, std::path::PathBuf::from("/host/data"));
    assert_eq!(vol.container_path, std::path::PathBuf::from("/container/data"));
    assert_eq!(vol.options, "rw");

    let readonly = VolumeMapping::readonly("/host/ro", "/container/ro");
    assert_eq!(readonly.options, "ro");
}

/// Tests QuadletFile filename generation.
#[test]
fn test_quadlet_file_filename() {
    let quadlet = QuadletFile {
        quadlet_type: QuadletType::Container,
        name: "mycontainer".to_string(),
        path: std::path::PathBuf::from("mycontainer.container"),
        content: String::new(),
    };

    assert_eq!(quadlet.filename(), "mycontainer.container");
}

/// Tests building a pod quadlet.
#[test]
fn test_quadlet_pod_builder() {
    let quadlet = QuadletBuilder::new_pod("chronos-pod")
        .network("chronos-network")
        .build()
        .unwrap();

    assert_eq!(quadlet.name, "chronos-pod");
    assert_eq!(quadlet.quadlet_type, QuadletType::Pod);
    assert!(quadlet.content.contains("[Pod]"), "Should have [Pod] section");
    assert!(quadlet.content.contains("PodName=chronos-pod"));
}
