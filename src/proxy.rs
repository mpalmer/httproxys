use std::{fs::File, future::Future, net::SocketAddr, sync::Arc};

use anyhow::Context as _;
use rustls_platform_verifier::ConfigVerifierExt;
use time::UtcOffset;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{
	rustls::{
		pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
		ClientConfig, RootCertStore, ServerConfig,
	},
	TlsAcceptor,
};
use webpki::anchor_from_trusted_cert;

use super::{ChunkFormatter, Config, Conn};

pub(super) struct Proxy {
	cfg: Config,
}

impl Proxy {
	pub(super) fn new(cfg: Config) -> Self {
		Proxy { cfg }
	}

	pub(super) async fn run(&self) -> anyhow::Result<()> {
		let certs = CertificateDer::pem_file_iter(&self.cfg.cert)
			.context(format!(
				"failed to read cert file {}",
				self.cfg.cert.display()
			))?
			.collect::<Result<Vec<_>, _>>()
			.context("failed to parse cert")?;
		let key = PrivateKeyDer::from_pem_file(&self.cfg.key).context(format!(
			"failed to read key file {}",
			self.cfg.key.display()
		))?;
		let tls_server_config = Arc::new(
			ServerConfig::builder()
				.with_no_client_auth()
				.with_single_cert(certs, key)
				.context("failed to create ServerConfig")?,
		);

		let tls_client_config = Arc::new(if let Some(cafile) = &self.cfg.ca {
			let ca_certs = CertificateDer::pem_file_iter(cafile)
				.context(format!("failed to read CA certs file {}", cafile.display()))?
				.map::<anyhow::Result<_>, _>(|cert| {
					Ok(
						anchor_from_trusted_cert(&cert.context("failed to parse CA cert")?)
							.context("failed to convert cert to trust anchor")?
							.to_owned(),
					)
				})
				.collect::<Result<Vec<_>, _>>()?;
			let trust_store = RootCertStore { roots: ca_certs };
			ClientConfig::builder()
				.with_root_certificates(trust_store)
				.with_no_client_auth()
		} else {
			ClientConfig::with_platform_verifier()
		});

		let acceptor = TlsAcceptor::from(tls_server_config);
		let listener = TcpListener::bind(self.cfg.listen)
			.await
			.context(format!("failed to listen on {}", self.cfg.listen))?;

		loop {
			let (stream, peer) = listener.accept().await?;

			let fut = self.connection_processor(
				acceptor.clone(),
				tls_client_config.clone(),
				stream,
				peer,
			)?;

			tokio::spawn(fut);
		}
	}

	fn connection_processor(
		&self,
		acceptor: TlsAcceptor,
		tls_client_cfg: Arc<ClientConfig>,
		server_stream: TcpStream,
		peer: SocketAddr,
	) -> anyhow::Result<impl Future<Output = ()> + Send> {
		let logprefix = Arc::new(self.cfg.baselogfile.display().to_string());
		let tz_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
		let logfile = format!("{logprefix}.{}.{}", peer.ip(), peer.port());
		let target = self.cfg.connect.clone();
		let log = File::create(&logfile).context("failed to create logfile {logfile}")?;

		let formatter = if self.cfg.raw {
			ChunkFormatter::Raw
		} else if self.cfg.hex {
			ChunkFormatter::Hex
		} else {
			ChunkFormatter::Text
		};

		Ok(async move {
			Conn::new(
				log,
				logfile,
				tz_offset,
				formatter,
				acceptor,
				server_stream,
				peer,
				target,
				tls_client_cfg,
			)
			.run()
			.await
		})
	}
}
