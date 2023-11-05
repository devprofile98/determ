use crossterm::{
    event::{self, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::{Alignment, Backend, Constraint, CrosstermBackend, Direction, Layout, Terminal},
    style::{Color, Modifier, Style},
    symbols,
    text::Text,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
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
use tui_textarea::{Key, TextArea};

struct App {
    ports: Vec<SerialPortInfo>,
    // selected_port: Option<& SerialPortInfo>,
    is_active: bool,
    scroll_buffer: VecDeque<String>,
    mode: currentMode,
}

impl App {
    pub fn new() -> App {
        App {
            ports: serialport::available_ports().expect("No ports found!"),
            // selected_port: None,
            is_active: false,
            scroll_buffer: VecDeque::with_capacity(1000),
            mode: currentMode::Main,
        }
    }

    pub fn toggle_is_active(&mut self) {
        self.is_active = !self.is_active;
    }

    pub fn selected_port(&self, idx: usize) -> Option<&SerialPortInfo> {
        Some(&self.ports[idx])
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    Ok(true)
}

#[derive(PartialEq)]
enum currentMode {
    Main,
    Term,
    Listing,
    Config,
    Writing,
}

enum PortCommand {
    conn(String),
    sendData(String),
}

fn main() -> Result<()> {
    // let output = "AT\r\n".as_bytes();
    // port.write(output).expect("Write failed!");

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
    let c_tx = tx.clone();
    let thread = std::thread::spawn(move || {
        let mut port_name = String::new();
        port_name = port_rx.recv().expect("Failed to read the port");
        let mut port = serialport::new(&port_name, 115_200)
            .timeout(Duration::from_millis(10))
            .open()
            .expect("Failed to open port");
        let mut serial_buf: Vec<u8> = vec![0; 300];
        while true {
            if let Ok(port_name) = port_rx.recv_timeout(Duration::from_millis(10)) {
                match serialport::new(&port_name, 115_200)
                    .timeout(Duration::from_millis(10))
                    .open()
                {
                    Ok(p) => {
                        port = p;
                    }
                    Err(_e) => {}
                }
            }
            if let Ok(size) = port.read(serial_buf.as_mut_slice()) {
                c_tx.send(String::from_utf8(serial_buf.to_ascii_lowercase()).unwrap_or_default());
            } else {
                // break;
            }
        }
    });
    let mut last_line = String::new();
    let mut main_block_title = "Not active".to_owned();
    let mut textarea = TextArea::default();
    // textarea.set_style(Style::default().bg(Color::Yellow));
    textarea.set_block(Block::default().borders(Borders::ALL).title("write"));
    let mut accessible_ports: Vec<ListItem> = app
        .ports
        .iter()
        .map(|p| ListItem::new(p.port_name.clone()))
        .collect();
    let mut state = ListState::default();
    if accessible_ports.len() >= 0 {
        state.select(Some(0));
    }

    if let Err(_e) = port_tx.send(app.selected_port(0).unwrap().port_name.clone()) {
    } else {
        main_block_title = app.selected_port(0).unwrap().port_name.clone();
    }
    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(5),
                    Constraint::Percentage(90),
                    Constraint::Percentage(5),
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
            let selected_block = Block::default()
                .border_style(Style::default().fg(Color::LightGreen))
                .borders(Borders::ALL);
            let title = Paragraph::new(Text::styled(
                "comterm",
                Style::default().fg(Color::LightYellow),
            ))
            .alignment(Alignment::Center);

            frame.render_widget(title, chunks[0]);

            frame.render_stateful_widget(
                List::new(accessible_ports.clone())
                    .block(
                        if app.mode == currentMode::Listing {
                            selected_block.clone()
                        } else {
                            title_block.clone()
                        }
                        .title("ports"),
                    )
                    .style(Style::default().fg(Color::White))
                    .highlight_style(
                        Style::default()
                            .add_modifier(Modifier::ITALIC)
                            .bg(Color::LightGreen),
                    )
                    .highlight_symbol(symbols::block::FIVE_EIGHTHS),
                middle[0],
                &mut state,
            );

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
                Paragraph::new(last_line.clone()).block(
                    if app.mode == currentMode::Term {
                        selected_block.clone()
                    } else {
                        title_block.clone()
                    }
                    .title(main_block_title.clone()),
                ),
                io_box[0],
            );
            textarea.set_block(
                if app.mode == currentMode::Writing {
                    selected_block.clone()
                } else {
                    title_block.clone()
                }
                .title("write message"),
            );
            frame.render_widget(textarea.widget(), io_box[1]);

            // render footer
            let footer = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(8),
                    Constraint::Percentage(2),
                    Constraint::Percentage(90),
                ])
                .split(chunks[2]);

            frame.render_widget(
                Paragraph::new("Quit: Ctrl + q")
                    .style(Style::default().fg(Color::Black))
                    .alignment(Alignment::Center), // .block(title_block.clone())
                footer[0],
            );
        })?;

        if let Ok(l) = rx.recv_timeout(Duration::from_millis(5)) {
            app.scroll_buffer.push_back(l);
        }

        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Char('q')
                    && key.modifiers == KeyModifiers::ALT
                {
                    break;
                }
                if key.kind == KeyEventKind::Press {
                    if app.mode == currentMode::Main {
                        if key.code == KeyCode::Left {
                            app.mode = currentMode::Listing;
                        } else if key.code == KeyCode::Right {
                            app.mode = currentMode::Term;
                        } else if key.code == KeyCode::Down {
                            app.mode = currentMode::Writing;
                        }
                    } else if app.mode == currentMode::Listing {
                        let idx: usize = state.selected().unwrap_or(0);
                        if key.code == KeyCode::Down {
                            if idx < accessible_ports.len() - 1 {
                                state.select(Some(idx + 1));
                            } else {
                                state.select(Some(0));
                            }
                        } else if key.code == KeyCode::Right {
                            app.mode = currentMode::Writing;
                        } else if key.code == KeyCode::Left {
                            app.mode = currentMode::Term;
                        } else if key.code == KeyCode::Up {
                            if idx > 0 {
                                state.select(Some(idx - 1));
                            } else {
                                state.select(Some(accessible_ports.len() - 1));
                            }
                        } else if key.code == KeyCode::Enter {
                            if let Err(_e) = port_tx
                                .clone()
                                .send(app.ports[state.selected().unwrap()].port_name.clone())
                            {
                            } else {
                                main_block_title =
                                    app.ports[state.selected().unwrap()].port_name.clone();
                            }
                        }
                    } else if app.mode == currentMode::Writing {
                        if key.code == KeyCode::Left {
                            app.mode = currentMode::Listing;
                        } else if key.code == KeyCode::Up {
                            app.mode = currentMode::Term;
                        } else {
                            textarea.input(key);
                        }
                    } else if app.mode == currentMode::Term {
                        if key.code == KeyCode::Left {
                            app.mode = currentMode::Listing;
                        } else if key.code == KeyCode::Right {
                            app.mode = currentMode::Writing;
                        } else {
                            // textarea.input(key);
                        }
                    }
                    // break;
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
