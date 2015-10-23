
pub enum Reason {
    PeerHangoutUp,
    HangoutUp
}

pub struct Transport;

impl Transport {

    pub fn new() -> Transport {
        Transport
    }

    pub fn write(&self, data: &str) {
        
    }

    pub fn hang_up(&self) {
        
    }

}


pub trait ServerProtocol {

    fn get_transport(&self) -> &Transport;
    fn connection_made(&self) {}
    fn data_received(&self, data: &str) {}
    fn connection_lost(&self, reason: Reason) {}

}

pub trait ServerFactory {
    fn build_protocol(client: Client) -> Box<ServerProtocol>;
}


pub struct Client;

impl Client {
    
    pub fn new() -> Client {
        Client
    }

}

pub struct Rio;

impl Rio {

    pub fn new() -> Rio {
        Rio
    }

    pub fn listen<T: ServerFactory>(&mut self, addr: &str, server: T) {

    }

    pub fn run_forever(&mut self) {
        
    }
}

