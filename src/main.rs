use crossterm::{
    event::{self, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::{Alignment, Backend},
    style::{Color, Style, Styled},
    text::Text,
    widgets::{Block, Borders},
};
use ratatui::{
    prelude::{Constraint, CrosstermBackend, Direction, Layout, Stylize, Terminal},
    widgets::Paragraph,
};
use serialport::SerialPortInfo;
use std::{
    collections::VecDeque,
    io,
    io::{stdout, Result},
    ops::{Add, Range},
    string,
    sync::mpsc::channel,
    thread::sleep,
    time::Duration,
};
use tui_textarea::TextArea;

struct App {
    ports: Vec<SerialPortInfo>,
    // selected_port: Option<& SerialPortInfo>,
    is_active: bool,
    scroll_buffer: VecDeque<String>,
}

impl App {
    pub fn new() -> App {
        App {
            ports: serialport::available_ports().expect("No ports found!"),
            // selected_port: None,
            is_active: false,
            scroll_buffer: VecDeque::with_capacity(1000),
        }
    }

    pub fn toggle_is_active(&mut self) {
        self.is_active = !self.is_active;
    }

    // pub fn select_port(& mut self) {
    //     //, port: &'a SerialPortInfo
    //     self.selected_port = Some(&self.ports[0]);
    // }

    pub fn selected_port(&self, idx: usize) -> Option<&SerialPortInfo> {
        Some(&self.ports[idx])
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    Ok(true)
}

enum currentMode {
    Main,
    Listing,
    Config,
    Writing,
}

fn main() -> Result<()> {
    // let ports = serialport::available_ports().expect("No ports found!");
    // for p in &ports {
    //     println!("{}", p.port_name);
    // }

    // let mut port = serialport::new(&ports[0].port_name, 115_200)
    // .timeout(Duration::from_millis(10))
    // .open().expect("Failed to open port");

    // let output = "AT\r\n".as_bytes();
    // port.write(output).expect("Write failed!");

    // let mut serial_buf: Vec<u8> = vec![0; 100];
    // loop {
    //     if let Ok(size) = port.read(serial_buf.as_mut_slice()){
    //         println!("{}",String::from_utf8(serial_buf.to_ascii_lowercase()).unwrap());
    //     }
    //     else{
    //         // break;
    //     }
    //     sleep(Duration::from_millis(100));
    // }

    // println!("Hello, world!");
    let (tx, rx) = channel::<String>();
    let (port_tx, port_rx) = channel::<String>();
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // TODO main loop
    let mut app = App::new();
    app.is_active = true;
    // app.select_port();

    let thread = std::thread::spawn(move || {
        let mut port_name = String::new();
        port_name = port_rx.recv().expect("Failed to read the port");
        let mut port = serialport::new(&port_name, 115_200)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");
        let mut serial_buf: Vec<u8> = vec![0; 100];
        while true {
            if let Ok(port_name) = port_rx.recv_timeout(Duration::from_millis(10)) {
                port = serialport::new(&port_name, 115_200)
                    .timeout(Duration::from_millis(10))
                    .open()
                    .expect("Failed to Change port");
            }
            if let Ok(size) = port.read(serial_buf.as_mut_slice()) {
                tx.send(String::from_utf8(serial_buf.to_ascii_lowercase()).unwrap());
            } else {
                // break;
            }

            // if let Ok(new_line) = rx.recv_timeout(10) {}
        }
    });
    let mut last_line = String::new();
    let mut textarea = TextArea::default();
    port_tx.send(app.selected_port(0).unwrap().port_name.clone());
    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(5),
                    Constraint::Percentage(95),
                    Constraint::Percentage(0),
                ])
                .split(frame.size());

            let middle = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
                .split(chunks[1]);

            let io_box = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
                .split(middle[1]);

            let title_block = Block::default()
                .border_style(Style::default())
                .borders(Borders::ALL);
            let title = Paragraph::new(Text::styled(
                "nini-Com",
                Style::default().fg(Color::LightYellow),
            ))
            .alignment(Alignment::Center);
            // .block(title_block.clone());

            frame.render_widget(title, chunks[0]);

            let area = frame.size();
            frame.render_widget(
                Paragraph::new(Text::styled(
                    if app.selected_port(0).is_some() {
                        app.selected_port(0).unwrap().port_name.to_owned()
                    } else {
                        "No ports available".to_owned()
                    },
                    Style::default().bg(Color::Yellow),
                ))
                .white()
                .alignment(Alignment::Center)
                .block(title_block.clone()),
                // .on_dark_gray(),
                middle[0],
            );

            // frame.render_widget(
            //     Block::default()
            //         .border_style(Style::default())
            //         .borders(Borders::ALL)
            //         .style(Style::default()),
            //     // .bg(Color::LightMagenta)),
            //     io_box[0],
            // );
            let len = app.scroll_buffer.len();
            last_line = "".to_owned();
            if len > 1 {
                for i in (len - (io_box[0].height as usize)).max(0)..(len) {
                    last_line += app.scroll_buffer[i].as_str();
                }
            } else {
                last_line = format!("{}", io_box[0].height);
            }
            frame.render_widget(
                Paragraph::new(last_line.clone()).block(title_block.clone()),
                // .borders(Borders::ALL)
                // .style(Style::default().bg(Color::LightMagenta)),
                io_box[0],
            );
            frame.render_widget(
                textarea.widget(),
                // Block::default().style(Style::default().bg(Color::DarkGray)),
                io_box[1],
            )
        })?;

        // last_line += &;
        if let Ok(l) = rx.recv_timeout(Duration::from_millis(5)) {
            app.scroll_buffer.push_back(l);
        }
        //     rx.recv_timeout(Duration::from_millis(1))
        //         .unwrap_or_else(|f| "d".to_owned()),
        // );

        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
                if key.kind == KeyEventKind::Press {
                    textarea.input(key);
                    // break;
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
// fn main() {

// let ports = serialport::available_ports().expect("No ports found!");
// for p in &ports {
//     println!("{}", p.port_name);
// }

// let mut port = serialport::new(&ports[0].port_name, 115_200)
// .timeout(Duration::from_millis(10))
// .open().expect("Failed to open port");

// let output = "AT\r\n".as_bytes();
// port.write(output).expect("Write failed!");

// let mut serial_buf: Vec<u8> = vec![0; 100];
// loop {
//     if let Ok(size) = port.read(serial_buf.as_mut_slice()){
//         println!("{}",String::from_utf8(serial_buf.to_ascii_lowercase()).unwrap());
//     }
//     else{
//         // break;
//     }
//     sleep(Duration::from_millis(100));
// }

// println!("Hello, world!");

// }
