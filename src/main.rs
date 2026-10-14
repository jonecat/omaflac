use omaflac::app::App;
use omaflac::ui;
use omaflac::updater::Updater;
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

fn print_help() {
    println!("OmaFLAC {} — Audiophile Terminal FLAC & Music Player", Updater::display_version());
    println!();
    println!("USAGE:");
    println!("  omaflac                  Launch the interactive TUI player");
    println!("  omaflac --version, -v    Print version and Git build revision");
    println!("  omaflac check-update     Check GitHub for available updates");
    println!("  omaflac update [--force] Download, compile, and install the latest version");
    println!("  omaflac --help, -h       Print this help message");
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first().map(|s| s.as_str()) {
        match first {
            "--version" | "-v" | "-V" | "version" => {
                println!("OmaFLAC {}", Updater::display_version());
                return Ok(());
            }
            "check-update" | "--check-update" => {
                std::process::exit(Updater::run_cli_check());
            }
            "update" | "--update" | "self-update" => {
                let force = args.iter().any(|a| a == "--force" || a == "-f");
                std::process::exit(Updater::run_cli_update(force));
            }
            "--help" | "-h" | "help" => {
                print_help();
                return Ok(());
            }
            other => {
                eprintln!("Unknown option or command: {}", other);
                eprintln!();
                print_help();
                std::process::exit(2);
            }
        }
    }

    setup_panic_hook();

    // Initialize terminal with alternate screen and mouse capture
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Initialize application state
    let mut app = match App::new() {
        Ok(app) => app,
        Err(e) => {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
            eprintln!("Failed to initialize OmaFLAC: {}", e);
            return Err(e);
        }
    };

    let mut last_tick = Instant::now();

    while !app.should_quit {
        let fps = app.config.visualizer_fps.clamp(10, 240);
        let tick_rate = Duration::from_nanos(1_000_000_000 / fps);

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

    let should_restart = app.should_restart;
    drop(app);

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    )?;
    terminal.show_cursor()?;

    if should_restart {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let bin_path = Updater::install_binary_path();
            let target = if bin_path.exists() {
                bin_path
            } else {
                std::env::current_exe().unwrap_or(bin_path)
            };
            let _ = std::process::Command::new(target).exec();
        }
    }

    println!("OmaFLAC terminated cleanly. Goodbye!");
    Ok(())
}
