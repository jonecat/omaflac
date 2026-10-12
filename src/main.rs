mod app;
mod audio;
mod config;
mod library;
mod ui;

use app::App;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::panic;
use std::time::{Duration, Instant};

fn setup_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
        original_hook(panic_info);
    }));
}

fn main() -> anyhow::Result<()> {
    setup_panic_hook();

    // Initialize terminal with alternate screen and mouse capture
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize application state
    let mut app = match App::new() {
        Ok(app) => app,
        Err(e) => {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
            eprintln!("Failed to initialize omaflac: {}", e);
            return Err(e);
        }
    };

    let tick_rate = Duration::from_millis(33); // ~30 FPS for smooth FFT visualizer
    let mut last_tick = Instant::now();

    while !app.should_quit {
        terminal.draw(|f| {
            ui::render_app(f, &mut app);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    // Only process Press events (ignore Release on platforms that send them)
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        app.handle_key(key);
                    }
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                Event::Resize(_, _) => {
                    // Redraw on next iteration
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    )?;
    terminal.show_cursor()?;

    println!("omaflac terminated cleanly. Goodbye!");
    Ok(())
}
