use std::{
    sync::mpsc::{Receiver, Sender},
    thread::JoinHandle,
    time::Duration,
};

use serialport::SerialPort;

pub fn read_line(
    port: &mut Box<dyn SerialPort>,
    // rx: Receiver<String>,
    // tx: Sender<String>,
) -> Option<String> {
    let mut serial_buf: Vec<u8> = vec![0; 1];
    let mut big_buffer: Vec<u8> = vec![0; 1000];

    loop {
        if port.read_exact(&mut serial_buf).is_ok() {
            big_buffer.push(serial_buf[0]);
            if serial_buf[0] == '\n' as u8 {
                match std::str::from_utf8(&big_buffer) {
                    Ok(buffer_str) => {
                        if let Some((line, _)) = buffer_str.split_once("\r\n") {
                            // tx.send(line.to_string()).unwrap();
                            // big_buffer.clear();
                            return Some(line.to_owned());
                        } else if let Some((line, _)) = buffer_str.split_once('\n') {
                            // tx.send(line.to_string()).unwrap();
                            // big_buffer.clear();
                            return Some(line.to_owned());
                        }
                    }
                    Err(e) => {
                        // println!(
                        //     "Error is {} {:?}",
                        //     e,
                        //     std::str::from_utf8(
                        //         big_buffer
                        //             .clone()
                        //             .into_iter()
                        //             .filter(|c| { c.is_ascii() })
                        //             .collect::<Vec<u8>>()
                        //             .as_slice()
                        //     )
                        // );
                        // big_buffer.clear();
                        return None;
                    }
                }
            }
        }
    }
    None
}

pub fn serial_thread(
    ui_tx: Sender<(String, String)>,
    port_rx: Receiver<String>,
    result_tx: Sender<(String, bool)>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut port_name = String::new();
        port_name = port_rx.recv().expect("Failed to read the port");
        let mut port = serialport::new(&port_name, 115_200)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");
        loop {
            if let Ok(port_nam) = port_rx.recv_timeout(Duration::from_millis(10)) {
                match serialport::new(&port_nam, 115_200)
                    .timeout(Duration::from_millis(10))
                    .open()
                {
                    Ok(p) => {
                        // panic!("sdfsdfsdf");
                        port = p;
                        result_tx.send((port_nam.clone(), true));
                    }
                    Err(_e) => {
                        // result_tx.send((port_name.clone(), false));
                        panic!("{}", _e);
                    }
                }
            }

            if let Some(line_data) = read_line(&mut port) {
                ui_tx.send((port_name.clone(), line_data));
            }
        }
    })
}
