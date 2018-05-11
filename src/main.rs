extern crate tp_lib;

use std::process::Command;


fn main() {
    let mut session = tp_lib::Session::new(Command::new("sh")).unwrap();
    println!("session: {:?}", session);
    session.pk();
}
