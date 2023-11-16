use core::panic;
use crossterm::{
    event::{self, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::Rect,
    prelude::{
        Alignment, Backend, Constraint, CrosstermBackend, Direction, Layout, Margin, Terminal,
    },
    style::{Color, Modifier, Style},
    symbols::scrollbar,
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
    Frame,
};
use serial::portCommand;
use serialport::SerialPortInfo;
use std::{
    collections::VecDeque,
    io,
    io::{stdout, Result},
    sync::{mpsc::channel, Arc, Mutex},
    time::Duration,
};
use tui_textarea::TextArea;

mod serial;

struct Port {
    name: String,
    paused: bool,
    scroll_buffer: VecDeque<String>,
    rts: bool,
    dtr: bool,
}

impl Port {
    fn new(name: String, paused: bool) -> Port {
        Port {
            name,
            paused,
            scroll_buffer: VecDeque::with_capacity(1000),
            rts: false,
            dtr: false,
        }
    }
}

struct App {
    ports: Vec<SerialPortInfo>,
    is_active: bool,
    ports_data: Vec<Port>,
    active_port_idx: usize,
    mode: Mode,
    v_scroll: usize,
}

impl App {
    pub fn new() -> App {
        App {
            ports: serialport::available_ports().expect("No ports found!"),
            // selected_port: None,
            is_active: false,
            // scroll_buffer: VecDeque::with_capacity(1000),
            ports_data: Vec::new(),
            active_port_idx: 0,
            mode: Mode::Main,
            v_scroll: 0,
        }
    }

    pub fn toggle_is_active(&mut self) {
        self.is_active = !self.is_active;
    }

    pub fn selected_port(&self, idx: usize) -> Option<&SerialPortInfo> {
        Some(&self.ports[idx])
    }
    fn add_data_with_name(&mut self, name: String, data: String) {
        // for i in self.ports_data.iter_mut() {
        // if i.name == name {
        self.ports_data[0].scroll_buffer.push_back(data.clone());
        //     }
        // }
    }

    fn is_port_open(&self, name: String) -> bool {
        for i in self.ports_data.iter() {
            if i.name == name {
                return true;
            }
        }
        false
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    Ok(true)
}

#[derive(PartialEq)]
enum Mode {
    Main,
    Term,
    Listing,
    Config,
    Writing,
}

fn main() -> Result<()> {
    // let output = "AT\r\n".as_bytes();
    // port.write(output).expect("Write failed!");

    let (tx, rx) = channel::<(String, String)>();
    let (port_tx, port_rx) = channel::<portCommand>();
    let (result_tx, result_rx) = channel::<(String, bool)>();
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // TODO main loop
    let mut app = App::new();
    app.is_active = true;
    let stop_flag = Arc::new(Mutex::new(false));
    let thread = serial::serial_thread(tx.clone(), port_rx, result_tx, stop_flag.clone());

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
    let mut scrollbar_state = ScrollbarState::default();

    if accessible_ports.len() >= 0 {
        state.select(Some(0));
    }

    if let Err(_e) = port_tx.send(portCommand::ChangePort(
        app.selected_port(0).unwrap().port_name.clone(),
    )) {
    } else {
        main_block_title = app.selected_port(0).unwrap().port_name.clone();
        app.ports_data.push(Port::new(
            app.selected_port(0).unwrap().port_name.clone(),
            false,
        ))
    }
    let mut temp_v_scroll = 0;
    let mut len = 0;

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(5),
                    Constraint::Percentage(91),
                    Constraint::Percentage(4),
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
                "⏱ determ",
                Style::default().fg(Color::LightYellow),
            ))
            .alignment(Alignment::Center);

            frame.render_widget(title, chunks[0]);

            frame.render_stateful_widget(
                List::new(accessible_ports.clone())
                    .block(
                        if app.mode == Mode::Listing {
                            selected_block.clone()
                        } else {
                            title_block.clone()
                        }
                        .title("╮ ports ╭"),
                    )
                    .style(Style::default().fg(Color::White))
                    .highlight_style(
                        Style::default()
                            .add_modifier(Modifier::ITALIC)
                            .bg(Color::LightGreen),
                    )
                    .highlight_symbol("●"),
                middle[0],
                &mut state,
            );

            if app.v_scroll <= 0 {
                len = app.ports_data[app.active_port_idx].scroll_buffer.len();
            }
            scrollbar_state = scrollbar_state
                .content_length(app.ports_data[app.active_port_idx].scroll_buffer.len());

            last_line = "".to_owned();
            if len > 1 {
                for i in (len
                    .saturating_sub(app.v_scroll)
                    .saturating_sub(io_box[0].height as usize)
                    // io_box[0].height as usize
                    .saturating_add(2))
                .max(0)..(len.saturating_sub(app.v_scroll))
                {
                    if app.ports_data[app.active_port_idx].scroll_buffer[i].ends_with("\n") {
                        last_line += &format!(
                            "{}",
                            app.ports_data[app.active_port_idx].scroll_buffer[i].as_str(),
                        );
                    } else {
                        last_line += &format!(
                            "{}{}",
                            app.ports_data[app.active_port_idx].scroll_buffer[i].as_str(),
                            "\n"
                        );
                    }
                }
            } else {
                last_line = format!("{}", io_box[0].height);
            }
            frame.render_widget(
                Paragraph::new(last_line.clone()).block(
                    if app.mode == Mode::Term {
                        selected_block.clone()
                    } else {
                        title_block.clone()
                    }
                    .title(format!("╮ {} ╭", main_block_title.clone())),
                ),
                io_box[0],
            );

            frame.render_stateful_widget(
                Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .symbols(scrollbar::VERTICAL),
                io_box[0].inner(&Margin {
                    vertical: 0,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
            textarea.set_block(
                if app.mode == Mode::Writing {
                    selected_block.clone()
                } else {
                    title_block.clone()
                }
                .title("╮ write message ╭"),
            );
            frame.render_widget(textarea.widget(), io_box[1]);

            // render footer
            let footer = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)])
                .split(chunks[2]);

            frame.render_widget(render_footer(&app.mode), chunks[2]);
        })?;

        if let Ok((port_name, recv_data)) = rx.recv_timeout(Duration::from_millis(1)) {
            app.add_data_with_name(port_name, recv_data);
        }

        if event::poll(std::time::Duration::from_millis(10))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Char('q')
                    && key.modifiers == KeyModifiers::ALT
                {
                    break;
                }

                if key.kind == KeyEventKind::Press {
                    if app.mode != Mode::Listing
                        && (key.code == KeyCode::Up
                            || key.code == KeyCode::Down
                            || key.code == KeyCode::End)
                    {
                        if key.code == KeyCode::Up {
                            app.v_scroll = app.v_scroll.saturating_add(1);
                            scrollbar_state = scrollbar_state.position(app.v_scroll);
                            // break;
                        } else if key.code == KeyCode::Down {
                            app.v_scroll = app.v_scroll.saturating_sub(1);
                            scrollbar_state = scrollbar_state.position(app.v_scroll);
                            // break;
                        } else if key.code == KeyCode::End {
                            app.v_scroll = 0;
                            scrollbar_state = scrollbar_state.position(app.v_scroll);
                            // break;
                        }
                        continue;
                    }
                    if app.mode == Mode::Main {
                        if key.code == KeyCode::Left {
                            app.mode = Mode::Listing;
                        } else if key.code == KeyCode::Right {
                            app.mode = Mode::Term;
                        } else if key.code == KeyCode::Down {
                            app.mode = Mode::Writing;
                        }
                    } else if app.mode == Mode::Listing {
                        let idx: usize = state.selected().unwrap_or(0);
                        if key.code == KeyCode::Down {
                            if idx < accessible_ports.len() - 1 {
                                state.select(Some(idx + 1));
                            } else {
                                state.select(Some(0));
                            }
                        } else if key.code == KeyCode::Right {
                            app.mode = Mode::Writing;
                        } else if key.code == KeyCode::Left {
                            app.mode = Mode::Term;
                        } else if key.code == KeyCode::Up {
                            if idx > 0 {
                                state.select(Some(idx - 1));
                            } else {
                                state.select(Some(accessible_ports.len() - 1));
                            }
                        } else if key.code == KeyCode::Enter {
                            *stop_flag.lock().unwrap() = true;
                            if !app.is_port_open(
                                app.ports[state.selected().unwrap()].port_name.clone(),
                            ) {
                                if let Err(_e) = port_tx.clone().send(portCommand::ChangePort(
                                    app.ports[state.selected().unwrap()].port_name.clone(),
                                )) {
                                    panic!("{}", _e);
                                } else {
                                    main_block_title =
                                        app.ports[state.selected().unwrap()].port_name.clone();
                                }
                            } else {
                                port_tx.clone().send(portCommand::ChangePort(
                                    app.ports[state.selected().unwrap()].port_name.clone(),
                                ));
                                main_block_title =
                                    app.ports[state.selected().unwrap()].port_name.clone();
                            }
                        }
                    } else if app.mode == Mode::Writing {
                        if key.code == KeyCode::Enter {
                            let mut tmp_data = textarea.lines()[0].clone();
                            tmp_data.push('\n');
                            port_tx.send(portCommand::Write(serial::cmdType::Raw(tmp_data)));
                            textarea.delete_line_by_head();
                            *stop_flag.lock().unwrap() = true;
                        } else if key.code == KeyCode::Left {
                            app.mode = Mode::Listing;
                        } else if key.code == KeyCode::Up {
                            app.mode = Mode::Term;
                        } else if key.code == KeyCode::Char('z')
                            && key.modifiers == KeyModifiers::CONTROL
                        {
                            let mut tmp_data = textarea.lines()[0].clone();
                            tmp_data.push(26 as char);
                            port_tx.send(portCommand::Write(serial::cmdType::Raw(tmp_data)));
                            textarea.delete_line_by_head();
                            *stop_flag.lock().unwrap() = true;
                        } else if key.code == KeyCode::Char('d')
                            && key.modifiers == KeyModifiers::ALT
                        {
                            // set data ready level
                            app.ports_data[app.active_port_idx].dtr =
                                !app.ports_data[app.active_port_idx].dtr;
                            port_tx.send(portCommand::Write(serial::cmdType::Dtr(
                                app.ports_data[app.active_port_idx].dtr,
                            )));
                        } else if key.code == KeyCode::Char('r')
                            && key.modifiers == KeyModifiers::ALT
                        {
                            //set terminal ready
                            app.ports_data[app.active_port_idx].rts =
                                !app.ports_data[app.active_port_idx].rts;
                            port_tx.send(portCommand::Write(serial::cmdType::Rts(
                                app.ports_data[app.active_port_idx].rts,
                            )));
                        } else {
                            textarea.input(key);
                        }
                    } else if app.mode == Mode::Term {
                        if key.code == KeyCode::Left {
                            app.mode = Mode::Listing;
                        } else if key.code == KeyCode::Right {
                            app.mode = Mode::Writing;
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

fn render_footer<'a>(mode: &Mode) -> Paragraph<'a> {
    const STYLE: Style = Style::new()
        .fg(Color::White)
        .bg(Color::LightGreen)
        .add_modifier(Modifier::BOLD);

    // Style::default().bg(Color::LightGreen).fg(Color::White)

    let line = Line::from(match mode {
        Mode::Config | Mode::Main | Mode::Term | Mode::Listing => {
            vec![
                Span::raw("Quit "),
                Span::styled(" Alt + q ", STYLE),
                Span::raw(" Search "),
                Span::styled("Alt + s ", STYLE),
                Span::raw(" Scroll "),
                Span::styled(" 🠕 🠗 ", STYLE),
            ]
        }
        Mode::Writing => vec![
            Span::raw("Quit "),
            Span::styled(" Alt + q ", STYLE),
            Span::raw(" Enter "),
            Span::styled(r#" \n "#, STYLE),
            Span::raw(" Ctrl + z "),
            Span::styled(r#" \x0A "#, STYLE),
            Span::raw(" Alt+d "),
            Span::styled(r#" DTR "#, STYLE),
            Span::raw(" Alt+r "),
            Span::styled(r#" RTS "#, STYLE),
        ],
    });

    Paragraph::new(line)
    // f.render_widget(, area);
}
