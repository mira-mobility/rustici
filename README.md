# rustici

> A minimal Rust client for the strongSwan **VICI** protocol — inspired by the ideas behind `davici`, but implemented from scratch in Rust.

**Status:** experimental MVP. Pure `std`, blocking I/O, UNIX-only. No external deps.

## Features

- Encode/decode VICI **messages** (sections, lists, key/values).
- Encode/decode VICI **packets** and transport framing (32-bit BE length).
- **Blocking** client over `UnixStream` for request/response commands.
- Register/unregister for **events** and read event messages.
- No dependency on `libstrongswan` or `davici` — fresh Rust code.

> Note: This library focuses on the protocol. It intentionally does not try to mirror the exact C API. Instead, it provides a small, idiomatic Rust surface that's easy to extend with higher-level helpers.

## Example

List IKE_SAs using the `list-sas` command:

```rust
use rustici::{Client, Message};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Client::connect(rustici::client::DEFAULT_SOCKET)?;

    // empty request message
    let req = Message::new();

    let resp = cli.call("list-sas", &req)?;
    println!("{}", resp);
    Ok(())
}
```

Build and run example:

```bash
cargo run --example list_sas
```

## Protocol references

- VICI plugin docs: https://docs.strongswan.org/docs/latest/plugins/vici.html
- VICI protocol README (packet/message grammar; transport is length-prefixed with a 32-bit BE header)

## License

Dual-licensed under MIT or Apache-2.0, at your option.
