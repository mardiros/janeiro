use transport::{Transport};

 /// reason of a connection closed.
pub enum Reason {
    /// the connection has been lost in an unexpected way
    ConnectionLost,
    /// the protocol has been respected to close the connection.
    HangUp,
}


#[allow(unused_variables)]
/// Instanciate on every connection by a factory, implement your protocol here.
pub trait Protocol {

    /// Call everytime a connection is made, use the transport
    /// to write bytes to the connected peer.
    fn connection_made(&self, transport: &mut Transport) {
    }

    /// Call everytime a peer sent bytes, use the transport,
    /// to write bytes to the connected peer.
    fn data_received(&self, data: &[u8], transport: &mut Transport) {
    }

    /// Call everytime a connection is closed, before the protocol
    /// instance will be destroyed.
    fn connection_lost(&self, reason: Reason) {
    }

}


/// Used by the ioloop to instanciate a protocol on each new connection.
pub trait ServerFactory {

    //fn new() -> Self where Self: Sized;

    /// Called every time a server socket need to handle a client connection.
    /// There is on instance of protocol per connection, that live until
    /// the connection is closed.
    fn build_protocol(&self) -> Box<Protocol>;
}

