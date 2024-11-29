use std::{
	error::Error as StdError,
	io::{self, Write},
	net::SocketAddr,
	sync::Arc,
	time::SystemTime,
};

use anyhow::Context as _;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};
use tokio::{
	io::{AsyncReadExt as _, AsyncWriteExt as _},
	net::TcpStream,
};
use tokio_rustls::{
	rustls::{self, pki_types::ServerName, ClientConfig},
	TlsAcceptor, TlsConnector,
};

use super::{AddrPort, ChunkFormatter};

pub(super) struct Conn {
	log: Box<dyn io::Write + Send>,
	logfile: String,
	tz_offset: UtcOffset,
	formatter: ChunkFormatter,

	acceptor: tokio_rustls::TlsAcceptor,
	server_stream: Option<TcpStream>,
	peer: SocketAddr,
	target: AddrPort,
	tls_client_cfg: Arc<rustls::ClientConfig>,

	dir: char,
	server_offset: usize,
	client_offset: usize,
	chunk: Vec<u8>,
	chunk_start_time: SystemTime,
}

impl Conn {
	#[allow(clippy::too_many_arguments)] // There's just no getting around it, really
	pub(super) fn new(
		log: impl io::Write + Send + 'static,
		logfile: String,
		tz_offset: UtcOffset,
		formatter: ChunkFormatter,

		acceptor: TlsAcceptor,
		server_stream: TcpStream,
		peer: SocketAddr,
		target: AddrPort,
		tls_client_cfg: Arc<ClientConfig>,
	) -> Self {
		Self {
			log: Box::new(log),
			logfile,
			tz_offset,
			formatter,

			acceptor,
			server_stream: Some(server_stream),
			peer,
			target,
			tls_client_cfg,

			dir: '|',
			server_offset: 0,
			client_offset: 0,
			chunk: Vec::new(),
			chunk_start_time: SystemTime::now(),
		}
	}

	pub(super) async fn run(mut self) {
		let server_stream = self
			.server_stream
			.take()
			.expect("server_stream should not be None!");
		if let Err(e) = self.process_connection(server_stream).await {
			writeln!(self.log, "# ERROR {} {e:?}", self.rfc3339_now())
				.ftw("error message", &self.logfile);
		};
	}

	async fn process_connection(&mut self, server_stream: TcpStream) -> anyhow::Result<()> {
		let mut tls_server_stream = self.acceptor.accept(server_stream).await?;

		writeln!(self.log, "# ACCEPT {} {}", self.rfc3339_now(), self.peer)
			.ftw("ACCEPT line", &self.logfile);

		let server_name = ServerName::try_from(self.target.host())?.to_owned();

		let client_stream = TcpStream::connect(self.target.to_string())
			.await
			.context(format!("connection failed to {}", self.target))?;
		let connector = TlsConnector::from(self.tls_client_cfg.clone());
		let mut tls_client_stream = connector
			.connect(server_name, client_stream)
			.await
			.context(format!(
				"failed to establish TLS connection to {}",
				self.target
			))?;

		writeln!(self.log, "# CONNECT {} {}", self.rfc3339_now(), self.target)
			.ftw("CONNECT line", &self.logfile);

		let mut server_buf = [0u8; 8192];
		let mut client_buf = [0u8; 8192];

		let mut server_closed = false;
		let mut client_closed = false;

		while !(server_closed && client_closed) {
			if server_closed {
				if let Ok(n) = tls_client_stream.read(&mut client_buf).await {
					if n > 0 {
						self.client_chunk(&client_buf[..n]);
					} else {
						writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.target)
							.ftw("writing close", &self.logfile);
						client_closed = true;
					}
				} else {
					writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.target)
						.ftw("writing close", &self.logfile);
					client_closed = true;
				}
			} else if client_closed {
				if let Ok(n) = tls_server_stream.read(&mut server_buf).await {
					if n > 0 {
						tls_client_stream.write_all(&server_buf[..n]).await?;
						self.server_chunk(&server_buf[..n]);
					} else {
						writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.peer)
							.ftw("writing close", &self.logfile);
						server_closed = true;
					}
				} else {
					writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.peer)
						.ftw("writing close", &self.logfile);
					server_closed = true;
				}
			} else {
				tokio::select! {
					server_result = tls_server_stream.read(&mut server_buf) => {
						match server_result {
							Ok(0) => {
								writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.peer).ftw("writing close", &self.logfile);
								let _ = tls_client_stream.shutdown().await;
								server_closed = true;
							},
							Ok(n) => {
								tls_client_stream.write_all(&server_buf[..n]).await?;
								self.server_chunk(&server_buf[..n]);
							},
							Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
								writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.peer).ftw("writing close", &self.logfile);
								let _ = tls_client_stream.shutdown().await;
								server_closed = true;
							},
							Err(e) => {
								writeln!(self.log, "# WARN {} failed to read from client: {e}", self.rfc3339_now()).ftw("writing server read warning", &self.logfile);
								let _ = tls_client_stream.shutdown().await;
								server_closed = true;
							},
						}
					}

					client_result = tls_client_stream.read(&mut client_buf) => {
						match client_result {
							Ok(0) => {
								writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.target).ftw("writing close", &self.logfile);
								let _ = tls_server_stream.shutdown().await;
								client_closed = true;
							},
							Ok(n) => {
								tls_server_stream.write_all(&client_buf[..n]).await?;
								self.client_chunk(&client_buf[..n]);
							},
							Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
								writeln!(self.log, "# CLOSE {} {}", self.rfc3339_now(), self.target).ftw("writing close", &self.logfile);
								let _ = tls_server_stream.shutdown().await;
								client_closed = true;
							},
							Err(e) => {
								writeln!(self.log, "# WARN {} failed to read from target: {e}", self.rfc3339_now()).ftw("writing client read warning", &self.logfile);
								let _ = tls_server_stream.shutdown().await;
								client_closed = true;
							},
						}
					},
				}
			}
		}
		self.log_chunk();

		Ok(())
	}

	fn log_chunk(&mut self) {
		if !self.chunk.is_empty() {
			writeln!(
				self.log,
				"{} {} offset={} len={}",
				self.dir,
				self.rfc3339_at(&self.chunk_start_time),
				self.dir_offset(),
				self.chunk.len()
			)
			.ftw("chunk header", &self.logfile);
			self.log
				.write_all(&self.formatter.format_chunk(&self.chunk))
				.ftw("chunk", &self.logfile);
			self.advance_dir_offset();
			self.chunk.clear();
		}
	}

	fn server_chunk(&mut self, chunk: &[u8]) {
		if self.dir == '<' {
			self.log_chunk();
		}
		if self.dir != '>' {
			self.dir = '>';
			self.chunk_start_time = SystemTime::now();
		}
		self.chunk.extend_from_slice(chunk);
	}

	fn client_chunk(&mut self, chunk: &[u8]) {
		if self.dir == '>' {
			self.log_chunk();
		}
		if self.dir != '<' {
			self.dir = '<';
			self.chunk_start_time = SystemTime::now();
		}
		self.chunk.extend_from_slice(chunk);
	}

	fn dir_offset(&self) -> usize {
		if self.dir == '>' {
			self.server_offset
		} else if self.dir == '<' {
			self.client_offset
		} else {
			panic!("called dir_offset on invalid dir")
		}
	}

	fn advance_dir_offset(&mut self) {
		if self.dir == '>' {
			self.server_offset += self.chunk.len();
		} else if self.dir == '<' {
			self.client_offset += self.chunk.len();
		} else {
			panic!("called advance_dir_offset on invalid dir")
		}
	}

	fn rfc3339_now(&self) -> String {
		self.rfc3339_at(&SystemTime::now())
	}

	fn rfc3339_at(&self, t: &SystemTime) -> String {
		OffsetDateTime::from(t.to_owned())
			.to_offset(self.tz_offset)
			.format(&Rfc3339)
			.expect("failed to format time")
	}
}

trait QuickError<T, E> {
	// "Failed To Write", very much not... the other one
	fn ftw(self, what: impl AsRef<str>, dest: impl AsRef<str>) -> T;
}

impl<T, E: StdError + 'static> QuickError<T, E> for Result<T, E> {
	fn ftw(self, what: impl AsRef<str>, dest: impl AsRef<str>) -> T {
		self.unwrap_or_else(|e| {
			panic!(
				"failed to write {} to {}: {e}",
				what.as_ref(),
				dest.as_ref()
			)
		})
	}
}
