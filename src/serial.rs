use std::{
    collections::HashMap,
    io::ErrorKind,
    sync::{
        mpsc::{Receiver, Sender},
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use serialport::SerialPort;

use self::utils::parse_flow;

pub enum CmdType {
    Dtr(bool),
    Rts(bool),
    Raw(String),
}

pub enum PortCommand {
    Write(CmdType),
    ChangePort(String),
    PausePort(String),
}

pub fn read_line(
    port: &mut Box<dyn SerialPort>,
    pending_buffer: &mut Vec<u8>,
    stop_flag: &AtomicBool,
) -> Option<String> {
    let mut serial_buf = [0_u8; 256];
    loop {
        if let Some(newline_idx) = pending_buffer.iter().position(|&byte| byte == b'\n') {
            let mut line = pending_buffer.drain(..=newline_idx).collect::<Vec<_>>();
            while matches!(line.last(), Some(b'\n' | b'\r')) {
                line.pop();
            }

            return Some(String::from_utf8_lossy(&line).into_owned());
        }

        match port.read(&mut serial_buf) {
            Ok(bytes_read) if bytes_read > 0 => {
                pending_buffer.extend_from_slice(&serial_buf[..bytes_read]);
            }
            Ok(_) => {
                if stop_flag.swap(false, Ordering::Relaxed) {
                    return None;
                }
            }
            Err(err) if err.kind() == ErrorKind::TimedOut => {
                if stop_flag.swap(false, Ordering::Relaxed) {
                    return None;
                }
            }
            Err(_) => {
                return None;
            }
        }
    }
}

pub fn serial_thread(
    ui_tx: Sender<(String, String)>,
    port_rx: Receiver<PortCommand>,
    result_tx: Sender<(String, bool)>,
    stop_flag: Arc<AtomicBool>,
) -> JoinHandle<()> {
    let mut serial_bookkeeping = HashMap::new();
    let mut read_buffers: HashMap<String, Vec<u8>> = HashMap::new();
    std::thread::spawn(move || {
        let mut port: Option<Box<dyn SerialPort>> = None;
        let mut port_name = String::new();
        match port_rx.recv().expect("Failed to read the port") {
            PortCommand::ChangePort(req_port_name) => {
                port = Some(
                    serialport::new(&req_port_name, 115_200)
                        .timeout(Duration::from_millis(100))
                        .open()
                        .expect("Failed to open port"),
                );
                port_name = req_port_name;
            }
            _ => {}
        };
        if port.is_some() {
            serial_bookkeeping.insert(port_name.clone(), port.unwrap());
            read_buffers.insert(port_name.clone(), Vec::new());
        }
        loop {
            if let Ok(cmd) = port_rx.recv_timeout(Duration::from_millis(5)) {
                match cmd {
                    PortCommand::ChangePort(req_name) => {
                        if let Some(tmp_port) = serial_bookkeeping.get_mut(&req_name.clone()) {
                            port_name = req_name.clone();
                        } else {
                            match serialport::new(&req_name, 115_200)
                                .timeout(Duration::from_millis(5))
                                .open()
                            {
                                Ok(p) => {
                                    port_name = req_name.clone();
                                    serial_bookkeeping.insert(port_name.clone(), p);
                                    read_buffers.entry(port_name.clone()).or_default();

                                    result_tx.send((req_name.clone(), true));
                                }
                                Err(_e) => {
                                    panic!("{}", _e);
                                }
                            }
                        }
                    }
                    PortCommand::PausePort(req_name) => {
                        serial_bookkeeping.remove(&req_name);
                        read_buffers.remove(&req_name);
                    }
                    PortCommand::Write(cmd) => match cmd {
                        CmdType::Raw(data) => {
                            if let Some(tmp_port) = serial_bookkeeping.get_mut(&port_name.clone()) {
                                ui_tx.send((port_name.clone(), data.clone()));
                                tmp_port.write(data.as_bytes());
                            }
                        }
                        CmdType::Dtr(level) => {
                            if let Some(tmp_port) = serial_bookkeeping.get_mut(&port_name.clone()) {
                                parse_flow(tmp_port, "r1:d0:s1000:d1:r0".to_owned());
                            }
                        }
                        CmdType::Rts(level) => {
                            if let Some(tmp_port) = serial_bookkeeping.get_mut(&port_name.clone()) {
                                parse_flow(
                                    tmp_port,
                                    "r0:d0:s100:d1:r0:s100:r1:d0:r1:s100:r0:d0".to_owned(),
                                );
                            }
                        }
                    },
                }
            }
            if let Some(tmp_port) = serial_bookkeeping.get_mut(&port_name) {
                let pending_buffer = read_buffers.entry(port_name.clone()).or_default();
                if let Some(line_data) = read_line(tmp_port, pending_buffer, stop_flag.as_ref()) {
                    ui_tx.send((port_name.clone(), line_data));
                }
            }
        }
    })
}

pub mod utils {
    use serialport::SerialPort;
    use std::time::Duration;

    pub fn parse_flow(port: &mut Box<dyn SerialPort>, flow_string: String) {
        for p in flow_string.split(":").collect::<Vec<_>>() {
            let op = p.as_bytes()[0] as char;
            let value = p[1..].parse::<u64>().unwrap();
            // println!("{}- {} ", p, value);

            if op == 'd' {
                dtr(port, if value == 0 { false } else { true });
            } else if op == 'r' {
                rts(port, if value == 0 { false } else { true });
            } else if op == 's' {
                sleep(value);
            } else {
            }
        }
    }
    fn dtr(port: &mut Box<dyn SerialPort>, level: bool) {
        port.write_data_terminal_ready(level);
    }
    fn rts(port: &mut Box<dyn SerialPort>, level: bool) {
        port.write_request_to_send(level);
    }
    fn sleep(ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    extern crate libc;
    extern crate libudev;

    use std::os::unix::io::AsRawFd;
    use std::ptr;

    use libc::{c_int, c_short, c_ulong, c_void, timespec};
    use libudev::Event;
    use libudev::EventType;

    #[repr(C)]
    struct pollfd {
        fd: c_int,
        events: c_short,
        revents: c_short,
    }

    #[repr(C)]
    struct sigset_t {
        __private: c_void,
    }

    #[allow(non_camel_case_types)]
    type nfds_t = c_ulong;

    const POLLIN: c_short = 0x0001;

    extern "C" {
        fn ppoll(
            fds: *mut pollfd,
            nfds: nfds_t,
            timeout_ts: *mut timespec,
            sigmask: *const sigset_t,
        ) -> c_int;
    }

    pub fn monitor(context: &libudev::Context) -> Option<Event> {
        if let Ok(mut monitor) = libudev::Monitor::new(context) {
            if let Err(_e) = monitor.match_subsystem_devtype("usb", "usb_device") {
                return None;
            }
            if let Ok(mut socket) = monitor.listen() {
                let mut fds = vec![pollfd {
                    fd: socket.as_raw_fd(),
                    events: POLLIN,
                    revents: 0,
                }];

                // loop {
                let result = unsafe {
                    ppoll(
                        (&mut fds[..]).as_mut_ptr(),
                        fds.len() as nfds_t,
                        ptr::null_mut(),
                        ptr::null(),
                    )
                };

                if result < 0 {
                    // return Err(io::Error::last_os_error());
                    return None;
                }
                println!("TEst!!!!!");

                let event = match socket.receive_event() {
                    Some(evt) => evt,
                    None => return None,
                };

                if event.event_type() == EventType::Add || event.event_type() == EventType::Remove {
                    return Some(event);
                }
                // }
            }

            // thread::sleep(Duration::from_secs(5));
        }
        None
    }
}
