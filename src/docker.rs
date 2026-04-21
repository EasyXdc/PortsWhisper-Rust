use crate::framework::detect_framework_from_image;
use crate::model::DockerInfo;
use crate::util::run_output;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

pub fn batch_docker_info() -> HashMap<u16, DockerInfo> {
    let raw = run_output(
        "docker",
        ["ps", "--format", "{{.Ports}}\t{{.Names}}\t{{.Image}}"],
        Some(Duration::from_millis(5000)),
    )
    .ok()
    .unwrap_or_default();
    let mut map = HashMap::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let ports = parse_docker_host_ports(parts[0]);
        for port in ports {
            map.insert(
                port,
                DockerInfo {
                    host_port: port,
                    container_name: parts[1].to_string(),
                    image: parts[2].to_string(),
                    framework: detect_framework_from_image(parts[2]),
                },
            );
        }
    }
    map
}

fn parse_docker_host_ports(s: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    let mut seen = HashSet::new();
    for chunk in s.split(',') {
        if let Some(arrow) = chunk.find("->") {
            let before = &chunk[..arrow];
            if let Some(idx) = before.rfind(':')
                && let Ok(port) = before[idx + 1..].parse::<u16>()
                && seen.insert(port)
            {
                ports.push(port);
            }
        }
    }
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_host_port_parser_deduplicates_host_ports() {
        assert_eq!(
            parse_docker_host_ports("0.0.0.0:5432->5432/tcp, :::5432->5432/tcp"),
            vec![5432]
        );
        assert_eq!(
            parse_docker_host_ports("127.0.0.1:6379->6379/tcp, 0.0.0.0:8080->80/tcp"),
            vec![6379, 8080]
        );
    }
}
