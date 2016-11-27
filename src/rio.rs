use std::net::SocketAddr;
use std::str::FromStr;

use std::io;

use std::io::{Read, Write};  // Used for TcpStream.read,  TcpStream.write
use mio::{Poll, Token, Events, Event, Ready, PollOpt};
use mio::tcp::{TcpListener, TcpStream};

use slab;
use interface::{ServerFactory, Protocol, Reason};
use transport::Transport;

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
    transport: Transport,
}

impl ClientConnection {
    fn new(protocol: Box<Protocol>, socket: TcpStream) -> ClientConnection {
        ClientConnection {
            protocol: protocol,
            socket: socket,
            interest: Ready::hup() | Ready::readable(),
            transport: Transport::new(),
        }
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.socket.peer_addr()
    }

    fn is_finished(&self) -> bool {
        self.interest == Ready::none()
    }

    fn handle_hup(&mut self) {
        self.interest = Ready::none();
    }

    fn handle_read(&mut self) -> io::Result<()> {
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
        self.protocol.data_received(&read_bytes[..], &mut self.transport);

        Ok(())
    }

    fn handle_write(&mut self) {

        {
            // a block where transport muting

            debug!("handle write");
            let buf = &mut self.transport.buf();
            let to_write_len = buf.len();
            if to_write_len > 0 {
                // let s_data = str::from_utf8(&buf[..]).unwrap();
                // info!(">>> {}", s_data);

                let result = self.socket.write(&buf[..]);
                match result {
                    Ok(written_len) => {
                        debug!("Write {} bytes", written_len);
                        if to_write_len != written_len {
                            error!("{} bytes to write but {} written, hanging up",
                                   to_write_len,
                                   written_len);
                            self.interest = Ready::hup();
                        }
                    }
                    Err(err) => {
                        error!("Error {} while writing to the socket, disconnecting", err);
                        self.interest = Ready::hup();
                    }
                };
            }
        };

        self.transport.clear();

        if self.transport.hup() {
            info!("Peer is disconnecting, will unregister connection");
            self.interest = Ready::none();
        }
    }
}


struct Connection {
    connection_type: ConnectionType,
    server: Option<ServerConnection>,
    client: Option<ClientConnection>,
    peer_addr: SocketAddr,
}


#[allow(dead_code)]
impl Connection {
    fn new_server(server: Box<ServerFactory>,
                  peer_addr: SocketAddr,
                  socket: TcpListener)
                  -> Connection {
        Connection {
            connection_type: ConnectionType::Server,
            server: Some(ServerConnection {
                server: server,
                socket: socket,
            }),
            client: None,
            peer_addr: peer_addr,
        }
    }

    fn new_client(protocol: Box<Protocol>, peer_addr: SocketAddr, socket: TcpStream) -> Connection {
        Connection {
            connection_type: ConnectionType::Client,
            client: Some(ClientConnection::new(protocol, socket)),
            server: None,
            peer_addr: peer_addr,
        }
    }

    fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
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
            ConnectionType::Server => true,
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
        let result = self.connections.insert(Connection::new_server(server, sock_addr, sock));
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
        info!("Connecting to socket {}", addr);
        let sock_addr: SocketAddr = FromStr::from_str(addr).unwrap();

        let sock = TcpStream::connect(&sock_addr).unwrap();
        let result = self.connections.insert(Connection::new_client(client, sock_addr, sock));
        match result {
            Ok(token) => {
                let client = self.connections[token].client_mut();
                client.protocol.connection_made(&mut client.transport);
                let _ = self.poll.register(&client.socket,
                                           token,
                                           Ready::readable() | Ready::writable(),
                                           PollOpt::edge());
                debug!(" socket {} registered in the poller", addr);
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

    fn handle_server(&mut self, token: Token) -> io::Result<()> {
        loop {
            let (sock, addr) = try!(self.connections[token].server_ref().socket.accept());

            info!("Accepting connection from {:?}", addr);

            debug!("Building procotol");
            let protocol = {
                let server = self.connections[token].server.as_ref();
                server.unwrap().server.build_protocol()
            };

            debug!("Take a token for the connection");
            let result = self.connections.insert(Connection::new_client(protocol, addr, sock));
            match result {
                Ok(client_token) => {
                    debug!("Registering procotol");
                    let mut client = self.connections[client_token].client_mut();
                    client.protocol.connection_made(&mut client.transport);
                    try!(self.poll.register(&client.socket,
                                            client_token,
                                            Ready::readable() | Ready::writable(),
                                            PollOpt::edge() | PollOpt::oneshot()));
                }
                Err(_) => error!("Cannot register client"),

            }
        }
        Ok(())  // the loop should break with event SIGTERM
    }

    fn handle_client(&mut self, token: Token, event: Event) -> io::Result<()> {
        debug!("handle client, {:?}", event);

        if !&self.connections.contains(token) {
            error!("Ignoring unkown token to handle");
            return Ok(());
        }

        let kind = event.kind();

        if !&self.connections[token].alive() || kind.is_error() {
            error!("Connection failed {:?}", &self.connections[token].peer_addr);
            info!("Removing connection {:?}",
                  &self.connections[token].peer_addr);

            {
                let client = &self.connections[token].client_ref();
                client.protocol.connection_lost(Reason::ConnectionError);
            }
            self.connections.remove(token);
            return Ok(());
        }

        let mut finished = false;
        let client_addr = &self.connections[token].client_ref().peer_addr().unwrap().clone();
        {
            let mut client = &mut self.connections[token].client_mut();

            debug!("handle client {:?}", client_addr);

            if kind.is_hup() {
                debug!("handle hup {:?}", client_addr);
                client.handle_hup();
                finished = true;
            } else {
                if kind.is_readable() {
                    debug!("handle readable {:?}", client_addr);
                    try!(client.handle_read());
                }

                if kind.is_writable() || client.transport.should_write() {
                    debug!("handle writable {:?}", client_addr);
                    client.handle_write();
                }
            }

            if client.is_finished() {
                debug!("Closing connection with {:?}", client_addr);
                match client.peer_addr() {
                    Ok(addr) => info!("Connection closed {:?}", addr),
                    Err(_) => error!("Connection closed (Peer already disconnected)"),
                }
                if client.transport.hup() {
                    info!("Peer hang up {:?}", client_addr);
                    client.protocol.connection_lost(Reason::HangUp);
                } else {
                    info!("Connection lost {:?}", client_addr);
                    client.protocol.connection_lost(Reason::ConnectionLost);
                }
                try!(self.poll.deregister(&client.socket));
                finished = true;
            } else {
                try!(self.poll.reregister(&client.socket, token, client.interest, PollOpt::edge()));
            }
        }


        if finished {
            info!("Removing connection {:?}", client_addr);
            self.connections.remove(token);
        }

        debug!("end handle client {:?}", client_addr);
        Ok(())
    }
}
