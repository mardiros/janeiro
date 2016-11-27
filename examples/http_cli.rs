extern crate janeiro;

#[macro_use]
extern crate log;
extern crate env_logger;


use std::str;

use janeiro::{Rio, Transport, Protocol, Reason};


struct HttpClientProtocol;
impl HttpClientProtocol {
    fn new() -> HttpClientProtocol {
        HttpClientProtocol
    }
}

impl Protocol for HttpClientProtocol {

    fn connection_made(&self, transport: &mut Transport) {
        debug!("Connection made");
        let data = b"GET /\r\nHost: www.gandi.net\r\nAccept: text/html\r\n\r\n";
        transport.write(data);    
    }

    #[allow(unused_variables)]
    fn data_received(&self, data: &[u8], transport: &mut Transport) {
        let s_data = str::from_utf8(data).unwrap().trim();

        println!("==========================================================");
        println!("{}", s_data);
        println!("==========================================================");
        //transport.hang_up();

    }

    fn connection_lost(&self, reason: Reason) {
        match reason {
            Reason::ConnectionLost => info!("Connection closed by peer"),
            Reason::HangUp => info!("Hang hup"),
            Reason::ConnectionError => println!("Connection error"),
        }
    }
}


fn main() {
    env_logger::init().unwrap();
    info!("Start the client");
    let mut rio = Rio::new();
    let protocol = HttpClientProtocol::new();
    rio.connect("217.70.184.1:80", Box::new(protocol));
    info!("Start running the loop");
    rio.run_forever();
}
