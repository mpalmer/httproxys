use std::{fmt, net::SocketAddr, path::PathBuf, str::FromStr};

use clap::Parser;

mod chunk_formatter;
use chunk_formatter::ChunkFormatter;
mod conn;
use conn::Conn;
mod proxy;
use proxy::Proxy;

#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
struct Config {
	/// File of CA certs to use to verify outgoing connections
	#[arg(long, value_name = "cafile")]
	ca: Option<PathBuf>,

	/// Log traffic chunks in a hexdump format
	#[arg(long, conflicts_with("raw"))]
	hex: bool,

	/// Log traffic chunks in "raw" format
	#[arg(long, conflicts_with("hex"))]
	raw: bool,

	/// File containing the private key to use on incoming connections
	#[arg(long)]
	key: PathBuf,

	/// File containing the certificate (and optional chain) to use on incoming connections
	#[arg(long)]
	cert: PathBuf,

	/// Address to listen for connections on
	listen: SocketAddr,

	/// Where to connect to
	connect: AddrPort,

	/// Base location to write logs
	baselogfile: PathBuf,
}

#[derive(Clone, Debug)]
enum AddrPort {
	Socket(SocketAddr),
	Host(String, u16),
}

impl FromStr for AddrPort {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if let Ok(sa) = s.parse::<SocketAddr>() {
			Ok(AddrPort::Socket(sa))
		} else {
			let bits: Vec<&str> = s.rsplitn(2, ':').collect();
			if bits.len() == 2 {
				Ok(AddrPort::Host(
					bits[1].to_string(),
					bits[0]
						.parse()
						.map_err(|_| format!("Invalid port: {}", bits[0]))?,
				))
			} else {
				Err(format!("Invalid address: {s}"))
			}
		}
	}
}

impl fmt::Display for AddrPort {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
		match self {
			AddrPort::Socket(sa) => f.write_str(&sa.to_string()),
			AddrPort::Host(host, port) => f.write_fmt(format_args!("{host}:{port}")),
		}
	}
}

impl AddrPort {
	fn host(&self) -> String {
		match self {
			AddrPort::Socket(sa) => sa.ip().to_string(),
			AddrPort::Host(host, _) => host.clone(),
		}
	}
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cfg = Config::parse();

	Proxy::new(cfg).run().await
}
