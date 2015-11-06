use std::net::SocketAddr;
use std::str;
use std::str::FromStr;
use std::io;

// use mio::{EventLoop, Handler, Token, EventSet, PollOpt};
// use mio::tcp::{TcpListener, TcpStream};
use mio::*;
use mio::tcp::*;

use mio::util::Slab;
use interface::{ServerFactory, Protocol, Reason};
use transport::{Transport};

const CONNS_MAX: usize = 65_536;
const BUF_SIZE: usize = 4096;


#[derive(Clone)]
enum ConnectionType {
    Server,
    Client,
}


struct ServerConnection {
    server: Box<ServerFactory>,
    socket: TcpListener,
}


struct ClientConnection {
    protocol: Box<Protocol>,
    socket: TcpStream,
    interest: EventSet,
    peer_hup: bool,
    transport: Transport,
}

impl ClientConnection {

    fn new(protocol: Box<Protocol>, socket: TcpStream) -> ClientConnection {
        ClientConnection {
            protocol: protocol,
            socket: socket,
            interest: EventSet::hup() | EventSet::readable(),
            peer_hup: false,
            transport: Transport::new(),
        }
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.socket.peer_addr()
    }

    fn is_finished(&self) -> bool {
        self.interest == EventSet::none()
    }

    fn reregister(&mut self,
                  event_loop: &mut EventLoop<MioHandler>,
                  token: Token)
                  -> io::Result<()> {

        // have somewhere to read to and someone to receive from
        if !self.peer_hup || false {
            info!("Insert read interest");
            self.interest.insert(EventSet::readable());
        } else {
            self.interest.remove(EventSet::readable());
        }

        event_loop.reregister(&self.socket,
                              token,
                              self.interest,
                              PollOpt::edge() | PollOpt::oneshot())
    }


    fn handle_hup(&mut self,
                  event_loop: &mut EventLoop<MioHandler>,
                  token: Token)
                  -> io::Result<()> {
        if self.interest == EventSet::hup() {
            self.interest = EventSet::none();
            try!(event_loop.deregister(&self.socket));
        } else {
            self.peer_hup = true;
            let _ = self.reregister(event_loop, token);
        }
        Ok(())
    }

    fn handle_read(&mut self,
                   event_loop: &mut EventLoop<MioHandler>,
                   token: Token)
                   -> io::Result<()> {
        {
            let mut read_bytes: Vec<u8> = Vec::new();
            loop {
                let (buf, read_len) = {
                    let mut buf = [0; BUF_SIZE];
                    let read_len = self.socket.try_read(&mut buf[..]);
                    // let s_data = str::from_utf8(&buf).unwrap();
                    // info!("<<< {}", s_data);
                    (buf, read_len)
                };
                match read_len {
                    Ok(None) => {
                        info!("read to end");
                        break;
                    }
                    Ok(Some(len)) => {
                        read_bytes.extend(buf[0..len].iter());
                        if len < BUF_SIZE {
                            info!("Nothing more to read");
                            break;
                        } else {
                            info!("More data to read");
                        }
                    }
                    Err(_) => {
                        info!("Error while reading to socket, consider to hang up");
                        break;
                    }
                }
            }
            let _ = self.reregister(event_loop, token);
            self.protocol.data_received(&read_bytes[..], &mut self.transport);
        }

        if self.transport.should_write() {
            let _ = self.handle_write(event_loop, token);
        }

        Ok(())
    }

    fn handle_write(&mut self,
                    event_loop: &mut EventLoop<MioHandler>,
                    token: Token)
                    -> io::Result<()> {
        {
            info!("handle write");
            loop {
                let (len, res) = {
                    let buf = &mut self.transport.buf();
                    let len = buf.len();
                    let s_data = str::from_utf8(&buf[..]).unwrap();
                    info!(">>> {}", s_data);

                    let res = self.socket.try_write(&buf[..]);
                    (len, res)
                };
                match res {
                    Ok(None) => {
                        break;
                    }
                    Ok(Some(written_len)) => {
                        info!("Write {}, attempt {}", written_len, len);
                        if written_len != len {
                            info!("Something is going wrong with the socket");
                            break;
                        }
                    }
                    Err(_) => {
                        info!("Error while writing to the socket");
                        self.peer_hup = true;
                        break;
                    }
                }
                break;
            }
            self.transport.clear();
            if self.transport.hup() {
                self.peer_hup = true;
            }
        }
        self.reregister(event_loop, token)
    }
}


struct Connection {
    connection_type: ConnectionType,
    server: Option<ServerConnection>,
    client: Option<ClientConnection>,
}


#[allow(dead_code)]
impl Connection {
    fn new_server(server: Box<ServerFactory>, socket: TcpListener) -> Connection {
        Connection {
            connection_type: ConnectionType::Server,
            server: Some(ServerConnection {
                server: server,
                socket: socket,
            }),
            client: None,
        }
    }

    fn new_client(protocol: Box<Protocol>, socket: TcpStream) -> Connection {
        Connection {
            connection_type: ConnectionType::Client,
            client: Some(ClientConnection::new(protocol, socket)),
            server: None,
        }
    }

    fn server_ref(&self) -> &ServerConnection {
        self.server.as_ref().unwrap()
    }

    fn client_ref(&self) -> &ClientConnection {
        self.client.as_ref().unwrap()
    }

    fn client_mut(&mut self) -> &mut ClientConnection {
        self.client.as_mut().unwrap()
    }

    fn alive(&self) -> bool {
        return match self.connection_type {
            ConnectionType::Server => {
                true
            }
            ConnectionType::Client => {
                match self.client_ref().peer_addr() {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
        };
    }

}


struct MioHandler {
    connections: Slab<Connection>,
}


impl MioHandler {

    pub fn new() -> MioHandler {
        MioHandler { connections: Slab::new(CONNS_MAX) }
    }

    fn listen(&mut self,
              event_loop: &mut EventLoop<MioHandler>,
              addr: &str,
              server: Box<ServerFactory>) {
        let sock_addr: SocketAddr = FromStr::from_str(addr).unwrap();
        debug!("Bind the server socket {}", addr);
        let sock = TcpListener::bind(&sock_addr).unwrap();
        let result = self.connections.insert(Connection::new_server(server, sock));
        match result {
            Ok(token) => {
                let _ = event_loop.register(&self.connections[token].server_ref().socket,
                                            token,
                                            EventSet::readable(),
                                            PollOpt::edge());
            }
            Err(_) => error!("Cannot register server"),
        }
    }

    fn handle_server(&mut self,
                     event_loop: &mut EventLoop<MioHandler>,
                     token: Token)
                     -> io::Result<()> {
        loop {
            let (sock, addr) = match try!(self.connections[token].server_ref().socket.accept()) {
                None => break,
                Some(sock) => sock,
            };

            info!("Accepting connection from {:?}", addr);

            debug!("Building procotol");
            let protocol = {
                let server = self.connections[token].server.as_ref();
                server.unwrap().server.build_protocol()
            };

            debug!("Take a token for the connection");
            let result = self.connections.insert(Connection::new_client(protocol, sock));
            match result {
                Ok(client_token) => {
                    debug!("Registering procotol");
                    let mut client = self.connections[client_token].client_mut();
                    client.protocol.connection_made(&mut client.transport);
                    try!(event_loop.register(&client.socket,
                                             client_token,
                                             EventSet::readable() | EventSet::writable(),
                                             PollOpt::edge() | PollOpt::oneshot()));
                }
                Err(_) => error!("Cannot register client"),

            }
        }
        Ok(())
    }

    fn handle_client_finished(&mut self, token: Token, finished: bool) {
        if finished {
            {
                let client = &self.connections[token].client_ref();
                info!("Connection closed {:?}", client.peer_addr().unwrap());
                if client.transport.hup() {
                    client.protocol.connection_lost(Reason::HangUp);
                } else {
                    client.protocol.connection_lost(Reason::ConnectionLost);
                }
            }
            self.connections.remove(token);
        }
    }

    fn handle_client(&mut self,
                     event_loop: &mut EventLoop<MioHandler>,
                     token: Token,
                     events: EventSet)
                     -> io::Result<()> {
        debug!("handle client",);
        if events.is_hup() {
            debug!("handle hup");
            let (_, finished) = {
                let mut client = self.connections[token].client_mut();
                let res = client.handle_hup(event_loop, token);
                (res, client.is_finished())
            };
            if finished {
                info!("Connection closed");
                self.connections.remove(token);
                self.handle_client_finished(token, true);
                info!("Token removed");
            }
        }

        if !&self.connections.contains(token) || !&self.connections[token].alive() {
            debug!("Don't pannic, not handling client");
            return Ok(());
        }

        let client_addr = &self.connections[token].client_ref().peer_addr().unwrap().clone();
        debug!("handle client {:?}", client_addr);
        if events.is_readable() {
            debug!("handle readable {:?}", client_addr);
            let (_, finished) = {
                let mut client = &mut self.connections[token].client_mut();
                let res = client.handle_read(event_loop, token);
                (res, client.is_finished())
            };
            self.handle_client_finished(token, finished);
        }

        if events.is_writable() {
            debug!("handle writable {:?}", client_addr);
            let (_, finished) = {
                let mut client = &mut self.connections[token].client_mut();
                let res = client.handle_write(event_loop, token);
                (res, client.is_finished())
            };
            self.handle_client_finished(token, finished);
        }
        let peer_hup = {
            let client = &self.connections[token].client_ref();
            client.peer_hup
        };
        self.handle_client_finished(token, peer_hup);
        debug!("end handle client {:?}", client_addr);
        Ok(())
    }

}

impl Handler for MioHandler {
    type Timeout = usize;
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<MioHandler>, token: Token, events: EventSet) {

        let _ = match self.connections[token].connection_type {
            ConnectionType::Server => self.handle_server(event_loop, token),
            ConnectionType::Client => self.handle_client(event_loop, token, events),
        };
        return;
    }

}


/// The I/O Loop
pub struct Rio {
    handler: MioHandler,
    event_loop: EventLoop<MioHandler>,
}


impl Rio {

    /// Instanciate the IOLoop, should be called once.
    pub fn new() -> Rio {
        let event_loop: EventLoop<MioHandler> = EventLoop::new().unwrap();
        let handler = MioHandler::new();
        Rio {
            handler: handler,
            event_loop: event_loop,
        }
    }

    /// Will listen on the given address when the loop will start.
    /// The ServerFactory.build_protocol method will be called on every
    /// new client connection.
    pub fn listen(&mut self, addr: &str, server: Box<ServerFactory>) {
        info!("Rio is listenning on {}", addr);
        self.handler.listen(&mut self.event_loop, addr, server);
    }

    /// Start the io loop
    pub fn run_forever(&mut self) {
        info!("Rio run forever");
        self.event_loop.run(&mut self.handler).unwrap();
    }
}
