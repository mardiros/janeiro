use transport::{Transport};

pub enum Reason {
    ConnectionLost,
    HangUp,
}


#[allow(unused_variables)]
pub trait Protocol {

    fn connection_made(&self, transport: &mut Transport) {
    }
    fn data_received(&self, data: &[u8], transport: &mut Transport) {
    }
    fn connection_lost(&self, reason: Reason) {
    }

}

pub trait ServerFactory {
    fn new() -> Self where Self: Sized;
    fn build_protocol(&self) -> Box<Protocol>;
}

