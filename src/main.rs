extern crate mio;
extern crate nix;

#[macro_use]
extern crate log;
extern crate env_logger;


use std::str;

mod janeiro;

use janeiro::{Rio, Transport, ServerFactory, Protocol, Reason};


struct EchoProtocol;
impl EchoProtocol {
    fn new() -> EchoProtocol {
        EchoProtocol
    }
}

impl Protocol for EchoProtocol {

    fn connection_made(&self, transport: &mut Transport) {
        let data = b"Hello from the Echo Server, say bye to quit\n";
        transport.write(data);
    }

    fn data_received(&self, data: &[u8], transport: &mut Transport) {

        transport.write(data);
        let s_data = str::from_utf8(data).unwrap().trim();

        debug!("::: [{}]", s_data);
        if s_data == "bye" {
            info!("Client want to hang hup");
            transport.hang_up();
        }
    }

    fn connection_lost(&self, reason: Reason) {
        match reason {
            Reason::ConnectionLost => info!("Connection closed by peer"),
            Reason::HangUp => info!("Hang hup"),
        }
    }
}


struct EchoServerFactory;

impl ServerFactory for EchoServerFactory {

    fn new() -> EchoServerFactory {
        EchoServerFactory
    }
    fn build_protocol(&self) -> Box<Protocol> {
       let proto = EchoProtocol::new();
       Box::new(proto)
    }

}


fn main() {
    env_logger::init().unwrap();
    info!("Start the echo server");
    let server = EchoServerFactory::new();
    let mut rio = Rio::new();
    rio.listen("0.0.0.0:8888", Box::new(server));
    info!("Start running the loop");
    rio.run_forever();
}
