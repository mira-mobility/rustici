use rustici::{Client, Message};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Client::connect(rustici::client::DEFAULT_SOCKET)?;
    let req = Message::new();
    let resp = cli.call("list-sas", &req)?;
    println!("{resp}");
    Ok(())
}
