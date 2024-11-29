`httproxys` is a TLS interception proxy for those who cherish simplicity.

The intended use case for `httproxys` is pretty simple: you need to see exactly what is going on in a TLS-protected TCP stream (HTTPS most commonly, but by no means exclusively).
You could use [`mitmproxy`](https://mitmproxy.org), but its myriad options and focus on interactive UI over "just gimme the data" [confuses and frightens you](https://en.wikipedia.org/wiki/Unfrozen_Caveman_Lawyer).
Alternately, you could use `socat -v OPENSSL-LISTEN:... OPENSSL:...`, but its frustrating habit of mashing together overlapping traffic in its single log file doesn't work for you.
Finally, you could also try to use [`SSLsplit`](https://www.roe.ch/SSLsplit), if it didn't `SIGSEGV` the moment it accepted a connection.

Thus... `httproxys` was born.


# Installation

With a [Rust](https://rust-lang.org) toolchain installed, it's as simple as:

```
cargo install httproxys
```


# Usage

Execute as follows:

```
httproxys [--ca <cafile>] [--raw|--hex] <listenaddr>:<listenport> <connectaddr>:<connectport> <baselogfile>
```

Where:

* `<listenaddr>` -- an IP address to listen on.
  Typically either `127.0.0.1` (aka `localhost`) or `0.0.0.0` (`INADDR_ANY`) if you're feeling adventurous.
  IPv6 addresses are supported, but must be wrapped in square brackets (eg `[::1]` or `[::]`), to differentiate its colons from the port. separator

* `<listenport>` -- the TCP port to listen on.

* `<connectaddr>` -- the IP address or hostname to make connections to.
  Like `<listenaddr>`, IPv6 address literals must be wrapped in square brackets.

* `<connectport>` -- the TCP port to connect to.

* `<baselogfile>` -- the prefix of a location that will be used to write log files.
  Whatever you specify here will have the source IP and port of each connection appended to it.

* `<cafile>` -- the path to a file containing PEM-formatted trust anchors (so-called "root certificates") used to verify the authenticity of TLS connections made to the "target" of the proxy.
  By default, we'll use the system CA bundle, if available, via [`rustls-platform-verifier`](https://crates.io/crates/rustls-platform-verifier).


# Log Format

Each log file that is written will contain data in the following form.

The first two lines will look like this:

```
# ACCEPT <rfc3339nano> <srcip>:<port>
# CONNECT <rfc3339nano> <dstip>:<port>
```

This just lets you know what you're dealing with.
The `<rfc3339nano>` timestamp is `<YYYY>-<mm>-<dd>T<HH>:<MM>:<SS>.<nnnnnnnnn><TZ>`, and represents the time that the TLS handshake was completed by the proxy (for `ACCEPT`), or the time that the TLS handshake was completed to the target (for `CONNECT`).

After that, each "chunk" of data passing between the `src` and `dst` will be printed.
Each chunk starts with a header line, which looks like this:

```
<dir> <rfc3339nano> offset=<offset> len=<len>
```

Where:

* `<dir>` is either `>` (chunk passed from `src` to `dst`) or `<` (chunk passed from `dst` to `src`);
* `<rfc3339nano>` is the time at which the *first* packet in the chunk was received by the proxy;
* `<offset>` is the offset of the first octet of the chunk in the stream of data in that direction;
* `<len>` is the number of octets output in this chunk.

After that, the octets of the chunk are printed.

By default, the chunk data is printed in a "text-friendly" form, which essentially involves converting everything that isn't a printable ASCII character into the `.` character (with the exception of the carriage return, ASCII value `0x0D`, which is converted to the two character sigil `\r`).
This is practically ideal for "mostly-text" protocols like HTTP/1.1, SMTP, and so on.
If you want to get a more nuanced look at binary protocols, you can specify either of the `--hex` or `--raw` options, which will write out the chunks as hex-encoded (in a format reminiscent of that produced by the `hd` utility) or exactly as provided (with a trailing newline, so that the next chunk header starts at the beginning of a line), respectively.

The logs can also contain lines that begin with `# ERROR` or `# WARN`, which provide error/warning information.
Nothing after an `# ERROR` line will be network data; instead, it will be diagnostic information, such as stacktraces.


# Maintenance Status

This program is fully maintained, in line with the tenets of [the Maintainer Manifesto](https://maintainermanifesto.org).
In the unlikely event that you wish to pay for a feature or bugfix, the primary author's commercial arm is [Tobermory Technology](https://tobermorytech.com).


# Licence

Unless otherwise stated, everything in this repo is covered by the following
copyright notice:

    Copyright (C) 2024  Matt Palmer <matt@hezmatt.org>

    This program is free software: you can redistribute it and/or modify it
    under the terms of the GNU General Public License version 3, as
    published by the Free Software Foundation.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
