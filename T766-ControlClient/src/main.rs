use crate::puppet::PuppetClient;

mod client;
mod host;
mod puppet;

fn main() {
    let client = PuppetClient::new();
    println!("{:?}", client.apply());
}
