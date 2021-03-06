extern crate janeiro;

#[macro_use]
extern crate log;
extern crate env_logger;


use std::str;

use janeiro::{Rio, Transport, ServerFactory, Protocol};


struct EchoProtocol;
impl EchoProtocol {
    fn new() -> EchoProtocol {
        EchoProtocol
    }
}

impl Protocol for EchoProtocol {

    fn connection_made(&mut self, transport: &mut Transport) {
        let data = b"Hello from the Echo Server, say bye to quit\n";
        transport.write(data);
    }

    fn data_received(&mut self, data: &[u8], transport: &mut Transport) {

        transport.write(data);
        let s_data = str::from_utf8(data).unwrap().trim();

        //debug!("::: [{}]", s_data);
        if s_data == "bye" {
            //info!("Client want to hang hup");
            transport.hang_up();
        }
    }
    /*
    fn connection_lost(&self, reason: Reason) {
        match reason {
            Reason::ConnectionLost => info!("Connection closed by peer"),
            Reason::HangUp => info!("Hang hup"),
        }
    }
    */
}


struct EchoServerFactory;

impl EchoServerFactory {
    fn new() -> EchoServerFactory {
        EchoServerFactory
    }
}

impl ServerFactory for EchoServerFactory {

    fn build_protocol(&self) -> Box<Protocol> {
       let proto = EchoProtocol::new();
       Box::new(proto)
    }

}


pub fn main() {
    env_logger::init().unwrap();
    info!("Start the echo server");
    let server = EchoServerFactory::new();
    let mut rio = Rio::new();
    let _ = rio.listen("0.0.0.0:8888", Box::new(server));
    info!("Start running the loop");
    rio.run_forever();
}
