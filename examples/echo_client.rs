extern crate janeiro;

#[macro_use]
extern crate log;
extern crate env_logger;


use std::str;
use std::io;

use janeiro::{Rio, Transport, Protocol, Reason};


struct EchoClientProtocol {
    running: bool
}

impl EchoClientProtocol {
    fn new() -> EchoClientProtocol {
        EchoClientProtocol{running: true}
    }
}

impl Protocol for EchoClientProtocol {

    fn connection_made(&mut self, _: &mut Transport) {
    }

    #[allow(unused_variables)]
    fn data_received(&mut self, data: &[u8], transport: &mut Transport) {
        let s_data = str::from_utf8(data).unwrap().trim();

        println!("{}", s_data);

        if !self.running {
            println!("I am done");
            return
        }

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(n) => {
                println!("{} bytes read", n);
                println!("{}", input);
            }
            Err(error) => println!("error: {}", error),
        }

        let input_str = input.as_str();
        if input_str == "bye\n".to_string() {
            self.running = false;
        }
        println!("{:?}", self.running);
        transport.write(input_str.as_bytes());

    }

    fn connection_lost(&mut self, reason: Reason) {
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
    let protocol = EchoClientProtocol::new();
    let result = rio.connect("0.0.0.0:8888", Box::new(protocol));

    match result {
        Ok(token) => {
            info!("Start running the loop");
            rio.run_until(&|rio: &Rio| -> bool {
                rio.contains(token)
                });
        },

        Err(_) => {
            panic!("Cannot register client")
        }

    }
}
