extern crate janeiro;

#[macro_use]
extern crate log;
extern crate env_logger;

use std::str;
use std::thread;
use std::process::Command;
use std::time::Duration;
use std::io::prelude::*;
use std::net::TcpStream;
use std::io::BufReader;



// The test is ignored by default because in case of regression
// it will leave the echo_server child process alive.
#[test]
#[ignore]
fn test_echo_server() {
    env_logger::init().unwrap();
    info!("Test echo server");

    let mut child = Command::new("cargo")
        .arg("run")
        .arg("--example=echo_server")
        .spawn().unwrap();

    info!("echo server started in pid {:?}", child.id());

    // wait the server start before connect
    for _ in 0..30 {
        info!("Wait for connection");
        match TcpStream::connect("127.0.0.1:8888") {
            Ok(_) => break,
            Err(_) => {
                thread::sleep(Duration::from_millis(10))
            },
        }
    }


    let stream = TcpStream::connect("127.0.0.1:8888").unwrap();
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

    // todo tests that the stream has been closed by peer here

    child.kill().unwrap();
}
