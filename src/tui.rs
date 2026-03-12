//! Basic TUI mode for Synapse using ratatui + crossterm.
//!
//! Provides a full-screen terminal UI with a status bar, scrollable chat area,
//! and an input box. Uses `model.stream_chat()` for responses (same as REPL mode).

use std::sync::Arc;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Terminal;
use synaptic::core::{ChatModel, ChatRequest, Message};

use crate::config::SynapseConfig;

/// A single chat message displayed in the TUI.
struct ChatEntry {
    role: String,
    content: String,
}

/// TUI application state.
struct App {
    /// Chat history for display.
    entries: Vec<ChatEntry>,
    /// Current input buffer.
    input: String,
    /// Scroll offset for the chat area (lines from bottom).
    scroll_offset: u16,
    /// Model name for the status bar.
    model_name: String,
    /// Session ID for the status bar.
    session_id: String,
    /// Messages sent to the model (includes system prompt).
    messages: Vec<Message>,
    /// Whether we are currently waiting for a model response.
    waiting: bool,
}

impl App {
    fn new(model_name: String, session_id: String, messages: Vec<Message>) -> Self {
        Self {
            entries: Vec::new(),
            input: String::new(),
            scroll_offset: 0,
            model_name,
            session_id,
            messages,
            waiting: false,
        }
    }

    fn message_count(&self) -> usize {
        self.entries.len()
    }
}

/// Run the TUI chat interface.
pub async fn run_tui(
    config: &SynapseConfig,
    model: Arc<dyn ChatModel>,
    session_id: &str,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let model_name = model_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.base.model.model.clone());

    // Build initial messages with system prompt
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut system_prompt = config
        .base
        .agent
        .system_prompt
        .clone()
        .unwrap_or_else(|| "You are Synapse, a helpful AI assistant.".to_string());

    let workspace_dir = config.workspace_dir();
    let project_context = crate::agent::load_project_context(&workspace_dir, &cwd, &config.context);
    if !project_context.is_empty() {
        system_prompt.push_str("\n\n# Project Context\n\n");
        system_prompt.push_str(&project_context);
    }

    let messages = vec![Message::system(&system_prompt)];

    let mut app = App::new(model_name, session_id.to_string(), messages);

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, &mut app, model).await;

    // Restore terminal
    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Main event loop: draw UI and handle keyboard events.
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    model: Arc<dyn ChatModel>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Draw
        terminal.draw(|f| draw_ui(f, app))?;

        // Poll for events with a short timeout so we stay responsive
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    // Quit
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => {
                        return Ok(());
                    }
                    // Send message
                    (KeyCode::Enter, _) => {
                        if !app.input.trim().is_empty() && !app.waiting {
                            let user_input = app.input.drain(..).collect::<String>();
                            let user_input = user_input.trim().to_string();

                            // Add user entry
                            app.entries.push(ChatEntry {
                                role: "You".to_string(),
                                content: user_input.clone(),
                            });
                            app.messages.push(Message::human(&user_input));
                            app.scroll_offset = 0;

                            // Stream response
                            app.waiting = true;
                            terminal.draw(|f| draw_ui(f, app))?;

                            let request = ChatRequest::new(app.messages.clone());
                            let mut stream = model.stream_chat(request);

                            let mut full_response = String::new();
                            app.entries.push(ChatEntry {
                                role: "Assistant".to_string(),
                                content: String::new(),
                            });

                            while let Some(chunk) = stream.next().await {
                                match chunk {
                                    Ok(c) => {
                                        full_response.push_str(&c.content);
                                        // Update the last entry in-place
                                        if let Some(last) = app.entries.last_mut() {
                                            last.content = full_response.clone();
                                        }
                                        app.scroll_offset = 0;
                                        terminal.draw(|f| draw_ui(f, app))?;
                                    }
                                    Err(e) => {
                                        if let Some(last) = app.entries.last_mut() {
                                            last.content = format!("[error] {}", e);
                                        }
                                        break;
                                    }
                                }
                            }

                            app.messages.push(Message::ai(&full_response));
                            app.waiting = false;
                        }
                    }
                    // Scroll
                    (KeyCode::PageUp, _) => {
                        app.scroll_offset = app.scroll_offset.saturating_add(10);
                    }
                    (KeyCode::PageDown, _) => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(10);
                    }
                    // Backspace
                    (KeyCode::Backspace, _) => {
                        app.input.pop();
                    }
                    // Regular character input
                    (KeyCode::Char(c), _) => {
                        if !app.waiting {
                            app.input.push(c);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Render the TUI layout.
fn draw_ui(f: &mut ratatui::Frame, app: &App) {
    let size = f.area();

    // Layout: status bar (3 lines) | chat area (flex) | input box (3 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // status bar
            Constraint::Min(5),    // chat area
            Constraint::Length(3), // input box
        ])
        .split(size);

    // --- Status bar ---
    let status_text = format!(
        " Model: {} | Session: {} | Messages: {}{}",
        app.model_name,
        if app.session_id.len() > 12 {
            &app.session_id[..12]
        } else {
            &app.session_id
        },
        app.message_count(),
        if app.waiting { " | Thinking..." } else { "" },
    );
    let status = Paragraph::new(Line::from(vec![Span::styled(
        status_text,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Synapse TUI "),
    );
    f.render_widget(status, chunks[0]);

    // --- Chat area ---
    let mut chat_lines: Vec<Line> = Vec::new();
    for entry in &app.entries {
        let role_style = match entry.role.as_str() {
            "You" => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "Assistant" => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            _ => Style::default().fg(Color::Gray),
        };

        chat_lines.push(Line::from(vec![Span::styled(
            format!("[{}] ", entry.role),
            role_style,
        )]));

        // Wrap content lines
        for line in entry.content.lines() {
            chat_lines.push(Line::from(Span::raw(format!("  {}", line))));
        }
        // Add blank line between entries
        chat_lines.push(Line::from(""));
    }

    let chat_area_height = chunks[1].height.saturating_sub(2); // minus borders
    let total_lines = chat_lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(chat_area_height);
    let effective_scroll = app.scroll_offset.min(max_scroll);
    let scroll_from_top = max_scroll.saturating_sub(effective_scroll);

    let chat = Paragraph::new(chat_lines)
        .block(Block::default().borders(Borders::ALL).title(" Chat "))
        .wrap(Wrap { trim: false })
        .scroll((scroll_from_top, 0));
    f.render_widget(chat, chunks[1]);

    // --- Input box ---
    let input_display = if app.waiting {
        " (waiting for response...)".to_string()
    } else {
        app.input.clone()
    };
    let input = Paragraph::new(Line::from(Span::raw(&input_display))).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Input (Enter=send, Esc/Ctrl-C=quit) "),
    );
    f.render_widget(input, chunks[2]);

    // Place cursor at the end of input
    if !app.waiting {
        let cursor_x = chunks[2].x + 1 + app.input.len() as u16;
        let cursor_y = chunks[2].y + 1;
        f.set_cursor_position((cursor_x.min(chunks[2].right() - 2), cursor_y));
    }
}
