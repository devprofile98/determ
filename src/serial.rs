use std::{
    collections::HashMap,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use serialport::SerialPort;

pub enum portCommand {
    Write(String),
    ChangePort(String),
}

pub fn read_line(
    port: &mut Box<dyn SerialPort>,
    stop_flag: Arc<Mutex<bool>>,
    // rx: Receiver<String>,
    // tx: Sender<String>,
) -> Option<String> {
    let mut serial_buf: Vec<u8> = vec![0; 1];
    let mut big_buffer: Vec<u8> = vec![0; 1000];

    loop {
        if port.bytes_to_read().unwrap_or(0) > 0 {
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
        } else if *stop_flag.lock().unwrap() {
            *stop_flag.lock().unwrap() = false;
            return None;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    None
}

pub fn serial_thread(
    ui_tx: Sender<(String, String)>,
    port_rx: Receiver<portCommand>,
    result_tx: Sender<(String, bool)>,
    stop_flag: Arc<Mutex<bool>>,
) -> JoinHandle<()> {
    let mut serial_bookkeeping = HashMap::new();
    std::thread::spawn(move || {
        let mut port: Option<Box<dyn SerialPort>> = None;
        let mut port_name = String::new();
        match port_rx.recv().expect("Failed to read the port") {
            portCommand::ChangePort(req_port_name) => {
                port = Some(
                    serialport::new(&req_port_name, 115_200)
                        .timeout(Duration::from_millis(5))
                        .open()
                        .expect("Failed to open port"),
                );
                port_name = req_port_name;
            }
            _ => {}
        };
        if port.is_some() {
            serial_bookkeeping.insert(port_name.clone(), port.unwrap());
        }
        loop {
            if let Ok(cmd) = port_rx.recv_timeout(Duration::from_millis(5)) {
                match cmd {
                    portCommand::ChangePort(req_name) => {
                        if let Some(tmp_port) = serial_bookkeeping.get_mut(&req_name.clone()) {
                            port_name = req_name.clone();
                        } else {
                            match serialport::new(&req_name, 115_200)
                                .timeout(Duration::from_millis(5))
                                .open()
                            {
                                Ok(p) => {
                                    // panic!("sdfsdfsdf");
                                    // port = p;
                                    port_name = req_name.clone();
                                    serial_bookkeeping.insert(port_name.clone(), p);

                                    result_tx.send((req_name.clone(), true));
                                }
                                Err(_e) => {
                                    // result_tx.send((port_name.clone(), false));
                                    panic!("{}", _e);
                                }
                            }
                        }
                    }
                    portCommand::Write(data) => {
                        if let Some(tmp_port) = serial_bookkeeping.get_mut(&port_name.clone()) {
                            ui_tx.send((port_name.clone(), data.clone()));
                            tmp_port.write(data.as_bytes());
                            tmp_port.flush();
                        }
                    }
                }
            } else {
            }
            if let Some(line_data) = read_line(
                serial_bookkeeping.get_mut(&port_name.clone()).unwrap(),
                stop_flag.clone(),
            ) {
                ui_tx.send((port_name.clone(), line_data));
            }
        }
    })
}
