
pub struct Transport {
    buf: Vec<u8>,
    hup: bool,
}


impl Transport {

    pub fn new() -> Transport {
        Transport {
            buf: Vec::new(),
            hup: false,
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.buf.extend(data.iter());
    }

    pub fn hang_up(&mut self) {
        info!("Client ask to hang up the connection");
        self.hup = true;
    }

    // Not the public api.

    pub fn hup(&self) -> bool {
        self.hup
    }

    pub fn buf(&self) -> &Vec<u8> {
        &self.buf
    }

    pub fn should_write(&self) -> bool {
        !self.buf.is_empty()
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

}
