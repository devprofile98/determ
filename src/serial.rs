use std::{
    collections::HashMap,
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
    let mut serial_bookkeeping = HashMap::new();
    std::thread::spawn(move || {
        let mut port_name = String::new();
        port_name = port_rx.recv().expect("Failed to read the port");
        let mut port = serialport::new(&port_name, 115_200)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");
        serial_bookkeeping.insert(port_name.clone(), port);
        loop {
            if let Ok(port_nam) = port_rx.recv_timeout(Duration::from_millis(10)) {
                if let Some(tmp_port) = serial_bookkeeping.get_mut(&port_nam.clone()) {
                    port_name = port_nam.clone();
                } else {
                    match serialport::new(&port_nam, 115_200)
                        .timeout(Duration::from_millis(10))
                        .open()
                    {
                        Ok(p) => {
                            // panic!("sdfsdfsdf");
                            // port = p;
                            port_name = port_nam.clone();
                            serial_bookkeeping.insert(port_name.clone(), p);

                            result_tx.send((port_nam.clone(), true));
                        }
                        Err(_e) => {
                            // result_tx.send((port_name.clone(), false));
                            panic!("{}", _e);
                        }
                    }
                }
            } else {
            }
            // problem lies here, if we connect to a non responding port
            // then the loop stays here and never got a chance to go up again
            // at begining of the loop and connect to the next port, so we should
            // other techniques to make the change port api like an interrupt
            // so, immediatly after changing port, we should switch the port profile
            if let Some(line_data) =
                read_line(serial_bookkeeping.get_mut(&port_name.clone()).unwrap())
            {
                ui_tx.send((port_name.clone(), line_data));
            }
        }
    })
}
