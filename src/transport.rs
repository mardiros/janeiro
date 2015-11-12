

/// Transport is a proxy for the socket write access.
/// Instance are passed as arguments of the trait `Protocol` event method.
pub struct Transport {
    buf: Vec<u8>,
    hup: bool,
}


impl Transport {

    #[doc(hidden)]
    pub fn new() -> Transport {
        Transport {
            buf: Vec::new(),
            hup: false,
        }
    }

    /// Will write the data to connected socket
    pub fn write(&mut self, data: &[u8]) {
        self.buf.extend(data.iter());
    }

    /// Will close the connection
    pub fn hang_up(&mut self) {
        info!("Peer ask to hang up the connection");
        self.hup = true;
    }

    // Not the public api.

    #[doc(hidden)]
    pub fn hup(&self) -> bool {
        self.hup
    }

    #[doc(hidden)]
    pub fn buf(&self) -> &Vec<u8> {
        &self.buf
    }

    #[doc(hidden)]
    pub fn should_write(&self) -> bool {
        !self.buf.is_empty()
    }

    #[doc(hidden)]
    pub fn clear(&mut self) {
        self.buf.clear();
    }

}




#[cfg(test)]
mod test {
    use super::Transport;

    #[test]
    pub fn test_transport() {
        let mut transport = Transport::new();
        transport.write(b"teleport");

        {
            let buf = &transport.buf();
            assert_eq!(&buf[..], b"teleport");
        }

        assert!(&transport.should_write());
        transport.clear();
        assert!(!&transport.should_write());

        assert!(!&transport.hup());
        transport.hang_up();
        assert!(&transport.hup());

    }

}