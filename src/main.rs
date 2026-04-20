use core::panic;
use crossterm::{
    event::{self, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::{
        Alignment, Backend, Constraint, CrosstermBackend, Direction, Layout, Margin, Rect,
        Terminal,
    },
    style::{Color, Modifier, Style},
    symbols::scrollbar,
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};
use serial::{utils::monitor, PortCommand};
use serialport::SerialPortInfo;
use std::{
    collections::VecDeque,
    io,
    io::{stdout, Result},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
        Arc,
    },
    time::Duration,
};
use tui_textarea::{Input, Key, TextArea};

mod serial;

#[derive(Default)]
struct RenderCache {
    text: String,
    width: u16,
    height: u16,
    v_scroll: usize,
    line_count: usize,
    dirty: bool,
}

struct Port {
    name: String,
    paused: bool,
    scroll_buffer: VecDeque<String>,
    rts: bool,
    dtr: bool,
    render_cache: RenderCache,
}

impl Port {
    fn new(name: String, paused: bool) -> Port {
        Port {
            name,
            paused,
            scroll_buffer: VecDeque::with_capacity(1000),
            rts: false,
            dtr: false,
            render_cache: RenderCache {
                dirty: true,
                ..RenderCache::default()
            },
        }
    }

    fn mark_render_dirty(&mut self) {
        self.render_cache.dirty = true;
    }

    fn rendered_text(&mut self, width: u16, height: u16, v_scroll: usize) -> &str {
        let line_count = self.scroll_buffer.len();
        if self.render_cache.dirty
            || self.render_cache.width != width
            || self.render_cache.height != height
            || self.render_cache.v_scroll != v_scroll
            || self.render_cache.line_count != line_count
        {
            self.render_cache.text = build_visible_text(&self.scroll_buffer, width, height, v_scroll);
            self.render_cache.width = width;
            self.render_cache.height = height;
            self.render_cache.v_scroll = v_scroll;
            self.render_cache.line_count = line_count;
            self.render_cache.dirty = false;
        }

        self.render_cache.text.as_str()
    }
}

fn build_visible_text(
    scroll_buffer: &VecDeque<String>,
    width: u16,
    height: u16,
    v_scroll: usize,
) -> String {
    if scroll_buffer.len() <= 1 {
        return height.to_string();
    }

    let width = usize::from(width).max(1);
    let height = usize::from(height);
    let len = scroll_buffer.len();
    let start = len
        .saturating_sub(v_scroll)
        .saturating_sub(height)
        .saturating_add(2);
    let end = len.saturating_sub(v_scroll);
    let mut rendered = String::new();

    for curr_line in scroll_buffer.range(start..end) {
        let filtered = curr_line
            .chars()
            .filter(|&c| c != '\0')
            .collect::<String>();

        if filtered.is_empty() {
            rendered.push('\n');
            continue;
        }

        let mut line_width = 0;
        for ch in filtered.chars() {
            rendered.push(ch);
            line_width += 1;
            if line_width >= width {
                line_width = 0;
            }
        }

        if !filtered.ends_with('\n') {
            rendered.push('\n');
        }
    }

    rendered
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let popup_width = width.min(area.width);
    let popup_height = height.min(area.height);
    let x = area.x + area.width.saturating_sub(popup_width) / 2;
    let y = area.y + area.height.saturating_sub(popup_height) / 2;

    Rect {
        x,
        y,
        width: popup_width,
        height: popup_height,
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
        if let Some(port) = self.ports_data.iter_mut().find(|port| port.name == name) {
            port.scroll_buffer.push_back(data);
            port.mark_render_dirty();
        }
    }

    fn is_port_open(&self, name: String) -> bool {
        for i in self.ports_data.iter() {
            if i.name == name {
                return true;
            }
        }
        false
    }

    fn port_data_index(&self, name: &str) -> Option<usize> {
        self.ports_data.iter().position(|port| port.name == name)
    }

    fn ensure_port_data(&mut self, name: &str) -> usize {
        if let Some(idx) = self.port_data_index(name) {
            idx
        } else {
            self.ports_data.push(Port::new(name.to_owned(), false));
            self.ports_data.len() - 1
        }
    }

    fn current_port_title(&self) -> String {
        let active_port = &self.ports_data[self.active_port_idx];
        let status = if active_port.paused { "paused" } else { "active" };
        format!("{} [{}]", active_port.name, status)
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
    let (port_tx, port_rx) = channel::<PortCommand>();
    let (result_tx, result_rx) = channel::<(String, bool)>();
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // TODO main loop
    let mut app = App::new();
    app.is_active = true;
    let stop_flag = Arc::new(AtomicBool::new(false));
    let thread = serial::serial_thread(tx.clone(), port_rx, result_tx, stop_flag.clone());

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

    if let Err(_e) = port_tx.send(PortCommand::ChangePort(
        app.selected_port(0).unwrap().port_name.clone(),
    )) {
    } else {
        app.ports_data.push(Port::new(
            app.selected_port(0).unwrap().port_name.clone(),
            false,
        ));
        main_block_title = app.current_port_title();
    }
    let mut temp_v_scroll = 0;
    let context = libudev::Context::new().unwrap();
    let mut dirty = true;

    loop {
        // if let Some(event) = monitor(&context) {
        //     println!(
        //         "{}: {} {} (subsystem={}, sysname={}, devtype={})",
        //         event.sequence_number(),
        //         event.event_type(),
        //         event.syspath().map_or("", |s| { s.to_str().unwrap_or("") }),
        //         event
        //             .subsystem()
        //             .map_or("", |s| { s.to_str().unwrap_or("") }),
        //         event.sysname().map_or("", |s| { s.to_str().unwrap_or("") }),
        //         event.devtype().map_or("", |s| { s.to_str().unwrap_or("") })
        //     );
        // }
        if dirty {
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

                let terminal_text = {
                    let active_port = &mut app.ports_data[app.active_port_idx];
                    scrollbar_state =
                        scrollbar_state.content_length(active_port.scroll_buffer.len());
                    active_port
                        .rendered_text(io_box[0].width, io_box[0].height, app.v_scroll)
                        .to_owned()
                };
                frame.render_widget(
                    Paragraph::new(terminal_text).block(
                        if app.mode == Mode::Term {
                            selected_block.clone()
                        } else {
                            title_block.clone()
                        }
                        .title(format!(
                            "╮ {} ({}*{}) ╭",
                            main_block_title.clone(),
                            io_box[0].width,
                            io_box[0].height
                        ))
                    ),
                    io_box[0],
                );

                if app.ports_data[app.active_port_idx].paused {
                    let paused_banner = centered_rect(32, 3, io_box[0]);
                    frame.render_widget(Clear, paused_banner);
                    frame.render_widget(
                        Paragraph::new("paused\n(Alt + p) to resume")
                            .alignment(Alignment::Center)
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .style(Style::default().bg(Color::Black).fg(Color::LightYellow))
                                    .title("status"),
                            ),
                        paused_banner,
                    );
                }

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

                frame.render_widget(render_footer(&app.mode), chunks[2]);
            })?;
            dirty = false;
        }

        while let Ok((port_name, recv_data)) = rx.try_recv() {
            app.add_data_with_name(port_name, recv_data);
            dirty = true;
        }

        if event::poll(Duration::from_millis(20))? {
            match event::read()? {
                event::Event::Key(key) => {
                if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Char('q')
                    && key.modifiers == KeyModifiers::ALT
                {
                    break;
                }

                if key.kind == KeyEventKind::Press {
                    if key.code == KeyCode::Char('p') && key.modifiers == KeyModifiers::ALT {
                        let active_port = &mut app.ports_data[app.active_port_idx];
                        if active_port.paused {
                            active_port.paused = false;
                            let _ = port_tx.send(PortCommand::ChangePort(active_port.name.clone()));
                        } else {
                            active_port.paused = true;
                            stop_flag.store(true, Ordering::Relaxed);
                            let _ = port_tx.send(PortCommand::PausePort(active_port.name.clone()));
                        }
                        main_block_title = app.current_port_title();
                        dirty = true;
                        continue;
                    }

                    if app.mode != Mode::Listing
                        && (key.code == KeyCode::Up
                            || key.code == KeyCode::Down
                            || key.code == KeyCode::End)
                    {
                        if key.code == KeyCode::Up {
                            app.v_scroll = app.v_scroll.saturating_add(1);
                            app.ports_data[app.active_port_idx].mark_render_dirty();
                            scrollbar_state = scrollbar_state.position(app.v_scroll);
                            dirty = true;
                            // break;
                        } else if key.code == KeyCode::Down {
                            app.v_scroll = app.v_scroll.saturating_sub(1);
                            app.ports_data[app.active_port_idx].mark_render_dirty();
                            scrollbar_state = scrollbar_state.position(app.v_scroll);
                            dirty = true;
                            // break;
                        } else if key.code == KeyCode::End {
                            app.v_scroll = 0;
                            app.ports_data[app.active_port_idx].mark_render_dirty();
                            scrollbar_state = scrollbar_state.position(app.v_scroll);
                            dirty = true;
                            // break;
                        }
                        continue;
                    }
                    if app.mode == Mode::Main {
                        if key.code == KeyCode::Left {
                            app.mode = Mode::Listing;
                            dirty = true;
                        } else if key.code == KeyCode::Right {
                            app.mode = Mode::Term;
                            dirty = true;
                        } else if key.code == KeyCode::Down {
                            app.mode = Mode::Writing;
                            dirty = true;
                        }
                    } else if app.mode == Mode::Listing {
                        let idx: usize = state.selected().unwrap_or(0);
                        if key.code == KeyCode::Down {
                            if idx < accessible_ports.len() - 1 {
                                state.select(Some(idx + 1));
                            } else {
                                state.select(Some(0));
                            }
                            dirty = true;
                        } else if key.code == KeyCode::Right {
                            app.mode = Mode::Writing;
                            dirty = true;
                        } else if key.code == KeyCode::Left {
                            app.mode = Mode::Term;
                            dirty = true;
                        } else if key.code == KeyCode::Up {
                            if idx > 0 {
                                state.select(Some(idx - 1));
                            } else {
                                state.select(Some(accessible_ports.len() - 1));
                            }
                            dirty = true;
                        } else if key.code == KeyCode::Enter {
                            stop_flag.store(true, Ordering::Relaxed);
                            let selected_port_name =
                                app.ports[state.selected().unwrap()].port_name.clone();
                            if !app.is_port_open(selected_port_name.clone()) {
                                if let Err(_e) =
                                    port_tx.clone().send(PortCommand::ChangePort(selected_port_name.clone()))
                                {
                                    panic!("{}", _e);
                                } else {
                                    app.active_port_idx = app.ensure_port_data(&selected_port_name);
                                    app.ports_data[app.active_port_idx].paused = false;
                                    main_block_title = app.current_port_title();
                                    dirty = true;
                                }
                            } else {
                                app.active_port_idx = app
                                    .port_data_index(&selected_port_name)
                                    .expect("selected port should exist in ports_data");
                                if !app.ports_data[app.active_port_idx].paused {
                                    let _ = port_tx
                                        .clone()
                                        .send(PortCommand::ChangePort(selected_port_name.clone()));
                                }
                                main_block_title = app.current_port_title();
                                dirty = true;
                            }
                        }
                    } else if app.mode == Mode::Writing {
                        if key.code == KeyCode::Enter {
                            let mut tmp_data = textarea.lines()[0].clone();
                            tmp_data.push('\n');
                            port_tx.send(PortCommand::Write(serial::CmdType::Raw(tmp_data)));
                            textarea = TextArea::default();
                            textarea.set_block(Block::default().borders(Borders::ALL).title("write"));
                            stop_flag.store(true, Ordering::Relaxed);
                            dirty = true;
                        } else if key.code == KeyCode::Char('d')
                            && key.modifiers == KeyModifiers::ALT
                        {
                            // set data ready level
                            app.ports_data[app.active_port_idx].dtr =
                                !app.ports_data[app.active_port_idx].dtr;
                            port_tx.send(PortCommand::Write(serial::CmdType::Dtr(
                                app.ports_data[app.active_port_idx].dtr,
                            )));
                            dirty = true;
                        } else if key.code == KeyCode::Char('r')
                            && key.modifiers == KeyModifiers::ALT
                        {
                            //set terminal ready
                            app.ports_data[app.active_port_idx].rts =
                                !app.ports_data[app.active_port_idx].rts;
                            port_tx.send(PortCommand::Write(serial::CmdType::Rts(
                                app.ports_data[app.active_port_idx].rts,
                            )));
                            dirty = true;
                        } else if key.code == KeyCode::Left
                            && key.modifiers == KeyModifiers::CONTROL
                        {
                            textarea.input(Input {
                                key: Key::Left,
                                ctrl: false,
                                alt: false,
                            });
                            dirty = true;
                        } else if key.code == KeyCode::Right
                            && key.modifiers == KeyModifiers::CONTROL
                        {
                            textarea.input(Input {
                                key: Key::Right,
                                ctrl: false,
                                alt: false,
                            });
                            dirty = true;
                        } else if key.code == KeyCode::Left {
                            app.mode = Mode::Listing;
                            dirty = true;
                        } else if key.code == KeyCode::Up {
                            app.mode = Mode::Term;
                            dirty = true;
                        } else if key.code == KeyCode::Char('z')
                            && key.modifiers == KeyModifiers::CONTROL
                        {
                            let mut tmp_data = textarea.lines()[0].clone();
                            tmp_data.push(26 as char);
                            port_tx.send(PortCommand::Write(serial::CmdType::Raw(tmp_data)));
                            textarea = TextArea::default();
                            textarea.set_block(Block::default().borders(Borders::ALL).title("write"));
                            stop_flag.store(true, Ordering::Relaxed);
                            dirty = true;
                        } else {
                            if textarea.input(key) {
                                dirty = true;
                            }
                        }
                    } else if app.mode == Mode::Term {
                        if key.code == KeyCode::Left {
                            app.mode = Mode::Listing;
                            dirty = true;
                        } else if key.code == KeyCode::Right {
                            app.mode = Mode::Writing;
                            dirty = true;
                        } else {
                            // textarea.input(key);
                        }
                    }
                    // break;
                }
                }
                event::Event::Resize(_, _) => {
                    dirty = true;
                }
                _ => {}
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
                Span::raw(" Pause/Resume "),
                Span::styled(" Alt + p ", STYLE),
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
            Span::raw(" Move cursor "),
            Span::styled(" Ctrl + ←/→ ", STYLE),
            Span::raw(" Pause/Resume "),
            Span::styled(" Alt + p ", STYLE),
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
