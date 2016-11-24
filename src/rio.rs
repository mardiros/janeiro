use std::net::SocketAddr;
use std::str::FromStr;

use std::io;
use std::io::{Read, Write};  // Used for TcpStream.read,  TcpStream.write

use mio::{Poll, Token, Events, Event, Ready, PollOpt};
use mio::tcp::{TcpListener, TcpStream};
//use mio::*;
//use mio::tcp::*;

use slab;
use interface::{ServerFactory, Protocol, Reason};
use transport::{Transport};

const CONNS_MAX: usize = 65_536;
const BUF_SIZE: usize = 4096;

type Slab<T> = slab::Slab<T, Token>;


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
    interest: Ready,
    peer_hup: bool,
    transport: Transport,
}

impl ClientConnection {

    fn new(protocol: Box<Protocol>, socket: TcpStream) -> ClientConnection {
        ClientConnection {
            protocol: protocol,
            socket: socket,
            interest: Ready::hup() | Ready::readable(),
            peer_hup: false,
            transport: Transport::new(),
        }
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.socket.peer_addr()
    }

    fn is_finished(&self) -> bool {
        self.interest == Ready::none()
    }

    fn reregister(&mut self,
                  event_loop: &mut Poll,
                  token: Token)
                  -> io::Result<()> {

        // have somewhere to read to and someone to receive from
        if !self.peer_hup || false {
            info!("Insert read interest");
            self.interest.insert(Ready::readable());
        } else {
            self.interest.remove(Ready::readable());
        }

        event_loop.reregister(&self.socket,
                              token,
                              self.interest,
                              PollOpt::edge() | PollOpt::oneshot())
    }


    fn handle_hup(&mut self,
                  event_loop: &mut Poll,
                  token: Token)
                  -> io::Result<()> {
        if self.interest == Ready::hup() {
            self.interest = Ready::none();
            try!(event_loop.deregister(&self.socket));
        } else {
            self.peer_hup = true;
            let _ = self.reregister(event_loop, token);
        }
        Ok(())
    }

    fn handle_read(&mut self,
                   event_loop: &mut Poll,
                   token: Token)
                   -> io::Result<()> {
        {
            let mut read_bytes: Vec<u8> = Vec::new();
            loop {
                let (buf, read_len) = {
                    let mut buf = [0; BUF_SIZE];
                    let read_len = try!(self.socket.read(&mut buf[..]));
                    // let s_data = str::from_utf8(&buf).unwrap();
                    // info!("<<< {}", s_data);
                    (buf, read_len)
                };
                if read_len > 0 {
                    read_bytes.extend(buf[0..read_len].iter());
                    if read_len < BUF_SIZE {
                        info!("Nothing more to read");
                        break;
                    } else {
                        info!("More data to read");
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
                    event_loop: &mut Poll,
                    token: Token)
                    -> io::Result<()> {

        {  // a block where transport is mutable

            debug!("handle write");
            let buf = &mut self.transport.buf();
            let to_write_len = buf.len();
            if to_write_len > 0 {
                //let s_data = str::from_utf8(&buf[..]).unwrap();
                //info!(">>> {}", s_data);

                let result = self.socket.write(&buf[..]);
                match result {
                    Ok(written_len) => {
                        debug!("Write {} bytes", written_len);
                        if to_write_len != written_len {
                            error!("{} bytes to write but {} written, hanging up", to_write_len, written_len);
                            self.peer_hup = true;
                        }
                    },
                    Err(err) => {
                        error!("Error {} while writing to the socket, disconnecting", err);
                        self.peer_hup = true;
                    },
                };
            }
        }

        self.transport.clear();
        if self.transport.hup() {
            self.peer_hup = true;
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


/// The I/O Loop
pub struct Rio {
    poll: Poll,
    connections: Slab<Connection>,
}


impl Rio {

    /// Instanciate the IOLoop, should be called once.
    pub fn new() -> Rio {
        let poll: Poll = Poll::new().unwrap();
        let connections = Slab::with_capacity(CONNS_MAX);
        Rio {
            poll: poll,
            connections: connections,
        }
    }

    /// Will listen on the given address when the loop will start.
    /// The ServerFactory.build_protocol method will be called on every
    /// new client connection.
    pub fn listen(&mut self, addr: &str, server: Box<ServerFactory>) {
        info!("Rio is listenning on {}", addr);
        let sock_addr: SocketAddr = FromStr::from_str(addr).unwrap();
        debug!("Bind the server socket {}", addr);
        let sock = TcpListener::bind(&sock_addr).unwrap();
        let result = self.connections.insert(Connection::new_server(server, sock));
        match result {
            Ok(token) => {
                let _ = self.poll.register(&self.connections[token].server_ref().socket,
                                           token,
                                           Ready::readable(),
                                           PollOpt::edge());
            }
            Err(_) => error!("Cannot register server"),
        }

    }

    /// Will listen on the given address when the loop will start.
    /// The ServerFactory.build_protocol method will be called on every
    /// new client connection.
    pub fn connect(&mut self, addr: &str, client: Box<Protocol>) {
        info!("Rio is connecting to {}", addr);
        let sock_addr: SocketAddr = FromStr::from_str(addr).unwrap();
        debug!("Connect to the socket {}", addr);

        let sock = TcpStream::connect(&sock_addr).unwrap();
        let result = self.connections.insert(Connection::new_client(client, sock));
        match result {
            Ok(token) => {
                let client = self.connections[token].client_mut();
                client.protocol.connection_made(&mut client.transport);
                let _ = self.poll.register(&client.socket,
                                           token,
                                           Ready::readable() | Ready::writable(),
                                           PollOpt::edge());
            }
            Err(_) => error!("Cannot register client"),
        }
    }

    /// Start the io loop
    pub fn run_forever(&mut self) {
        info!("Rio run forever");

        let mut events = Events::with_capacity(1024);

        loop {
            self.poll.poll(&mut events, None).unwrap();

            for event in events.iter() {
                let token = event.token();
                let _ = match self.connections[token].connection_type {
                    ConnectionType::Server => self.handle_server(token),
                    ConnectionType::Client => self.handle_client(token, event),
                };
            }
        }
    }

    fn handle_server(&mut self,
                     token: Token)
                     -> io::Result<()> {
        loop {
            let (sock, addr) = try!(self.connections[token].server_ref().socket.accept());

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
                    try!(self.poll.register(&client.socket,
                                            client_token,
                                            Ready::readable() | Ready::writable(),
                                            PollOpt::edge() | PollOpt::oneshot()
                                            ));
                }
                Err(_) => error!("Cannot register client"),

            }
        }
        Ok(())
    }

    fn handle_client(&mut self,
                     token: Token,
                     event: Event)
                     -> io::Result<()> {
        debug!("handle client",);
        let kind = event.kind();
        if kind.is_hup() {
            debug!("handle hup");
            let (_, finished) = {
                let mut client = self.connections[token].client_mut();
                let mut poll = &mut self.poll;
                let res = client.handle_hup(poll, token);
                (res, client.is_finished())
            };
            if finished {
                info!("Connection closed");
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
        if kind.is_readable() {
            debug!("handle readable {:?}", client_addr);
            let (_, finished) = {
                let mut client = &mut self.connections[token].client_mut();
                let mut poll = &mut self.poll;
                let res = client.handle_read(poll, token);
                (res, client.is_finished())
            };
            self.handle_client_finished(token, finished);
        }

        if kind.is_writable() {
            debug!("handle writable {:?}", client_addr);
            let (_, finished) = {
                let mut client = &mut self.connections[token].client_mut();
                let mut poll = &mut self.poll;
                let res = client.handle_write(poll, token);
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

    fn handle_client_finished(&mut self, token: Token, finished: bool) {
        if finished {
            {
                let client = &self.connections[token].client_ref();
                match client.peer_addr() {
                    Ok(addr) => info!("Connection closed {:?}", addr),
                    Err(_) => error!("Connection closed (Peer already disconnected)"),
                }
                if client.transport.hup() {
                    client.protocol.connection_lost(Reason::HangUp);
                } else {
                    client.protocol.connection_lost(Reason::ConnectionLost);
                }
            }
            self.connections.remove(token);
        }
    }

}
