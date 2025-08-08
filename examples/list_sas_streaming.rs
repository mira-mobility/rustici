
use rustici::{Client, Message};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Client::connect(rustici::client::DEFAULT_SOCKET)?;
    let req = Message::new();
    let final_resp = cli.call_streaming("list-sas", &req, |event, msg| {
        println!("EVENT: {}", event);
        println!("{}", msg);
    })?;
    println!("FINAL RESPONSE:\n{}", final_resp);
    Ok(())
}
