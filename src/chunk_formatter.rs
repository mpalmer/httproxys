pub(super) enum ChunkFormatter {
	Text,
	Hex,
	Raw,
}

impl ChunkFormatter {
	pub(super) fn format_chunk(&self, chunk: &[u8]) -> Vec<u8> {
		match self {
			ChunkFormatter::Text => Self::format_text(chunk),
			ChunkFormatter::Hex => Self::format_hex(chunk),
			ChunkFormatter::Raw => Self::format_raw(chunk),
		}
	}

	fn format_text(chunk: &[u8]) -> Vec<u8> {
		let mut res = Vec::new();

		for &c in chunk {
			if Self::is_printable(c) || c == b'\n' {
				res.push(c);
			} else if c == b'\r' {
				res.push(b'\\');
				res.push(b'r');
			} else {
				res.push(b'.');
			}
		}
		res.push(b'\n');

		res
	}

	fn format_hex(chunk: &[u8]) -> Vec<u8> {
		let mut res: Vec<u8> = Vec::new();

		for (i, chunklet) in chunk.chunks(16).enumerate() {
			res.extend_from_slice(format!("{:08x}   ", i * 16).as_bytes());

			for (j, c) in chunklet.iter().enumerate() {
				res.extend_from_slice(format!("{c:02x} ").as_bytes());
				if j == 7 {
					res.extend_from_slice(b" ");
				}
			}

			// Padding for the last short line, so the ASCII dump
			// lines up
			if chunklet.len() < 8 {
				res.extend_from_slice(b" ");
			}

			if chunklet.len() < 16 {
				res.extend_from_slice("   ".repeat(16 - chunklet.len()).as_bytes());
			}

			res.extend_from_slice(b"   ");
			for c in chunklet {
				if Self::is_printable(*c) {
					res.push(*c);
				} else {
					res.push(b'.');
				}
			}

			res.push(b'\n')
		}

		res
	}

	fn format_raw(chunk: &[u8]) -> Vec<u8> {
		let mut res = Vec::new();

		res.extend_from_slice(chunk);
		res.push(b'\n');

		res
	}

	fn is_printable(c: u8) -> bool {
		c.is_ascii() && !c.is_ascii_control()
	}
}
