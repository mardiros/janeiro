extern crate janeiro;

mod echo_server;

use std::thread;
use std::time::Duration;
use std::io::prelude::*;
use std::net::TcpStream;
use std::io::BufReader;


#[test]
fn test_echo_server() {

    thread::spawn(|| {
        echo_server::main();
    });

    // wait the server start before connect
    for _ in 0..30 {
        match TcpStream::connect("127.0.0.1:8888") {
            Ok(_) => break,
            Err(_) => thread::sleep_ms(40),
        }
    }
    let stream = TcpStream::connect("127.0.0.1:8888").unwrap();
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50_000)));
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    let _ = reader.read_line(&mut line).unwrap();
    assert_eq!("Hello from the Echo Server, say bye to quit\n", line);

    line = "".to_string();
    let _ = reader.get_mut().write(&"hello\n".to_string().into_bytes()[..]);
    let _ = reader.read_line(&mut line).unwrap();
    assert_eq!("hello\n", line);

    line = "".to_string();
    let _ = reader.get_mut().write(&"bye\n".to_string().into_bytes()[..]);
    let _ = reader.read_line(&mut line).unwrap();
    assert_eq!("bye\n", line);

}
