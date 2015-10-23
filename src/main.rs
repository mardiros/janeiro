extern crate mio;
extern crate nix;
extern crate bytes;

#[macro_use]
extern crate log;
extern crate env_logger;



mod janeiro;

use janeiro::{Rio, Transport, ServerFactory, ServerProtocol, Client};


struct EchoServerProtocol {
    transport: Transport
}

impl EchoServerProtocol {
    fn new() -> EchoServerProtocol {
        EchoServerProtocol {transport: Transport::new() }
    }
}

impl ServerProtocol for EchoServerProtocol {

 
    fn get_transport(&self) -> &Transport {
        &self.transport
    }

    fn data_received(&self, data: &str) {
        self.get_transport().write(data);
        if data == "bye" {
            self.get_transport().hang_up();
        }
    }
    
}


struct EchoServerFactory;

impl EchoServerFactory {

    fn new() -> EchoServerFactory {
        EchoServerFactory
    }

}

impl ServerFactory for EchoServerFactory {

    fn build_protocol(client: Client) -> Box<ServerProtocol> {
       let proto = EchoServerProtocol::new();
       Box::new(proto)
    }

}


fn main() {
    env_logger::init().unwrap();

    let mut rio = Rio::new();
    let server = EchoServerFactory::new();

    rio.listen("0.0.0.0:8888", server);
    rio.run_forever();

}
