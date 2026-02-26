use serde::{Deserialize, Serialize};
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Deserialize)]
struct Input {
    ip: String,
}

#[derive(Serialize)]
struct Output {
    valid: bool,
    version: String,
    address: String,
    is_loopback: bool,
    is_multicast: bool,
    is_private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    octets: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segments: Option<Vec<u16>>,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn main() {
    let mut input_str = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input_str) {
        let error = ErrorOutput {
            error: format!("Failed to read input: {}", e),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let input: Input = match serde_json::from_str(&input_str) {
        Ok(i) => i,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid JSON input: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let ip_addr: IpAddr = match input.ip.parse() {
        Ok(addr) => addr,
        Err(_) => {
            let output = Output {
                valid: false,
                version: "invalid".to_string(),
                address: input.ip,
                is_loopback: false,
                is_multicast: false,
                is_private: false,
                octets: None,
                segments: None,
            };
            println!("{}", serde_json::to_string(&output).unwrap());
            return;
        }
    };

    let (version, octets, segments) = match ip_addr {
        IpAddr::V4(ipv4) => ("IPv4".to_string(), Some(ipv4.octets().to_vec()), None),
        IpAddr::V6(ipv6) => ("IPv6".to_string(), None, Some(ipv6.segments().to_vec())),
    };

    let is_private = match ip_addr {
        IpAddr::V4(ipv4) => {
            ipv4.is_private()
                || ipv4.octets()[0] == 10
                || (ipv4.octets()[0] == 172 && (ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31))
                || (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168)
        }
        IpAddr::V6(_) => false,
    };

    let output = Output {
        valid: true,
        version,
        address: ip_addr.to_string(),
        is_loopback: ip_addr.is_loopback(),
        is_multicast: ip_addr.is_multicast(),
        is_private,
        octets,
        segments,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
