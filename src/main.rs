use std::{
    result, 
    env,
    net::IpAddr,
    time::Duration,
    io::BufRead,
    collections::HashMap,
};
use futures::future::join_all;
use rand::random;
use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence, ICMP};
use tokio::time;
use ipnet::{Ipv4Net, Ipv6Net};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let ips = get_ip_from_file(&args[2]).await?;
    let ipaddrs = get_ip_range(&args[1], ips).await?;
    let client_v4 = Client::new(&Config::default())?;
    let client_v6 = Client::new(&Config::builder().kind(ICMP::V6).build())?;
    let mut tasks = Vec::new();

    for ip in &ipaddrs {
        match ip.parse() {
            Ok(IpAddr::V4(addr)) => {
                tasks.push(tokio::spawn(ping(client_v4.clone(), IpAddr::V4(addr))))

            }
            Ok(IpAddr::V6(addr)) => {
                tasks.push(tokio::spawn(ping(client_v6.clone(), IpAddr::V6(addr))))
            }
            Err(e) => println!("{} parse to ipaddr error: {}", ip, e),
        }
    }
    let results: Vec<_> = join_all(tasks).await.into_iter().filter_map(Result::ok).collect();
    let mut ip_map = HashMap::new();

    for result in results {
        match result {
            Ok((ip, delay)) => {
                ip_map.insert(ip, delay);
            }
            Err(e) => {
                eprintln!("Error occurred: {:?}", e);
            }
        }
    }
    let mut top_10: Vec<_> = ip_map.into_iter()
    .filter(|(_, delay)| delay.as_millis() > 0) 
    .collect::<Vec<_>>();
    top_10.sort_by(|(_, delay1), (_, delay2)| delay1.cmp(delay2));
    let top_10 = top_10.into_iter().take(10).collect::<Vec<_>>();

    for (ip, delay) in top_10 {
        println!("{} {:?}", ip, delay.as_millis());
    }
    Ok(())
}
async fn ping(client: Client, addr: IpAddr) -> Result<(IpAddr, Duration), Box<dyn std::error::Error + Send + 'static>> {    let payload = [0; 56];
    let mut pinger = client.pinger(addr, PingIdentifier(random())).await;
    let mut interval = time::interval(Duration::from_secs(1));
    let mut delays = Vec::new();

    for idx in 0..4 {
        interval.tick().await;
        match pinger.ping(PingSequence(idx), &payload).await {
            Ok((IcmpPacket::V4(_packet), dur)) => {
                delays.push(dur); 
            }
            Ok((IcmpPacket::V6(_packet), dur)) => {
                delays.push(dur); 
            }
            Err(_e) => {
            }
        };
    }
    let average_delay = if !delays.is_empty() {
        let total: Duration = delays.iter().sum();
        total / delays.len() as u32 
    } else {
        Duration::new(0, 0)
    };
    Ok((addr, average_delay))
}

async fn get_ip_from_file(file_path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut ips = Vec::new();
    let file = std::fs::File::open(file_path)?;
    let mut buf_reader = std::io::BufReader::new(file);
    let mut line = String::new();
    
    while buf_reader.read_line(&mut line).unwrap() > 0 {
        let ip = line.trim();
        ips.push(ip.to_string()); 
        line.clear();
    }
    Ok(ips)
}


async fn get_ip_range(ip_type: &str, ips: Vec<String>) -> result::Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut ip_subnets = Vec::new();

    match ip_type {
        "ipv4" => {
            for ip in ips {
                let net: Ipv4Net = ip.parse().unwrap();
                let subnets = net.subnets(32).expect("PrefixLenError: new prefix length cannot be shorter than existing"); 
                for (_i, n) in subnets.enumerate() {
                    ip_subnets.push(n.to_string());
                }
            } 
        }
        "ipv6" => {
            for ip in ips {
                let net: Ipv6Net = ip.parse().unwrap();
                let subnets = net.subnets(128).expect("PrefixLenError: new prefix length cannot be shorter than existing");
                for (_i, n) in subnets.enumerate() {
                    ip_subnets.push(n.to_string());
                }
            }
        }
        _ => {
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid IP type")));
        }
    }
    let ip_range: Vec<String> =  ip_subnets.into_iter().map(|subnet: String| subnet.split('/').next().unwrap().to_string()).collect();
    Ok(ip_range)
}